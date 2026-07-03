/*
 * ============================================================================
 * linker.ld -- Jasterish Micro-Kernel Linker Script
 * ============================================================================
 *
 * This linker script defines the memory layout of the JMK kernel binary.
 * The kernel is linked as an ELF64 executable and loaded by a Multiboot2
 * compliant boot loader (e.g., GRUB2) at the address specified in the
 * Multiboot2 header.
 *
 * Memory Layout:
 *   0x00100000 - 0x00100FFF : Multiboot2 header + early boot code (.boot)
 *   0x00101000 - 0x00FFFFFF : Kernel text (code) section
 *   0x01000000 - 0x01FFFFFF : Kernel read-only data (.rodata)
 *   0x02000000 - 0x02FFFFFF : Kernel data (.data)
 *   0x03000000 - 0x03FFFFFF : Kernel BSS (uninitialized data)
 *   0x04000000 - 0x04FFFFFF : Kernel stack
 *   0x05000000 - 0x05FFFFFF : Kernel heap
 *
 * The kernel runs in the higher half (above 0xFFFFFFFF80000000) after
 * paging is enabled by the VMM initialization in memory.jstr.
 * The identity mapping in the boot page tables bridges the gap between
 * the physical load address and the higher-half virtual address.
 *
 * OUTPUT FORMAT: ELF64-x86-64
 * ENTRY POINT: _start (defined in boot.jstr)
 * ============================================================================
 */

/* Output format: 64-bit ELF for x86-64 */
OUTPUT_FORMAT(elf64-x86-64)

/* Target architecture */
OUTPUT_ARCH(i386:x86-64)

/* Entry point symbol -- must match the label in boot.jstr */
ENTRY(_start)

/* ============================================================================
 * Memory Regions
 * ============================================================================
 * Define the physical memory regions available to the kernel.
 * These correspond to the layout described above.
 */
MEMORY
{
    /* Low memory region: boot code and kernel image (physical addresses) */
    RAM (rwx) : ORIGIN = 0x00100000, LENGTH = 0x00F00000   /* 1MB - 16MB */
}

/* ============================================================================
 * Section Layout
 * ============================================================================
 * Define the order and placement of sections in the output binary.
 * Sections are aligned to page boundaries (4KB) where appropriate.
 */

SECTIONS
{
    /* ------------------------------------------------------------------------
     * Boot Section
     * ------------------------------------------------------------------------
     * The Multiboot2 header and early boot code must be within the first
     * 32KB of the kernel image. We place them at the very beginning.
     * This section contains:
     *   - Multiboot2 header (12 bytes minimum)
     *   - _start entry point
     *   - Early initialization code (stack setup, CLI)
     */
    . = 0x00100000;

    .boot : AT(ADDR(.boot))
    {
        /* Multiboot2 header must be 64-bit aligned */
        . = ALIGN(8);

        /* The boot section starts with the Multiboot2 header */
        *(.multiboot)
        *(.multiboot2)

        /* Early boot code: _start, serial_init, gdt_init */
        *(.boot)
        *(.boot.*)

        /* Ensure section ends on a 4KB boundary for page alignment */
        . = ALIGN(4096);
    } > RAM

    /* Record the end of the boot section */
    __boot_end = .;

    /* ------------------------------------------------------------------------
     * Text Section (Code)
     * ------------------------------------------------------------------------
     * Contains all executable code from all modules:
     *   - boot.jstr: _start, serial_putc, serial_puts, gdt_init, panic, etc.
     *   - memory.jstr: pmm_init, pmm_alloc, vmm_init, kmalloc, etc.
     *   - process.jstr: scheduler_init, schedule, context_switch, etc.
     *   - ipc.jstr: ipc_send, ipc_receive, ipc_notify, etc.
     *   - syscall.jstr: syscall_handler, sys_exit, sys_fork, etc.
     *   - drivers.jstr: pic_init, pit_init, keyboard_handler, etc.
     *   - kernel.jstr: kernel_main, scheduler_loop, init_function, etc.
     */
    . = 0x00101000;

    .text : AT(ADDR(.text))
    {
        . = ALIGN(4096);
        __text_start = .;

        /* All .text sections from all object files */
        *(.text)
        *(.text.*)
        *(.text.unlikely)
        *(.text.startup)

        /* Keep entry point and critical symbols */
        KEEP(*(.text._start))
        KEEP(*(.text.kernel_main))

        . = ALIGN(4096);
        __text_end = .;
    } > RAM

    /* ------------------------------------------------------------------------
     * Read-Only Data Section
     * ------------------------------------------------------------------------
     * Contains constant data that should not be modified:
     *   - String literals (kernel version, banner, error messages)
     *   - Read-only lookup tables (scancode_to_ascii table)
     *   - Constant configuration values
     */
    .rodata : AT(ADDR(.rodata))
    {
        . = ALIGN(4096);
        __rodata_start = .;

        *(.rodata)
        *(.rodata.*)
        *(.rodata.str*)

        /* Keep important read-only data */
        KEEP(*(.rodata.kernel_version))
        KEEP(*(.rodata.scancode_table))

        . = ALIGN(4096);
        __rodata_end = .;
    } > RAM

    /* ------------------------------------------------------------------------
     * Data Section
     * ------------------------------------------------------------------------
     * Contains initialized global and static variables:
     *   - Global constants (COM1_PORT, PAGE_SIZE, MAX_PROCS, etc.)
     *   - Global state variables (current_process, tick_count, etc.)
     *   - Initialized arrays (gdt_entries, ready_queue, scancode_table, etc.)
     */
    .data : AT(ADDR(.data))
    {
        . = ALIGN(4096);
        __data_start = .;

        *(.data)
        *(.data.*)

        /* Keep all global data structures */
        KEEP(*(.data.gdt_entries))
        KEEP(*(.data.phys_bitmap))
        KEEP(*(.data.proc_*))
        KEEP(*(.data.keyboard_buffer))
        KEEP(*(.data.ready_queue))

        . = ALIGN(4096);
        __data_end = .;
    } > RAM

    /* ------------------------------------------------------------------------
     * BSS Section (Uninitialized Data)
     * ------------------------------------------------------------------------
     * Contains uninitialized global and static variables that the
     * kernel startup must zero before use:
     *   - Uninitialized arrays
     *   - Zero-initialized globals
     *
     * The boot code must clear this region before calling kernel_main.
     */
    .bss : AT(ADDR(.bss))
    {
        . = ALIGN(4096);
        __bss_start = .;

        *(.bss)
        *(.bss.*)
        *(COMMON)

        . = ALIGN(4096);
        __bss_end = .;
    } > RAM

    /* ------------------------------------------------------------------------
     * Stack Section
     * ------------------------------------------------------------------------
     * The kernel stack grows downward from __stack_top.
     * Size: 64KB for kernel stack (sufficient for interrupt nesting)
     */
    . = ALIGN(4096);
    __stack_bottom = .;
    . += 65536;                          /* 64KB kernel stack */
    __stack_top = .;

    /* ------------------------------------------------------------------------
     * Heap Section
     * ------------------------------------------------------------------------
     * The kernel heap starts after the stack and grows upward.
     * Size: 1MB initial heap (expandable via VMM)
     */
    . = ALIGN(4096);
    __heap_start = .;
    . += 1048576;                       /* 1MB kernel heap */
    __heap_end = .;

    /* =========================================================================
     * Symbol Exports for Kernel Code
     * =========================================================================
     * These symbols are referenced by the Jasterish code in the kernel
     * modules. The linker makes them available as global addresses.
     */

    /* Kernel image bounds */
    __kernel_start = 0x00100000;
    __kernel_end = .;
    __kernel_size = __kernel_end - __kernel_start;

    /* Section size macros */
    __text_size = __text_end - __text_start;
    __rodata_size = __rodata_end - __rodata_start;
    __data_size = __data_end - __data_start;
    __bss_size = __bss_end - __bss_start;

    /* Higher half mapping offset (added by VMM at runtime) */
    __higher_half_offset = 0xFFFFFFFF80000000;

    /* Physical memory available to PMM (detected at boot) */
    __phys_mem_start = 0x00100000;      /* 1MB (after reserved low memory) */
    __phys_mem_end = 0x20000000;        /* 512MB default (configurable) */

    /* Discard unnecessary sections to reduce binary size */
    /DISCARD/ :
    {
        *(.comment)
        *(.note)
        *(.note.*)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.gnu.*)
    }
}
