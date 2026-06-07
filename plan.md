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
