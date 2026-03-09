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
    let elf = build_elf(code)?;

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

/// Build the complete ELF64 binary in memory.
fn build_elf(code: &MachineCode) -> MorphResult<Vec<u8>> {
    let text_size = code.text.len();
    let data_size = code.data.len();

    // Calculate number of program headers
    let num_phdrs = if data_size > 0 { 2 } else { 1 };
    let headers_size = ELF64_EHDR_SIZE + (num_phdrs * ELF64_PHDR_SIZE);

    // .text starts right after headers
    let text_offset = headers_size;
    let text_vaddr = VADDR_BASE + text_offset as u64;

    // .data follows .text (page-aligned)
    let data_offset = text_offset + text_size;
    let data_vaddr = VADDR_BASE + data_offset as u64;

    // Entry point = start of .text
    let entry_point = text_vaddr;

    let mut elf = Vec::with_capacity(headers_size + text_size + data_size);

    // ─── ELF Header (64 bytes) ──────────────────────────────────────────

    // e_ident[0..4]: magic
    elf.extend_from_slice(&ELF_MAGIC);
    // e_ident[4]: class (64-bit)
    elf.push(ELFCLASS64);
    // e_ident[5]: data (little-endian)
    elf.push(ELFDATA2LSB);
    // e_ident[6]: version
    elf.push(EV_CURRENT);
    // e_ident[7]: OS/ABI
    elf.push(ELFOSABI_NONE);
    // e_ident[8..16]: padding
    elf.extend_from_slice(&[0u8; 8]);

    // e_type: executable
    elf.extend_from_slice(&ET_EXEC.to_le_bytes());
    // e_machine: x86-64
    elf.extend_from_slice(&EM_X86_64.to_le_bytes());
    // e_version
    elf.extend_from_slice(&1u32.to_le_bytes());
    // e_entry: entry point virtual address
    elf.extend_from_slice(&entry_point.to_le_bytes());
    // e_phoff: program header table offset (right after ELF header)
    elf.extend_from_slice(&(ELF64_EHDR_SIZE as u64).to_le_bytes());
    // e_shoff: section header table offset (0 = none)
    elf.extend_from_slice(&0u64.to_le_bytes());
    // e_flags
    elf.extend_from_slice(&0u32.to_le_bytes());
    // e_ehsize: ELF header size
    elf.extend_from_slice(&(ELF64_EHDR_SIZE as u16).to_le_bytes());
    // e_phentsize: program header entry size
    elf.extend_from_slice(&(ELF64_PHDR_SIZE as u16).to_le_bytes());
    // e_phnum: number of program headers
    elf.extend_from_slice(&(num_phdrs as u16).to_le_bytes());
    // e_shentsize: section header entry size (0 = none)
    elf.extend_from_slice(&0u16.to_le_bytes());
    // e_shnum: number of section headers
    elf.extend_from_slice(&0u16.to_le_bytes());
    // e_shstrndx: section name string table index
    elf.extend_from_slice(&0u16.to_le_bytes());

    assert_eq!(elf.len(), ELF64_EHDR_SIZE);

    // ─── Program Header: .text (PT_LOAD, R+X) ──────────────────────────

    // p_type
    elf.extend_from_slice(&PT_LOAD.to_le_bytes());
    // p_flags: read + execute
    elf.extend_from_slice(&(PF_R | PF_X).to_le_bytes());
    // p_offset: file offset of segment
    elf.extend_from_slice(&(text_offset as u64).to_le_bytes());
    // p_vaddr: virtual address
    elf.extend_from_slice(&text_vaddr.to_le_bytes());
    // p_paddr: physical address (same as vaddr)
    elf.extend_from_slice(&text_vaddr.to_le_bytes());
    // p_filesz: size in file
    elf.extend_from_slice(&(text_size as u64).to_le_bytes());
    // p_memsz: size in memory
    elf.extend_from_slice(&(text_size as u64).to_le_bytes());
    // p_align: alignment
    elf.extend_from_slice(&0x1000u64.to_le_bytes());

    // ─── Program Header: .data (PT_LOAD, R+W) if needed ────────────────

    if data_size > 0 {
        elf.extend_from_slice(&PT_LOAD.to_le_bytes());
        elf.extend_from_slice(&(PF_R | PF_W).to_le_bytes());
        elf.extend_from_slice(&(data_offset as u64).to_le_bytes());
        elf.extend_from_slice(&data_vaddr.to_le_bytes());
        elf.extend_from_slice(&data_vaddr.to_le_bytes());
        elf.extend_from_slice(&(data_size as u64).to_le_bytes());
        elf.extend_from_slice(&(data_size as u64).to_le_bytes());
        elf.extend_from_slice(&0x1000u64.to_le_bytes());
    }

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
            text: vec![0x90], // nop
            data: vec![],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        assert_eq!(&elf[0..4], &ELF_MAGIC);
    }

    #[test]
    fn test_elf_class_64() {
        let code = MachineCode {
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        assert_eq!(elf[4], ELFCLASS64);
    }

    #[test]
    fn test_elf_machine_x86_64() {
        let code = MachineCode {
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        let machine = u16::from_le_bytes([elf[18], elf[19]]);
        assert_eq!(machine, EM_X86_64);
    }

    #[test]
    fn test_elf_header_size() {
        let code = MachineCode {
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        // ELF header (64) + 1 phdr (56) + 1 byte text = 121
        assert_eq!(elf.len(), ELF64_EHDR_SIZE + ELF64_PHDR_SIZE + 1);
    }

    #[test]
    fn test_elf_entry_point() {
        let code = MachineCode {
            text: vec![0x90],
            data: vec![],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        let entry = u64::from_le_bytes(elf[24..32].try_into().unwrap());
        let expected = VADDR_BASE + (ELF64_EHDR_SIZE + ELF64_PHDR_SIZE) as u64;
        assert_eq!(entry, expected);
    }

    #[test]
    fn test_elf_with_data_section() {
        let code = MachineCode {
            text: vec![0x90],
            data: vec![0x42, 0x43],
            stack_size: 0,
        };
        let elf = build_elf(&code).unwrap();
        // Should have 2 program headers
        let phnum = u16::from_le_bytes([elf[56], elf[57]]);
        assert_eq!(phnum, 2);
        // Total size: header + 2 phdrs + 1 text + 2 data
        assert_eq!(
            elf.len(),
            ELF64_EHDR_SIZE + 2 * ELF64_PHDR_SIZE + 1 + 2
        );
    }

    #[test]
    fn test_elf_determinism() {
        let code = MachineCode {
            text: vec![0xB8, 0x01, 0x00, 0x00, 0x00], // mov eax, 1
            data: vec![],
            stack_size: 0,
        };
        let a = build_elf(&code).unwrap();
        let b = build_elf(&code).unwrap();
        assert_eq!(a, b, "ELF output must be deterministic");
    }
}
