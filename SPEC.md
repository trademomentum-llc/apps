# SPEC.md — Jasterish Micro-Kernel (JMK)

## 1. Architecture Overview

JMK (Jasterish Micro-Kernel) is a bare-metal x86-64 operating system kernel written in Jasterish. It follows the micro-kernel design philosophy: keep the kernel minimal and implement services as user-space processes communicating via IPC.

### 1.1 Memory Layout (Higher Half Kernel)
```
0xFFFF800000000000 - 0xFFFF80007FFFFFFF  : Kernel mapped physical memory (128MB)
0xFFFFFFFF80000000 - 0xFFFFFFFFFFFFFFFF  : Kernel text/data/bss
0x0000000000000000 - 0x00007FFFFFFFFFFF  : User-space address space
```

### 1.2 Kernel Components
```
+----------------------------+
|       User Processes       |
+----------------------------+
|  IPC (Message Passing)     |
+----------------------------+
|  Scheduler (Round-Robin)   |
+----------------------------+
|  Process Manager           |
+----------------------------+
|  Memory Manager (VMM/PMM)  |
+----------------------------+
|  Interrupt Handlers (IDT)  |
+----------------------------+
|  Boot / GDT / Serial       |
+----------------------------+
```

## 2. Data Structures

### 2.1 Process Control Block (PCB)
```
struct pcb:
  int pid              # Process ID
  int state            # 0=UNUSED, 1=READY, 2=RUNNING, 3=BLOCKED, 4=ZOMBIE
  long rsp             # Saved stack pointer
  long rip             # Saved instruction pointer
  long cr3             # Page table root
  int priority         # Scheduling priority (1-10)
  int time_slice       # Remaining time slice (ticks)
  int parent           # Parent process ID
  int exit_code        # Exit status
  array 64 message_buf # IPC message buffer (64 bytes)
  int msg_pending      # Non-zero if message waiting
  int msg_source       # Source PID of pending message
  int msg_size         # Size of pending message
```

### 2.2 Memory Block Header (Heap)
```
struct block:
  int size             # Block size in bytes (positive=free, negative=used)
  int prev             # Offset to previous block (or 0)
```

### 2.3 Message Structure (IPC)
```
struct message:
  int source           # Sender PID
  int dest             # Receiver PID
  int type             # Message type
  int size             # Payload size
  array 48 payload     # Message payload (48 bytes)
```

### 2.4 Interrupt Stack Frame
```
struct frame:
  long rip             # Instruction pointer
  long cs              # Code segment
  long rflags          # Flags
  long rsp             # Stack pointer
  long ss              # Stack segment
```

## 3. Global Kernel State

```
global kernel_pml4          # Kernel page table root
global phys_bitmap          # Physical memory bitmap
global phys_bitmap_size     # Bitmap size in bytes
global phys_total_pages     # Total physical pages
global phys_used_pages      # Currently used pages
global heap_start           # Start of kernel heap
global heap_end             # End of kernel heap
global current_process      # Index of currently running process
global process_table        # Array of MAX_PROCS (256) PCB entries
global process_count        # Number of active processes
global next_pid             # Next available PID
global tick_count           # Timer tick counter
global ready_queue_head     # Head of ready queue
global ready_queue_tail     # Tail of ready queue
global keyboard_buffer      # Keyboard input buffer (ring buffer)
global keyboard_head        # Read position
global keyboard_tail        # Write position
```

## 4. Module Specifications

### 4.1 Boot Module (boot.jstr)

**Functions:**
```
entry _start:
  - Multiboot2 header (magic + architecture + length + checksum)
  - Early setup: stack pointer, disable interrupts
  - Call kernel_main

function serial_init:
  - Configure COM1 port (0x3F8)
  - Set baud rate to 115200
  - 8 data bits, no parity, 1 stop bit
  
function serial_putc(char c):
  - Wait for transmitter ready
  - Output character to COM1 data port
  
function serial_puts(string s):
  - Loop calling serial_putc for each character
  
function serial_print_hex(long value):
  - Print 64-bit value as hexadecimal
```

**Constants:**
```
COM1_PORT = 0x3F8
SERIAL_DLL = 0x00  # Divisor latch low
SERIAL_DLH = 0x01  # Divisor latch high
SERIAL_LCR = 0x03  # Line control
SERIAL_LSR = 0x05  # Line status
MAX_PROCS = 256
PAGE_SIZE = 4096
STACK_SIZE = 8192
HEAP_SIZE = 1048576  # 1MB kernel heap
KERNEL_VIRT_BASE = 0xFFFFFFFF80000000
```

### 4.2 Memory Management (memory.jstr)

**Functions:**
```
function pmm_init(long mem_lower, long mem_upper):
  - Initialize physical memory bitmap
  - Mark kernel memory as used
  - Mark first 1MB as used (hardware reserved)
  
function pmm_alloc:
  - Find first free bit in bitmap
  - Mark as used, return physical address
  - Returns 0 if out of memory
  
function pmm_free(long page_addr):
  - Clear bit in bitmap for given page
  
function vmm_init:
  - Create kernel PML4 table
  - Identity map first 1GB
  - Map kernel to higher half
  - Enable paging (set CR3, CR0.PG)
  
function vmm_map_page(long virt, long phys, int flags):
  - Walk page tables, create if needed
  - Set PTE with given flags
  
function vmm_unmap_page(long virt):
  - Clear PTE, invalidate TLB
  
function heap_init:
  - Initialize buddy allocator
  - Create initial free block
  
function kmalloc(int size):
  - Find best-fit block
  - Split if needed
  - Return virtual address
  
function kfree(long addr):
  - Mark block as free
  - Coalesce with adjacent free blocks
```

### 4.3 Process Management (process.jstr)

**Functions:**
```
function scheduler_init:
  - Initialize process table (all UNUSED)
  - Create idle process (PID 0)
  - Set current_process = 0
  - Initialize ready queue (empty)
  
function process_create(long entry_point, int priority):
  - Find unused PCB slot
  - Allocate stack page
  - Setup initial context (RIP, RSP, CR3)
  - Set state = READY
  - Add to ready queue
  - Return PID
  
function process_exit(int code):
  - Set state = ZOMBIE
  - Store exit code
  - Notify parent if waiting
  - Schedule next process
  
function process_kill(int pid):
  - Find PCB by PID
  - Free stack and page table
  - Mark as UNUSED
  
function context_switch:
  - Save current context (RSP, RIP, registers)
  - Select next process from ready queue
  - Restore next context
  - Update CR3, return
  
function schedule:
  - Decrement current time slice
  - If time slice expired: move to tail of ready queue
  - Pick head of ready queue as next
  - Call context_switch if different
  
function yield:
  - Move current to end of ready queue
  - Trigger schedule
  
function sleep(int ticks):
  - Set state = BLOCKED
  - Store wake-up tick count
  - Schedule next process
```

**Process States:**
```
PROC_UNUSED  = 0
PROC_READY   = 1
PROC_RUNNING = 2
PROC_BLOCKED = 3
PROC_ZOMBIE  = 4
```

### 4.4 IPC System (ipc.jstr)

**Functions:**
```
function ipc_send(int dest_pid, array buf, int size):
  - Validate destination process exists
  - If receiver is BLOCKED waiting for message:
    - Copy message directly
    - Wake receiver (set READY, add to queue)
  - Else:
    - Copy to receiver's message buffer
    - Set msg_pending flag
  - Block sender until reply (optional sync)
  
function ipc_receive(array buf, int max_size):
  - Check if message pending
  - If yes: copy to buf, clear flag
  - If no: BLOCK current process, schedule
  - Return message size
  
function ipc_reply(int source_pid, array buf, int size):
  - Send reply to original sender
  - Wake sender if blocked
  
function ipc_notify(int dest_pid, int event):
  - Send lightweight notification (no payload)
  - Non-blocking
```

### 4.5 System Calls (syscall.jstr)

**System Call Numbers:**
```
SYS_EXIT    = 0
SYS_FORK    = 1
SYS_YIELD   = 2
SYS_SEND    = 3
SYS_RECV    = 4
SYS_SLEEP   = 5
SYS_GETPID  = 6
SYS_PUTS    = 7
SYS_BRK     = 8
```

**System Call Handler:**
```
function syscall_handler:
  - Save all registers
  - Get syscall number from RAX
  - Get arguments from RDI, RSI, RDX, R10, R8, R9
  - Dispatch based on syscall number
  - Store return value in RAX
  - Restore registers
  - IRETQ
  
function sys_exit(int code):
  - Call process_exit
  
function sys_fork:
  - Create new process as copy of current
  - Return 0 to child, PID to parent
  
function sys_yield:
  - Call yield
  
function sys_send(int dest, array buf, int size):
  - Call ipc_send
  
function sys_recv(array buf, int max_size):
  - Call ipc_receive
  
function sys_sleep(int ticks):
  - Call sleep
  
function sys_getpid:
  - Return current_process PID
  
function sys_puts(array str):
  - Print string via serial
  
function sys_brk(long new_end):
  - Adjust program break (heap end)
```

### 4.6 Drivers (drivers.jstr)

**Timer (PIT):**
```
function pit_init:
  - Configure Channel 0
  - Set frequency to 1000Hz (1ms ticks)
  - Enable IRQ0 in PIC
  
function pit_handler:
  - Increment tick_count
  - Check for processes that should wake up
  - Call schedule() every 10 ticks (10ms quantum)
  - Send EOI to PIC
```

**Keyboard (PS/2):**
```
function keyboard_init:
  - Enable IRQ1 in PIC
  
function keyboard_handler:
  - Read scan code from port 0x60
  - Convert to ASCII if make code
  - Add to ring buffer
  - Send EOI to PIC
  
function keyboard_getc:
  - Return character from buffer (or 0 if empty)
```

**PIC (8259):**
```
function pic_init:
  - Remap PIC vectors to 0x20-0x2F
  - Mask all interrupts except timer and keyboard
  
function pic_eoi(int irq):
  - Send End-of-Interrupt signal
```

**Interrupt Handlers:**
```
function idt_init:
  - Create IDT with 256 entries
  - Set handler for each vector
  - Load IDT with LIDT
  
function exception_handler:
  - Print error message and register dump
  - Halt system
  
function irq_handler:
  - Get IRQ number
  - Dispatch to specific handler
  - Send EOI
```

### 4.7 Kernel Main (kernel.jstr)

```
function kernel_main:
  - serial_init
  - print "JMK v1.0 — Jasterish Micro-Kernel"
  - gdt_init
  - idt_init
  - pic_init
  - pmm_init (from multiboot memory map)
  - vmm_init
  - heap_init
  - scheduler_init
  - pit_init
  - keyboard_init
  - Create init process (user-space shell)
  - Enable interrupts
  - Enter scheduler loop (never returns)
```

## 5. Jasterish Implementation Patterns

### 5.1 Inline Assembly Patterns
For operations Jasterish cannot express, use inline raw bytes:
```
# HLT instruction
store 0xF4 into port 0x00  # hlt opcode

# OUTB instruction
store value into port addr  # maps to outb

# INB instruction
load from port addr         # maps to inb
```

### 5.2 Memory Access
```
# Read from memory address
load address into variable

# Write to memory address
store value into address

# Read with offset
load base at offset into variable

# Write with offset
store value into base at offset
```

### 5.3 Interrupt Safety
```
# Disable interrupts
store 0x00 into interrupt_state

# Enable interrupts  
store 0x01 into interrupt_state
```

## 6. Build System

### 6.1 Compilation Flow
```
.jstr source files
    |
    v
Jasterish Compiler (jstar)
    |
    v
ELF64 object files (.o)
    |
    v
LD (x86_64-elf-ld) with linker.ld
    |
    v
Kernel binary (kernel.bin)
    |
    v
GRUB2 multiboot2 (boot via QEMU)
```

### 6.2 Makefile Targets
```
all: Build kernel.bin
run: Launch in QEMU with -kernel
iso: Create bootable ISO with GRUB
debug: Launch in QEMU with GDB server
clean: Remove build artifacts
```

## 7. Testing Strategy

1. **Unit tests**: Test each module independently
2. **Integration test**: Boot kernel in QEMU, verify serial output
3. **Stress test**: Create multiple processes, verify scheduling fairness
4. **IPC test**: Send messages between processes, verify delivery
5. **Stability test**: Run for extended period, check for crashes

## 8. Deliverables Checklist

- [ ] boot.jstr — Boot sequence + serial driver
- [ ] memory.jstr — PMM + VMM + heap allocator
- [ ] process.jstr — PCB + scheduler + context switch
- [ ] ipc.jstr — Message passing primitives
- [ ] syscall.jstr — System call interface + dispatch
- [ ] drivers.jstr — PIT timer + keyboard + PIC
- [ ] kernel.jstr — Main kernel initialization
- [ ] linker.ld — Kernel binary layout
- [ ] Makefile — Build automation
- [ ] README.md — Documentation
