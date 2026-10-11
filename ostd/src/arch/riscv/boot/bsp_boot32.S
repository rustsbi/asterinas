/* SPDX-License-Identifier: MPL-2.0 */

# Sv32 startup. The linker supplies KERNEL_LMA, KERNEL_VMA and their offset.
.option push
.option norelax
.section ".boot", "awx", @progbits

.global _start
_start:
    # a0 = hart ID, a1 = DTB physical address. Preserve both for riscv_boot.
    lla t0, sv32_boot_root

    # Linear map PA 0..0x90000000 at VA 0x40000000..0xd0000000.
    # Leaf PTEs have V/R/W/X/A/D set during bootstrap.
    addi t1, t0, 256 * 4
    li t2, 0xcf
    li t3, 576
    li t4, 0x100000
bsp32_map_linear:
    sw t2, 0(t1)
    addi t1, t1, 4
    add t2, t2, t4
    addi t3, t3, -1
    bnez t3, bsp32_map_linear

    # Map the kernel's 256 MiB virtual window starting at KERNEL_VMA.
    lui t2, %hi(KERNEL_LMA)
    addi t2, t2, %lo(KERNEL_LMA)
    srli t2, t2, 2
    ori t2, t2, 0xcf
    lui t1, %hi(KERNEL_VMA)
    addi t1, t1, %lo(KERNEL_VMA)
    srli t1, t1, 20
    add t1, t1, t0
    li t3, 64
bsp32_map_code:
    sw t2, 0(t1)
    addi t1, t1, 4
    add t2, t2, t4
    addi t3, t3, -1
    bnez t3, bsp32_map_code

    # Temporarily identity-map the boot trampoline's 4 MiB region. This
    # replaces one low-RAM linear alias that is not used by either board.
    lui t2, %hi(KERNEL_LMA)
    addi t2, t2, %lo(KERNEL_LMA)
    srli t1, t2, 20
    add t1, t1, t0
    srli t2, t2, 2
    ori t2, t2, 0xcf
    sw t2, 0(t1)

    lla sp, boot_stack_top
    lui t1, %hi(KERNEL_VMA_OFFSET)
    addi t1, t1, %lo(KERNEL_VMA_OFFSET)
    add sp, sp, t1
    lui t2, %hi(bsp32_boot_virtual)
    addi t2, t2, %lo(bsp32_boot_virtual)

    srli t0, t0, 12
    li t1, 1 << 31
    or t0, t0, t1
    sfence.vma
    csrw satp, t0
    sfence.vma
    jr t2

.balign 4096
sv32_boot_root:
    .zero 4096

.section ".boot.stack", "aw", @nobits
.balign 4096
boot_stack_bottom:
    .skip 0x40000
boot_stack_top:

.text
bsp32_boot_virtual:
    lla gp, __cpu_local_start
    li fp, 0
    lla t0, bsp32_early_trap
    csrw stvec, t0
    tail riscv_boot

# The full trap handler is installed after memory initialization. Keep faults
# before that point from jumping back to the firmware's kernel entry vector.
.balign 4
bsp32_early_trap:
    csrr a0, scause
    csrr a1, sepc
    csrr a2, stval
    tail {early_boot_trap}
.option pop
