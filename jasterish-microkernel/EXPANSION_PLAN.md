# JMK Expansion Plan — Phase 2 (COMPLETED)

## Goal
Close the critical gaps between the existing v1.0 micro-kernel and a bootable, higher-half, multi-user-space kernel with basic storage.

## Phase 2A: Foundation Fixes ✅
- [x] Add missing per-process globals (`proc_heap_end`, `proc_data_base`, `proc_fork_retval`)
- [x] Fix `sys_fork` to use defined variables (removed `proc_stack_base`, `child_stack_top_minus_size`)
- [x] Fix `sys_brk` to use real per-process heap tracking with physical page allocation
- [x] Add `echo` command to init shell

## Phase 2B: IDT & Exception Handling ✅ (`idt.jstr`)
- [x] IDT descriptor and 256-entry gate array
- [x] `idt_init` — load gates for exceptions 0-31 and IRQs 32-47
- [x] `idt_set_gate` — encode 64-bit IDT entry
- [x] Exception handlers: divide-by-zero, GPF, page-fault, double-fault, and 12 others
- [x] `irq_dispatcher` — route IRQ0→pit_handler, IRQ1→keyboard_handler
- [x] System call entry stub for int 0x80
- [x] Wire `idt_init` into `kernel_main` before `drivers_init`

## Phase 2C: Higher-Half Kernel & TSS ✅
- [x] Save multiboot info pointer from EBX in `_start`
- [x] `pmm_init_multiboot` — parse multiboot2 memory map tag (type 6)
- [x] `vmm_map_kernel_high` — map kernel image to `0xFFFFFFFF80000000`
- [x] TSS structure + `tss_init` with RSP0 stack for ring-3 interrupts
- [x] TSS descriptor added to GDT entry 5, loaded via `ltr`
- [x] `enter_user_mode` — `iretq`-based entry into ring 3

## Phase 2D: Minimal Block Device + VFS ✅ (`disk.jstr`, `vfs.jstr`)
- [x] ATA PIO driver (LBA28 read sectors, write sectors, identify, cache flush)
- [x] Simple RAMFS (in-memory flat filesystem with 16 files × 1KB)
- [x] `sys_open`, `sys_read`, `sys_write`, `sys_close`, `sys_list` syscalls
- [x] Shell commands: `ls`, `cat`, `sync`
- [x] Disk persistence: `vfs_load_from_disk` / `vfs_sync_to_disk` (sectors 1-35)

## Phase 2E: ELF Loader + sys_exec ✅ (`elf.jstr`)
- [x] `elf_validate` — check magic, class, type, machine
- [x] `elf_load` — parse program headers, map LOAD segments via VMM, zero-fill BSS
- [x] `sys_exec` — read ELF from VFS, validate, load, return entry point

## Phase 2F: Copy-on-Write Fork ✅
- [x] `process_fork_cow` — create child slot, copy stack, share page tables
- [x] `sys_fork` delegates to `process_fork_cow`

## Phase 2G: Build Integration ✅
- [x] Add `idt.jstr`, `disk.jstr`, `vfs.jstr`, `elf.jstr` to Makefile `JSTR_SRCS`
- [x] Update README.md with new modules, syscalls, and shell commands

---

## Statistics

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Source files | 7 | 11 | +4 |
| Total lines | ~7,500 | ~13,000 | +5,500 |
| Syscalls | 11 | 17 | +6 |
| Shell commands | 5 | 8 | +3 |
| Modules | 7 | 11 | +4 (IDT, Disk, VFS, ELF) |

---
*Completed: 2026-05-28*

---

## Phase 3: Data, Neural & Sovereign Services (IN PROGRESS - Mac build)

### Goals
- Enable the micro-kernel + JStar compiler to support on-device neurodivergence-aware LLM inference and dataset handling (tying into Morphlex vectors and NeuroDiOS vision).
- Add primitives for loading/processing compiled datasets (e.g. morphlex .db shards) in user-space processes.
- Expand compiler with neuro-specific pattern ops (new JStarInstructions for vector similarity, role-tagged embedding, habit pattern matching).
- Kernel VFS/syscall support for memory-mapping large vector dbs, secure IPC for Neural Engine swarms (rr/ integration).

### Planned
- [ ] New kernel module: `data.jstr` - VFS-backed loader for .db / vector files, simple key-value + vector search.
- [x] `primitives pull` CLI implemented (src/primitives.rs + main.rs wiring). Automates the 8-criteria validation + report + proposal generation against morphlex .db and raw sources. Run on neuro_all.db surfaced train/mask/optimize (lemma-cleaned); integrated as 3 additional ops (now ~12 ops total + 3 flags). See src/jstar/token_map.rs (enum+resolve+keywords+known), parser/type/ir/codegen updates, neuro_patterns.jstr, and primitives_report.md artifacts. This is the "poor primitive CLI" that continues expansion of the governing Architecture primitives.
- [ ] Syscalls: `sys_mmap_data`, `sys_vector_lookup` (fast id== or morph-flag match using morphlex determinism).
- [ ] Compiler (src/jstar + compiler.jstr): add `DatasetCompile`, `NeuroPattern` instructions; extend grammar for "declare neuro habit { ... }".
- [ ] Self-host dataset compiler snippet in JStar once bootstrap stable (use jstar_macos on Mac).
- [ ] Integration test: load neuro_all.db in a user process, run simple pattern recognition via kernel IPC.
- [ ] Update build to include data.jstr; expand shell with `neuro-load`, `pattern-match`.

See `datasets/neurodivergence/` (compiled on this Mac worktree using the morphlex pipeline) and `cargo run -- llm compile-neuro ...` for the data side.

*Started: 2026-06 (Mac-isolated)*

