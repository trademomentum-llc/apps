//! ELF Linker — Phase 6 of the JStar compiler.
//!
//! Assembles x86-64 machine code into a minimal ELF64 executable.
//! Static linking only in the bootstrap phase (no dynamic linking).
//!
//! Output format:
//!   ELF64 header (64 bytes)
//!   Program header table (1 entry = 56 bytes)
//!   .text section (executable code)
//!   .data section (if any)
//!
//! The _start entry point is at the beginning of .text.

use super::codegen::Arch;
use super::codegen::MachineCode;
use crate::types::{MorphResult, MorphlexError};
use std::path::Path;

// ─── ELF64 Constants ────────────────────────────────────────────────────────

// ELF magic
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

// ELF class
const ELFCLASS64: u8 = 2;

// ELF data encoding
const ELFDATA2LSB: u8 = 1; // little-endian

// ELF version
const EV_CURRENT: u8 = 1;

// ELF OS/ABI
const ELFOSABI_NONE: u8 = 0; // System V

// ELF type
const ET_EXEC: u16 = 2; // executable

// ELF machine
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

// Program header types
const PT_LOAD: u32 = 1;

// Program header flags
const PF_X: u32 = 1; // execute
const PF_W: u32 = 2; // write
const PF_R: u32 = 4; // read

// Header sizes
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: usize = 56;

// Virtual address base (standard Linux user-space / kernel load addresses).
const VADDR_BASE_X86_64: u64 = 0x400000;
/// QEMU `virt` machine loads `-kernel` images at this physical address.
const VADDR_BASE_AARCH64: u64 = 0x40080000;

fn vaddr_base(arch: Arch) -> u64 {
    match arch {
        Arch::X86_64 => VADDR_BASE_X86_64,
        Arch::Aarch64 => VADDR_BASE_AARCH64,
    }
}

/// Link machine code into an ELF64 executable.
pub fn link(code: &MachineCode, output_path: &Path, arch: Arch) -> MorphResult<()> {
    // Patch data section addresses in the .text before building ELF
    let mut code = code.clone();
    patch_data_addresses(&mut code, arch);
    let elf = build_elf(&code, arch)?;

    std::fs::write(output_path, &elf).map_err(MorphlexError::IoError)?;

    // Set executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(output_path, perms).map_err(MorphlexError::IoError)?;
    }

    Ok(())
}

/// Patch data section addresses and function addresses in the .text section.
///
/// Uses the data_fixups list from codegen: each entry is the byte offset
/// in .text of an 8-byte value (a .data section offset) to which we add
/// the actual data vaddr (VADDR_BASE + text_size, plus the ELF headers
/// size on x86-64 where the headers are part of the loaded image).
///
/// Uses the text_fixups list from codegen: each entry is the byte offset
/// in .text of an 8-byte function-offset placeholder to which we add the
/// .text base virtual address.
///
/// This replaces the old byte-pattern scanning approach. Every movabs
/// that references .data now records its fixup position explicitly.
fn patch_data_addresses(code: &mut MachineCode, arch: Arch) {
    if code.data.is_empty() && code.data_fixups.is_empty() && code.text_fixups.is_empty() {
        return;
    }

    let headers_size = ELF64_EHDR_SIZE + ELF64_PHDR_SIZE;
    // AArch64 raw images are booted with the first .text byte at VADDR_BASE
    // (the ELF headers are stripped when producing the .bin), so code/data
    // vaddrs start at the base itself rather than after the headers.
    let data_offset = match arch {
        Arch::X86_64 => headers_size + code.text.len(),
        Arch::Aarch64 => code.text.len(),
    };
    let data_vaddr = vaddr_base(arch) + data_offset as u64;
    let text_vaddr = vaddr_base(arch)
        + match arch {
            Arch::X86_64 => headers_size as u64,
            Arch::Aarch64 => 0,
        };

    eprintln!(
        "[linker] arch={:?} text_len={} data_vaddr={:#x} text_vaddr={:#x} data_fixups={} text_fixups={}",
        arch,
        code.text.len(),
        data_vaddr,
        text_vaddr,
        code.data_fixups.len(),
        code.text_fixups.len()
    );
    for &fixup_pos in &code.data_fixups {
        if fixup_pos + 8 <= code.text.len() {
            let offset_bytes: [u8; 8] = code.text[fixup_pos..fixup_pos + 8].try_into().unwrap();
            let current_val = u64::from_le_bytes(offset_bytes);
            let patched = current_val + data_vaddr;
            eprintln!(
                "[linker] data fixup @ {} current={:#x} patched={:#x}",
                fixup_pos, current_val, patched
            );
            code.text[fixup_pos..fixup_pos + 8].copy_from_slice(&patched.to_le_bytes());
        }
    }
    for &fixup_pos in &code.text_fixups {
        if fixup_pos + 8 <= code.text.len() {
            let offset_bytes: [u8; 8] = code.text[fixup_pos..fixup_pos + 8].try_into().unwrap();
            let current_val = u64::from_le_bytes(offset_bytes);
            let patched = current_val + text_vaddr;
            eprintln!(
                "[linker] text fixup @ {} current={:#x} patched={:#x}",
                fixup_pos, current_val, patched
            );
            code.text[fixup_pos..fixup_pos + 8].copy_from_slice(&patched.to_le_bytes());
        }
    }
}

/// Build the complete ELF64 binary in memory.
///
/// Uses a single PT_LOAD segment (R+W+X) for the bootstrap compiler.
/// This avoids multi-segment mapping complexity. All code and data
/// are in one segment mapped at VADDR_BASE.
fn build_elf(code: &MachineCode, arch: Arch) -> MorphResult<Vec<u8>> {
    let text_size = code.text.len();
    let data_size = code.data.len();

    let headers_size = ELF64_EHDR_SIZE + ELF64_PHDR_SIZE;

    // File segment = text + initialized data (no BSS)
    let segment_size = text_size + data_size;
    // Memory segment = file segment + BSS (zero-filled by kernel)
    let mem_segment_size = segment_size + code.bss_size;

    // Entry point = start of .text. x86-64 images keep the ELF headers inside
    // the loaded segment, so .text begins right after them; AArch64 images
    // are stripped to a raw .bin and booted directly at VADDR_BASE, so the
    // segment starts at .text and the headers are not part of the image.
    let base = vaddr_base(arch);
    let (entry_point, seg_offset, seg_file_size, seg_mem_size) = match arch {
        Arch::X86_64 => (
            base + headers_size as u64,
            0u64,
            (headers_size + segment_size) as u64,
            (headers_size + mem_segment_size) as u64,
        ),
        Arch::Aarch64 => (
            base,
            headers_size as u64,
            segment_size as u64,
            mem_segment_size as u64,
        ),
    };

    let mut elf = Vec::with_capacity(headers_size + segment_size);

    // ─── ELF Header (64 bytes) ──────────────────────────────────────────

    elf.extend_from_slice(&ELF_MAGIC);
    elf.push(ELFCLASS64);
    elf.push(ELFDATA2LSB);
    elf.push(EV_CURRENT);
    elf.push(ELFOSABI_NONE);
    elf.extend_from_slice(&[0u8; 8]); // padding

    let emachine = match arch {
        Arch::X86_64 => EM_X86_64,
        Arch::Aarch64 => EM_AARCH64,
    };

    elf.extend_from_slice(&ET_EXEC.to_le_bytes());
    elf.extend_from_slice(&emachine.to_le_bytes());
    elf.extend_from_slice(&1u32.to_le_bytes()); // version
    elf.extend_from_slice(&entry_point.to_le_bytes());
    elf.extend_from_slice(&(ELF64_EHDR_SIZE as u64).to_le_bytes()); // phoff
    elf.extend_from_slice(&0u64.to_le_bytes()); // shoff
    elf.extend_from_slice(&0u32.to_le_bytes()); // flags
    elf.extend_from_slice(&(ELF64_EHDR_SIZE as u16).to_le_bytes());
    elf.extend_from_slice(&(ELF64_PHDR_SIZE as u16).to_le_bytes());
    elf.extend_from_slice(&1u16.to_le_bytes()); // phnum = 1
    elf.extend_from_slice(&0u16.to_le_bytes()); // shentsize
    elf.extend_from_slice(&0u16.to_le_bytes()); // shnum
    elf.extend_from_slice(&0u16.to_le_bytes()); // shstrndx

    assert_eq!(elf.len(), ELF64_EHDR_SIZE);

    // ─── Single Program Header: PT_LOAD (R+W+X) ────────────────────────
    // x86-64 maps the entire file from offset 0 (headers included); AArch64
    // maps only .text+.data starting at VADDR_BASE.

    elf.extend_from_slice(&PT_LOAD.to_le_bytes());
    elf.extend_from_slice(&(PF_R | PF_W | PF_X).to_le_bytes()); // rwx
    elf.extend_from_slice(&seg_offset.to_le_bytes()); // p_offset
    elf.extend_from_slice(&base.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&base.to_le_bytes()); // p_paddr
    elf.extend_from_slice(&seg_file_size.to_le_bytes()); // p_filesz
    elf.extend_from_slice(&seg_mem_size.to_le_bytes()); // p_memsz (includes BSS)
    elf.extend_from_slice(&0x1000u64.to_le_bytes()); // p_align

    assert_eq!(elf.len(), headers_size);

    // ─── .text section ──────────────────────────────────────────────────

    elf.extend_from_slice(&code.text);

    // ─── .data section ──────────────────────────────────────────────────

    if data_size > 0 {
        elf.extend_from_slice(&code.data);
    }

    Ok(elf)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_magic() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90], // nop
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        assert_eq!(&elf[0..4], &ELF_MAGIC);
    }

    #[test]
    fn test_elf_class_64() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        assert_eq!(elf[4], ELFCLASS64);
    }

    #[test]
    fn test_elf_machine_x86_64() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        let machine = u16::from_le_bytes([elf[18], elf[19]]);
        assert_eq!(machine, EM_X86_64);
    }

    #[test]
    fn test_elf_header_size() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        // ELF header (64) + 1 phdr (56) + 1 byte text = 121
        assert_eq!(elf.len(), ELF64_EHDR_SIZE + ELF64_PHDR_SIZE + 1);
    }

    #[test]
    fn test_elf_entry_point() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        let entry = u64::from_le_bytes(elf[24..32].try_into().unwrap());
        let expected = VADDR_BASE_X86_64 + (ELF64_EHDR_SIZE + ELF64_PHDR_SIZE) as u64;
        assert_eq!(entry, expected);
    }

    #[test]
    fn test_elf_with_data_section() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![0x42, 0x43],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::X86_64).unwrap();
        // Single PT_LOAD segment — always 1 program header
        let phnum = u16::from_le_bytes([elf[56], elf[57]]);
        assert_eq!(phnum, 1);
        // Total size: header + 1 phdr + 1 text + 2 data
        assert_eq!(elf.len(), ELF64_EHDR_SIZE + ELF64_PHDR_SIZE + 1 + 2);
    }

    #[test]
    fn test_elf_entry_point_aarch64() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x1F, 0x20, 0x03, 0xD5], // nop
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let elf = build_elf(&code, Arch::Aarch64).unwrap();
        let entry = u64::from_le_bytes(elf[24..32].try_into().unwrap());
        // Raw AArch64 images boot with the first .text byte at VADDR_BASE.
        assert_eq!(entry, VADDR_BASE_AARCH64);
        // The LOAD segment starts at .text (after the ELF headers).
        let p_offset = u64::from_le_bytes(elf[72..80].try_into().unwrap());
        assert_eq!(p_offset, (ELF64_EHDR_SIZE + ELF64_PHDR_SIZE) as u64);
        let p_vaddr = u64::from_le_bytes(elf[80..88].try_into().unwrap());
        assert_eq!(p_vaddr, VADDR_BASE_AARCH64);
    }

    #[test]
    fn test_elf_determinism() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0xB8, 0x01, 0x00, 0x00, 0x00], // mov eax, 1
            data: vec![],
            bss_size: 0,
            stack_size: 0,
            data_fixups: vec![],
            text_fixups: vec![],
        };
        let a = build_elf(&code, Arch::X86_64).unwrap();
        let b = build_elf(&code, Arch::X86_64).unwrap();
        assert_eq!(a, b, "ELF output must be deterministic");
    }
}
