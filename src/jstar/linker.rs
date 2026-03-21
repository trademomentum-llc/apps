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

use crate::types::{MorphResult, MorphlexError};
use super::codegen::MachineCode;
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

// Program header types
const PT_LOAD: u32 = 1;

// Program header flags
const PF_X: u32 = 1; // execute
const PF_W: u32 = 2; // write
const PF_R: u32 = 4; // read

// Header sizes
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: usize = 56;

// Virtual address base (standard Linux user-space)
const VADDR_BASE: u64 = 0x400000;

/// Link machine code into an ELF64 executable.
pub fn link(code: &MachineCode, output_path: &Path) -> MorphResult<()> {
    // Patch data section addresses in the .text before building ELF
    let mut code = code.clone();
    patch_data_addresses(&mut code);
    let elf = build_elf(&code)?;

    std::fs::write(output_path, &elf)
        .map_err(|e| MorphlexError::IoError(e))?;

    // Set executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(output_path, perms)
            .map_err(|e| MorphlexError::IoError(e))?;
    }

    Ok(())
}

/// Patch data section addresses in the .text section.
///
/// Uses the data_fixups list from codegen: each entry is the byte offset
/// in .text of an 8-byte value (a .data section offset) to which we add
/// the actual data vaddr (VADDR_BASE + headers + text_size).
///
/// This replaces the old byte-pattern scanning approach. Every movabs
/// that references .data now records its fixup position explicitly.
fn patch_data_addresses(code: &mut MachineCode) {
    if code.data.is_empty() && code.data_fixups.is_empty() {
        return;
    }

    let headers_size = ELF64_EHDR_SIZE + ELF64_PHDR_SIZE;
    let data_offset = headers_size + code.text.len();
    let data_vaddr = VADDR_BASE + data_offset as u64;

    for &fixup_pos in &code.data_fixups {
        if fixup_pos + 8 <= code.text.len() {
            let offset_bytes: [u8; 8] = code.text[fixup_pos..fixup_pos + 8]
                .try_into()
                .unwrap();
            let current_val = u64::from_le_bytes(offset_bytes);
            let patched = current_val + data_vaddr;
            code.text[fixup_pos..fixup_pos + 8]
                .copy_from_slice(&patched.to_le_bytes());
        }
    }
}

/// Build the complete ELF64 binary in memory.
///
/// Uses a single PT_LOAD segment (R+W+X) for the bootstrap compiler.
/// This avoids multi-segment mapping complexity. All code and data
/// are in one segment mapped at VADDR_BASE.
fn build_elf(code: &MachineCode) -> MorphResult<Vec<u8>> {
    let text_size = code.text.len();
    let data_size = code.data.len();

    let headers_size = ELF64_EHDR_SIZE + ELF64_PHDR_SIZE;

    // Total segment size = text + data
    let segment_size = text_size + data_size;

    // Entry point = start of .text (right after headers)
    let entry_point = VADDR_BASE + headers_size as u64;

    let mut elf = Vec::with_capacity(headers_size + segment_size);

    // ─── ELF Header (64 bytes) ──────────────────────────────────────────

    elf.extend_from_slice(&ELF_MAGIC);
    elf.push(ELFCLASS64);
    elf.push(ELFDATA2LSB);
    elf.push(EV_CURRENT);
    elf.push(ELFOSABI_NONE);
    elf.extend_from_slice(&[0u8; 8]); // padding

    elf.extend_from_slice(&ET_EXEC.to_le_bytes());
    elf.extend_from_slice(&EM_X86_64.to_le_bytes());
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
    // Maps the entire file from offset 0 so header/text/data are all in one segment.

    elf.extend_from_slice(&PT_LOAD.to_le_bytes());
    elf.extend_from_slice(&(PF_R | PF_W | PF_X).to_le_bytes()); // rwx
    elf.extend_from_slice(&0u64.to_le_bytes()); // p_offset: start of file
    elf.extend_from_slice(&VADDR_BASE.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&VADDR_BASE.to_le_bytes()); // p_paddr
    let total_file_size = (headers_size + segment_size) as u64;
    elf.extend_from_slice(&total_file_size.to_le_bytes()); // p_filesz
    elf.extend_from_slice(&total_file_size.to_le_bytes()); // p_memsz
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
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        assert_eq!(&elf[0..4], &ELF_MAGIC);
    }

    #[test]
    fn test_elf_class_64() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        assert_eq!(elf[4], ELFCLASS64);
    }

    #[test]
    fn test_elf_machine_x86_64() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        let machine = u16::from_le_bytes([elf[18], elf[19]]);
        assert_eq!(machine, EM_X86_64);
    }

    #[test]
    fn test_elf_header_size() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        // ELF header (64) + 1 phdr (56) + 1 byte text = 121
        assert_eq!(elf.len(), ELF64_EHDR_SIZE + ELF64_PHDR_SIZE + 1);
    }

    #[test]
    fn test_elf_entry_point() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        let entry = u64::from_le_bytes(elf[24..32].try_into().unwrap());
        let expected = VADDR_BASE + (ELF64_EHDR_SIZE + ELF64_PHDR_SIZE) as u64;
        assert_eq!(entry, expected);
    }

    #[test]
    fn test_elf_with_data_section() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0x90],
            data: vec![0x42, 0x43],
            stack_size: 0,
            data_fixups: vec![],
        };
        let elf = build_elf(&code).unwrap();
        // Single PT_LOAD segment — always 1 program header
        let phnum = u16::from_le_bytes([elf[56], elf[57]]);
        assert_eq!(phnum, 1);
        // Total size: header + 1 phdr + 1 text + 2 data
        assert_eq!(
            elf.len(),
            ELF64_EHDR_SIZE + ELF64_PHDR_SIZE + 1 + 2
        );
    }

    #[test]
    fn test_elf_determinism() {
        let code = MachineCode {
            data_vaddr: 0,
            text: vec![0xB8, 0x01, 0x00, 0x00, 0x00], // mov eax, 1
            data: vec![],
            stack_size: 0,
            data_fixups: vec![],
        };
        let a = build_elf(&code).unwrap();
        let b = build_elf(&code).unwrap();
        assert_eq!(a, b, "ELF output must be deterministic");
    }
}
