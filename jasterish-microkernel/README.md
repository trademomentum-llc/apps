# JMK -- Jasterish Micro-Kernel

A complete bare-metal x86-64 micro-kernel written in [Jasterish](https://www.jasterish.com/) (JStar), a system-level language where English words compile directly to x86-64 machine code. JMK demonstrates the feasibility of using natural-language syntax for systems programming, implementing core operating system concepts including process scheduling, virtual memory, and inter-process communication via message passing.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Project Structure](#project-structure)
- [Jasterish Language Primer](#jasterish-language-primer)
- [Building](#building)
- [Running](#running)
- [System Calls](#system-calls)
- [Kernel Shell](#kernel-shell)
- [Design Decisions](#design-decisions)
- [Specification](#specification)

---

## Overview

JMK (Jasterish Micro-Kernel) is a minimal operating system kernel that follows the micro-kernel architecture philosophy: keep the kernel small and implement services as separate processes communicating via IPC. The kernel provides:

| Subsystem | Implementation |
|-----------|---------------|
| **Boot** | Multiboot2-compliant entry point, COM1 serial driver, GDT setup |
| **Memory** | Bitmap-based physical page allocator, 4-level x86-64 page tables, buddy heap allocator |
| **Processes** | 256-slot PCB table, round-robin scheduler with time-slice preemption, context switching |
| **IPC** | Message passing with 64-byte messages, send/receive/notify/broadcast primitives |
| **Syscalls** | 11 system calls via `int 0x80`: exit, fork, yield, send, recv, sleep, getpid, puts, brk, kill, getticks |
| **Drivers** | 8259 PIC remapping, PIT timer at 1000Hz, PS/2 keyboard with scan-code translation |
| **Shell** | Interactive command shell in the init process (PID 1) |

**Total Size**: ~7,500 lines of Jasterish across 7 source files.

---

## Architecture

```
+---------------------------+
|      User Processes       |  <- Shell (PID 1), counter demo, etc.
|  (Ring 3, isolated)       |
+---------------------------+      IPC via send/recv
|  System Call Interface    |  <- int 0x80 gateway
|  (syscall_handler)        |
+---------------------------+
|  IPC Message Passing      |  <- Buffered async messaging
|  (ipc_send / ipc_receive) |
+---------------------------+
|  Process Scheduler        |  <- Round-robin, 10ms quantum
|  (schedule / switch_to)   |
+---------------------------+
|  Memory Management        |  <- PMM bitmap + VMM page tables + heap
|  (pmm_alloc / vmm_map)    |
+---------------------------+
|  Hardware Abstraction     |  <- PIC, PIT, Keyboard, Serial
|  (pic_init / pit_handler) |
+---------------------------+
|  Boot & CPU Setup         |  <- GDT, Multiboot2, stack
|  (_start / gdt_init)      |
+---------------------------+
```

### Memory Layout

```
Physical Address Space:
  0x00000000 - 0x000FFFFF  : Reserved (hardware, BIOS, VGA)
  0x00100000 - 0x00100FFF  : Kernel boot section (Multiboot2 + _start)
  0x00101000 - 0x00FFFFFF  : Kernel code (.text)
  0x01000000 - 0x01FFFFFF  : Kernel read-only data (.rodata)
  0x02000000 - 0x02FFFFFF  : Kernel data (.data)
  0x03000000 - 0x03FFFFFF  : Kernel BSS (.bss)
  0x04000000 - 0x040FFFFF  : Kernel stack (64KB)
  0x05000000 - 0x05FFFFFF  : Kernel heap (1MB)

Higher Half Virtual Address Space (after paging enabled):
  0xFFFF800000000000 - 0xFFFF80007FFFFFFF  : Kernel-mapped physical memory
  0xFFFFFFFF80000000 - 0xFFFFFFFFFFFFFFFF  : Kernel code/data (higher half mirror)
```

---

## Project Structure

```
jasterish-microkernel/
|
|-- boot.jstr        (828 lines)  Boot sequence, serial driver, GDT, panic handler
|-- memory.jstr      (1453 lines) PMM bitmap allocator, VMM page tables, kernel heap
|-- process.jstr     (2394 lines) PCB management, round-robin scheduler, context switching
|-- ipc.jstr         (636 lines)  Message passing: send, receive, notify, broadcast
|-- syscall.jstr     (791 lines)  System call interface: 11 syscalls via int 0x80
|-- drivers.jstr     (703 lines)  PIC, PIT timer, PS/2 keyboard drivers
|-- kernel.jstr      (576 lines)  Main initialization, scheduler loop, init shell
|
|-- linker.ld                    ELF64 linker script for kernel binary layout
|-- Makefile                     Build system: compile, link, QEMU, ISO, debug
|-- README.md                    This file
|-- SPEC.md                      Full kernel specification document
```

---

## Jasterish Language Primer

Jasterish (JStar) is a system-level programming language where English words compile directly to x86-64 machine code. The grammar maps English parts of speech to machine-level semantics.

### Part-of-Speech to Machine Mapping

| Part of Speech | Role | Examples |
|---------------|------|----------|
| **Verb** | Operation / Instruction | `add`, `store`, `jump`, `compare`, `return` |
| **Noun** | Data Declaration | `integer`, `buffer`, `counter`, `result` |
| **Adjective** | Type Modifier | `unsigned`, `static`, `mutable`, `volatile` |
| **Adverb** | Execution Modifier | `immediately`, `conditionally`, `repeatedly` |
| **Preposition** | Addressing Mode | `into`, `from`, `at`, `through` |
| **Determiner** | Scope / Lifetime | `the` (global), `a` (local), `this` (self) |
| **Conjunction** | Control Flow Join | `and` (sequence), `or` (branch), `if` (conditional) |
| **Pronoun** | Register Alias | `it` (accumulator), `that` (last result) |

### Basic Syntax Examples

```jasterish
# Declare global variable
global counter

# Assignment
store 0 into counter

# Arithmetic (result goes into pronoun "it")
add counter 1
store it into counter

# While loop
while less counter 10
    add counter 1
    store it into counter
end

# If conditional
if equal counter 10
    print "Counter reached 10!"
end

# Function definition and return
my_function
    add 2 3
    return it

# Array declaration and access
array 64 message_buffer
store 65 into message_buffer at 0
load message_buffer at 0
```

### Type System

| Jasterish | x86-64 | Range |
|-----------|--------|-------|
| `boolean` | i8 | 0 or 1 |
| `byte` | i8 | -128 to 127 |
| `short` | i16 | -32K to 32K |
| `int` | i32 | default integer |
| `long` | i64 | full 64-bit |
| `float` | f32 | IEEE 754 |
| `double` | f64 | IEEE 754 |
| `char` | u16 | UTF-16 code unit |

---

## Building

### Prerequisites

- **Jasterish Compiler** (`jstar`): The Jasterish-to-x86-64 compiler (bootstrap in Rust)
- **x86_64-elf-ld**: x86-64 ELF linker (from a cross-compiler toolchain)
- **x86_64-elf-objcopy**: Object file conversion utility
- **QEMU** (`qemu-system-x86_64`): For running the kernel
- **GRUB2** (`grub-mkrescue`): For creating bootable ISO images

### Quick Build

```bash
# Build the kernel binary (kernel.bin)
make

# Build with custom Jasterish compiler path
make JASTERISH_COMPILER=/path/to/jstar
```

### Build Outputs

| File | Description |
|------|-------------|
| `jmk.elf` | ELF64 kernel executable with debug symbols |
| `jmk.bin` | Raw binary for Multiboot2 loading |
| `jmk.map` | Linker map showing symbol addresses |

### Build Process

```
boot.jstr    --jstar-->   boot.o
memory.jstr  --jstar-->   memory.o
process.jstr --jstar-->   process.o
ipc.jstr     --jstar-->   ipc.o
syscall.jstr --jstar-->   syscall.o
drivers.jstr --jstar-->   drivers.o
kernel.jstr  --jstar-->   kernel.o
                              |
                              v
                          x86_64-elf-ld
                        (with linker.ld)
                              |
                              v
                          jmk.elf
                              |
                              v
                          objcopy -O binary
                              |
                              v
                          jmk.bin
```

---

## Running

### QEMU (Direct Kernel Boot)

```bash
# Build and run in QEMU
make run

# Run with 1GB RAM and display enabled
make run QEMU_MEM=1024
```

### QEMU (ISO with GRUB2)

```bash
# Create bootable ISO
make iso

# Run ISO in QEMU
make run-iso
```

### QEMU (GDB Remote Debugging)

```bash
# Terminal 1: Launch QEMU with GDB server (waits for connection)
make debug

# Terminal 2: Connect GDB
make gdbinit
gdb -x .gdbinit
(gdb) continue
```

---

## System Calls

User processes invoke kernel services via `int 0x80` with the syscall number in RAX.

| # | Name | Arguments | Description |
|---|------|-----------|-------------|
| 0 | `sys_exit` | `code` | Terminate current process |
| 1 | `sys_fork` | (none) | Create child process as copy |
| 2 | `sys_yield` | (none) | Voluntarily relinquish CPU |
| 3 | `sys_send` | `dest_pid`, `buf`, `size` | Send IPC message (up to 48 bytes) |
| 4 | `sys_recv` | `buf`, `max_size` | Receive IPC message (blocking) |
| 5 | `sys_sleep` | `ticks` | Sleep for N timer ticks (1ms each) |
| 6 | `sys_getpid` | (none) | Get current process ID |
| 7 | `sys_puts` | `str` | Print string to serial console |
| 8 | `sys_brk` | `new_end` | Adjust program break (heap) |
| 9 | `sys_kill` | `pid` | Terminate another process |
| 10 | `sys_getticks` | (none) | Get system uptime in ticks |

### Syscall ABI

```
RAX = syscall number (0-10)
RDI = argument 1
RSI = argument 2
RDX = argument 3
Return value in RAX
Invoke: int 0x80
```

---

## Kernel Shell

The init process (PID 1) provides an interactive shell with the following commands:

| Command | Description |
|---------|-------------|
| `help` | Show available commands |
| `ps` | List running processes (PID, state, parent) |
| `tick` | Show system uptime in timer ticks |
| `mem` | Show memory statistics (total/used/free pages) |
| `quit` | Exit the shell |

---

## Design Decisions

### 1. Micro-Kernel Architecture

Services (file system, network, device drivers) run as user-space processes, not in kernel mode. Only the minimal mechanisms (scheduling, memory, IPC) are in the kernel. This provides:

- **Fault isolation**: A crashing service doesn't bring down the kernel
- **Security**: Services have limited privileges
- **Modularity**: Services can be restarted or upgraded independently

### 2. English-Native Syntax

Using English words as machine instructions makes kernel code self-documenting. Every operation reads like a sentence, reducing the cognitive gap between design intent and implementation.

### 3. Bitmap Physical Memory Manager

A simple first-fit bitmap allocator was chosen for the PMM because:
- Predictable O(n) allocation (n = bitmap size in bytes)
- No external fragmentation
- Easy to debug and verify correctness

### 4. Round-Robin Scheduling

A simple round-robin scheduler with a 10ms time quantum provides:
- Fair CPU distribution among equal-priority processes
- Predictable latency
- Simplicity for a teaching/reference kernel

### 5. Message-Passing IPC

Synchronous message passing (with async buffering) was chosen over shared memory because:
- No cache coherency issues between processes
- Natural synchronization (receiver blocks until message arrives)
- Easier to secure and validate

---

## Specification

The complete kernel specification, including all data structures, function signatures, memory layouts, and algorithms, is documented in `SPEC.md`.

---

## License

MIT License -- See the Jasterish project at https://www.jasterish.com/

---

*"Natural Language is Machine Code."*
