# ============================================================================
# Makefile -- Jasterish Micro-Kernel Build System
# ============================================================================
#
# This Makefile compiles the JMK (Jasterish Micro-Kernel) from Jasterish
# source files (.jstr) into a bootable kernel binary.
#
# Build Chain:
#   1. Jasterish Compiler (jstar): .jstr -> .o (ELF64 object files)
#   2. x86-64 ELF Linker (ld):     .o files -> kernel.elf
#   3. Object Copy (objcopy):       kernel.elf -> kernel.bin (raw binary)
#
# Tools Required:
#   - jstar          : Jasterish compiler (bootstrap or self-hosted)
#   - x86_64-elf-ld  : x86-64 ELF linker (from binutils or cross-compiler)
#   - x86_64-elf-objcopy : Object file conversion utility
#   - grub-mkrescue  : GRUB2 rescue image creator (for ISO target)
#   - qemu-system-x86_64 : QEMU emulator (for run/debug targets)
#   - gdb            : GNU debugger (for debug target)
#
# Usage:
#   make          -- Build kernel.bin
#   make all      -- Full build (kernel.bin + kernel.elf)
#   make run      -- Build and launch in QEMU
#   make iso      -- Create bootable ISO with GRUB2
#   make debug    -- Launch in QEMU with GDB server
#   make test     -- Run kernel self-test in QEMU
#   make clean    -- Remove all build artifacts
#   make count    -- Count total lines of Jasterish source code
# ============================================================================

# ----------------------------------------------------------------------------
# Configuration
# ----------------------------------------------------------------------------

# Target architecture
ARCH            := x86_64
TARGET          := $(ARCH)-elf

# Kernel name
KERNEL_NAME     := jmk
KERNEL_ELF      := $(KERNEL_NAME).elf
KERNEL_BIN      := $(KERNEL_NAME).bin
KERNEL_ISO      := $(KERNEL_NAME).iso

# Compiler and linker tools
# Use JASTERISH_COMPILER environment variable, or default to "jstar"
JSTAR           := $(JASTERISH_COMPILER:jstar)

# Cross-compiler toolchain (adjust prefix if using a cross-compiler)
CROSS_PREFIX    := $(CROSS_COMPILE:x86_64-elf-)
CC              := $(CROSS_PREFIX)gcc
LD              := $(CROSS_PREFIX)ld
OBJCOPY         := $(CROSS_PREFIX)objcopy
OBJDUMP         := $(CROSS_PREFIX)objdump

# Emulator and debugger
QEMU            := qemu-system-x86_64
GDB             := gdb

# GRUB2 tools
GRUB_MKRESCUE   := grub-mkrescue

# ----------------------------------------------------------------------------
# Source Files
# ----------------------------------------------------------------------------
# All Jasterish source files for the kernel.
# Order matters: boot must be first (contains _start entry point).

JSTR_SRCS       := boot.jstr \
                   memory.jstr \
                   elf.jstr \
                   process.jstr \
                   ipc.jstr \
                   syscall.jstr \
                   drivers.jstr \
                   idt.jstr \
                   disk.jstr \
                   vfs.jstr \
                   kernel.jstr

# Object files (one .o per .jstr)
OBJS            := $(JSTR_SRCS:.jstr=.o)

# Dependency files (for incremental builds)
DEPS            := $(OBJS:.o=.d)

# ----------------------------------------------------------------------------
# Compiler Flags
# ----------------------------------------------------------------------------

# Jasterish compiler flags
#   --target x86_64     : Generate x86-64 machine code
#   --format elf        : Output ELF64 object files
#   --optimize 2        : Optimization level 2 (default)
#   --debug             : Include debug information
#   --warn-all          : Enable all warnings
#   --output            : Specify output file
JSTARFLAGS      := --target x86_64 \
                   --format elf \
                   --optimize 2 \
                   --warn-all

# Preprocessor flags (for C header compatibility if needed)
CPPFLAGS        := -I. -DJMK_KERNEL \
                   -DARCH_$(shell echo $(ARCH) | tr '[:lower:]' '[:upper:]')

# Linker flags
#   -T linker.ld        : Use our custom linker script
#   -nostdlib           : Do not link against standard libraries
#   -static             : Static linking only
#   -z max-page-size=4096 : Set maximum page size to 4KB
LDFLAGS         := -T linker.ld \
                   -nostdlib \
                   -static \
                   -z max-page-size=4096 \
                   --no-relax

# Linker map file (for debugging memory layout)
LDMAP           := $(KERNEL_NAME).map

# ----------------------------------------------------------------------------
# QEMU Configuration
# ----------------------------------------------------------------------------

# QEMU machine settings
QEMU_MACHINE    := q35
QEMU_CPU        := qemu64
QEMU_MEM        := 512

# QEMU common flags
QEMU_BASE       := -machine $(QEMU_MACHINE) \
                   -cpu $(QEMU_CPU) \
                   -m $(QEMU_MEM) \
                   -serial stdio \
                   -no-reboot \
                   -no-shutdown

# QEMU display (disable for headless operation)
QEMU_DISPLAY    := -display none

# QEMU debug flags (for GDB remote debugging)
QEMU_DEBUG      := -s -S

# QEMU Multiboot2 boot (direct kernel loading)
QEMU_KERNEL     := -kernel $(KERNEL_BIN)

# QEMU ISO boot
QEMU_ISO_BOOT   := -cdrom $(KERNEL_ISO) -boot d

# Combined QEMU run flags
QEMU_RUN_FLAGS  := $(QEMU_BASE) $(QEMU_DISPLAY) $(QEMU_KERNEL)
QEMU_DEBUG_FLAGS:= $(QEMU_BASE) $(QEMU_DISPLAY) $(QEMU_KERNEL) $(QEMU_DEBUG)
QEMU_ISO_FLAGS  := $(QEMU_BASE) $(QEMU_DISPLAY) $(QEMU_ISO_BOOT)

# ----------------------------------------------------------------------------
# GDB Configuration
# ----------------------------------------------------------------------------

# GDB initialization script
GDB_INIT        := .gdbinit

# GDB remote target
GDB_TARGET      := target remote localhost:1234

# GDB symbols
GDB_SYMBOLS     := file $(KERNEL_ELF)

# ----------------------------------------------------------------------------
# Default Target
# ----------------------------------------------------------------------------

.PHONY: all build run iso debug test selftest clean count help

# Default: build the kernel binary
all: build

# Full build: ELF + binary
build: $(KERNEL_BIN) $(KERNEL_ELF)
	@echo ""
	@echo "========================================"
	@echo "  Build Complete: $(KERNEL_NAME)"
	@echo "========================================"
	@echo "  ELF:   $(KERNEL_ELF)"
	@echo "  BIN:   $(KERNEL_BIN)"
	@echo "  Size:  $$(wc -c < $(KERNEL_BIN)) bytes"
	@echo "========================================"

# ----------------------------------------------------------------------------
# Compilation Rules
# ----------------------------------------------------------------------------

# Rule: .jstr -> .o (compile Jasterish source to ELF object)
%.o: %.jstr
	@echo "  JSTAR   $<"
	@$(JSTAR) $(JSTARFLAGS) --input $< --output $@

# Rule: Link all objects into kernel ELF
$(KERNEL_ELF): $(OBJS) linker.ld
	@echo "  LD      $@"
	@$(LD) $(LDFLAGS) -o $@ $(OBJS) -Map=$(LDMAP)

# Rule: Convert ELF to raw binary (for Multiboot2 loading)
$(KERNEL_BIN): $(KERNEL_ELF)
	@echo "  OBJCOPY $@"
	@$(OBJCOPY) -O binary $< $@

# Include dependency files if they exist
-include $(DEPS)

# ----------------------------------------------------------------------------
# QEMU Targets
# ----------------------------------------------------------------------------

# Run the kernel in QEMU (normal mode)
run: build
	@echo "========================================"
	@echo "  Launching JMK in QEMU..."
	@echo "========================================"
	@$(QEMU) $(QEMU_RUN_FLAGS)

# Run with ISO (GRUB2 bootloader)
run-iso: iso
	@echo "========================================"
	@echo "  Launching JMK ISO in QEMU..."
	@echo "========================================"
	@$(QEMU) $(QEMU_ISO_FLAGS)

# ----------------------------------------------------------------------------
# Debug Targets
# ----------------------------------------------------------------------------

# Launch QEMU with GDB server (waits for debugger connection)
debug: build
	@echo "========================================"
	@echo "  QEMU waiting for GDB on port 1234..."
	@echo "  Run: gdb -x $(GDB_INIT)"
	@echo "========================================"
	@$(QEMU) $(QEMU_DEBUG_FLAGS)

# Create GDB initialization script
gdbinit:
	@echo "set architecture i386:x86-64" > $(GDB_INIT)
	@echo "$(GDB_SYMBOLS)" >> $(GDB_INIT)
	@echo "$(GDB_TARGET)" >> $(GDB_INIT)
	@echo "set disassemble-next-line on" >> $(GDB_INIT)
	@echo "layout asm" >> $(GDB_INIT)
	@echo "layout regs" >> $(GDB_INIT)
	@echo "Created $(GDB_INIT)"

# ----------------------------------------------------------------------------
# ISO Target (GRUB2 Bootable Image)
# ----------------------------------------------------------------------------

# Create a bootable ISO using GRUB2
iso: build
	@echo "========================================"
	@echo "  Creating bootable ISO..."
	@echo "========================================"
	@mkdir -p iso/boot/grub
	@cp $(KERNEL_BIN) iso/boot/$(KERNEL_NAME)
	@echo "menuentry \"Jasterish Micro-Kernel\" {" > iso/boot/grub/grub.cfg
	@echo "    multiboot2 /boot/$(KERNEL_NAME)" >> iso/boot/grub/grub.cfg
	@echo "    boot" >> iso/boot/grub/grub.cfg
	@echo "}" >> iso/boot/grub/grub.cfg
	@$(GRUB_MKRESCUE) -o $(KERNEL_ISO) iso/ 2>/dev/null
	@echo "  ISO: $(KERNEL_ISO)"
	@echo "========================================"

# ----------------------------------------------------------------------------
# Test Target
# ----------------------------------------------------------------------------

# Run kernel self-test in QEMU (non-interactive, capture output)
test: build
	@echo "========================================"
	@echo "  Running JMK self-test in QEMU..."
	@echo "========================================"
	@$(QEMU) $(QEMU_RUN_FLAGS) | tee test_output.log || true
	@echo ""
	@echo "Test output saved to: test_output.log"

# Quick line count of all Jasterish source files
count:
	@echo "========================================"
	@echo "  Source Code Statistics"
	@echo "========================================"
	@echo "  Files:  $$(echo $(JSTR_SRCS) | wc -w)"
	@echo "  Lines:  $$(cat $(JSTR_SRCS) 2>/dev/null | wc -l)"
	@echo "  Bytes:  $$(cat $(JSTR_SRCS) 2>/dev/null | wc -c)"
	@echo "========================================"
	@echo "  Per-file breakdown:"
	@echo ""
	@for f in $(JSTR_SRCS); do \
		printf "  %-20s %6d lines\n" "$$f" "$$(wc -l < $$f)"; \
	done
	@echo "========================================"

# ----------------------------------------------------------------------------
# Cleanup Target
# ----------------------------------------------------------------------------

clean:
	@echo "Cleaning build artifacts..."
	@rm -f $(OBJS)
	@rm -f $(DEPS)
	@rm -f $(KERNEL_ELF)
	@rm -f $(KERNEL_BIN)
	@rm -f $(KERNEL_ISO)
	@rm -f $(LDMAP)
	@rm -f $(GDB_INIT)
	@rm -rf iso/
	@rm -f test_output.log
	@echo "Clean complete."

# ----------------------------------------------------------------------------
# Help Target
# ----------------------------------------------------------------------------

help:
	@echo ""
	@echo "Jasterish Micro-Kernel -- Build System Help"
	@echo "==========================================="
	@echo ""
	@echo "Build Targets:"
	@echo "  make              Build kernel binary (default)"
	@echo "  make all          Full build (ELF + BIN)"
	@echo "  make build        Alias for 'make all'"
	@echo ""
	@echo "Run Targets:"
	@echo "  make run          Build and launch in QEMU (direct kernel boot)"
	@echo "  make run-iso      Build and launch ISO in QEMU (GRUB2 boot)"
	@echo "  make test         Run kernel in QEMU, capture output to log"
	@echo ""
	@echo "Debug Targets:"
	@echo "  make debug        Launch QEMU with GDB server (port 1234)"
	@echo "  make gdbinit      Create GDB initialization script"
	@echo ""
	@echo "Utility Targets:"
	@echo "  make iso          Create bootable ISO image with GRUB2"
	@echo "  make count        Show source code line counts"
	@echo "  make clean        Remove all build artifacts"
	@echo "  make help         Show this help message"
	@echo ""
	@echo "Configuration Variables:"
	@echo "  JASTERISH_COMPILER   Jasterish compiler path (default: jstar)"
	@echo "  CROSS_COMPILE        Cross-compiler prefix (default: x86_64-elf-)"
	@echo "  QEMU_MEM             QEMU RAM in MB (default: 512)"
	@echo ""
	@echo "Example:"
	@echo "  make JASTERISH_COMPILER=/usr/local/bin/jstar run"
	@echo ""
