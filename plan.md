# Jasterish Micro-Kernel — Implementation Plan

## Overview
Create a complete x86-64 micro-kernel using the Jasterish (JStar) natural language compiler. The kernel follows the micro-kernel architecture: minimal core with services running as processes communicating via IPC.

## Jasterish Language Profile
- **Syntax**: English words -> x86-64 machine code
- **Types**: boolean, byte, short, int, long, float, double, char (Java primitives)
- **Semantics**: Grammar = Architecture (Verb=Op, Noun=Data, Prep=Addressing, etc.)
- **Features**: arrays, for/while loops, if conditionals, hash function, print
- **Compiler**: Bootstrap in Rust, self-hosting target, zero external dependencies

## Stage 1 — SPEC Creation (Main Agent)
- Write comprehensive SPEC.md with full micro-kernel architecture
- Define all data structures, interfaces, memory layout, and algorithms
- Define Jasterish coding patterns for kernel development

## Stage 2 — Parallel Implementation (Subagents)
Group into 4 parallel workstreams:

### Module A: Boot & Core Initialization
- Multiboot2 header for GRUB compatibility
- Entry point (_start), early boot assembly
- GDT setup (x86-64 long mode)
- IDT setup with interrupt handlers
- Serial COM1 driver (for early debugging output)

### Module B: Memory Management
- Physical memory manager (bitmap-based frame allocation)
- Page table management (PML4, PDP, PD, PT)
- Virtual memory mapper
- Kernel heap allocator (buddy allocator)

### Module C: Process Management + Scheduler
- Process Control Block (PCB) structure
- Context switching (save/restore registers)
- Round-robin scheduler with time slicing
- Process creation and termination primitives
- Idle process

### Module D: IPC System Calls + Drivers
- Message passing (send/receive with message buffers)
- System call interface (int 0x80 or syscall/sysret)
- Timer driver (PIT - Programmable Interval Timer)
- Keyboard driver (PS/2 scan code handling)
- Kernel main loop tying everything together

## Stage 3 — Integration & Testing (Main Agent)
- Merge all modules
- Write linker script for kernel binary layout
- Write build system (Makefile)
- Test with QEMU
- Fix integration issues

## Stage 4 — Documentation
- Kernel architecture document
- Build and run instructions
- System call reference

## Deliverables
- `kernel.jstr` — Main kernel source in Jasterish
- `boot.jstr` — Boot and initialization
- `memory.jstr` — Memory management subsystem
- `process.jstr` — Process management and scheduler
- `ipc.jstr` — Inter-process communication
- `drivers.jstr` — Hardware drivers (timer, keyboard, serial)
- `syscall.jstr` — System call interface
- `linker.ld` — Linker script
- `Makefile` — Build system
- `README.md` — Documentation

## CodeQL Command-Execution Hardening (2026-09-02)

### Scope

Harden the six reported Python process-launch sites in
`scripts/generate_release_provenance.py`, `scripts/jasterish_orchestrator.py`,
and `scripts/jasterish_regression.py`, and restore the pinned CodeQL workflow
needed to verify closure, without changing intended build, regression, or
provenance behavior.

### Security Requirements

1. Permit only explicitly supported architecture identifiers.
2. Resolve executable names to concrete files and reject missing,
   non-executable, or unapproved compiler overrides.
3. Confine internal scripts and corpus paths to their expected repository
   roots before process launch.
4. Keep subprocess arguments as arrays with shell execution disabled.
5. Fail closed before launching a process when validation fails.
6. Pin every third-party CodeQL workflow action to a verified full commit SHA.

### Status

| Step | State | Evidence |
|---|---|---|
| Inspect all six CodeQL data flows | Complete | Alerts 171 through 176 traced from CLI or environment input to subprocess sinks |
| Implement boundary validation | Complete | Allowlisted architectures, executables, Git operations, scripts, corpora, and external signing-key placement |
| Restore CodeQL execution | Complete | Workflow actions pinned to verified full commit SHAs |
| Add malicious-input regression tests | Complete | Thirteen standard-library security tests added |
| Run targeted tests | Complete | Thirteen tests passed; compiler integration completed with expected host-architecture skips |
| Run Semgrep and Gitleaks | Complete | Semgrep and current-tree Gitleaks passed; historical matches documented for separate classification |
| Review final diff and residual risk | Complete | Remote CodeQL closure remains pending until publication |

### Verification Plan

- Run focused unit tests for accepted commands and rejected architecture,
  executable, corpus, and path inputs.
- Run the complete Python test suite relevant to the three utilities.
- Run Semgrep against all modified Python files.
- Run Gitleaks against the working tree and Git history with redacted output.
- Treat GitHub CodeQL as the authoritative closure check after publication;
  local checks cannot close remote alert records.
