/* SPDX-License-Identifier: MPL-2.0 */

.option push
.option norelax
.option arch, +a
.section ".ap_boot", "awx", @progbits

.balign 4096
.global ap_boot_start
ap_boot_start:
    # The SBI starts this hart in S-mode with its hardware ID in a0.
    mv t4, a0
    lui t0, %hi(__ap_boot_page_table_pointer)
    addi t0, t0, %lo(__ap_boot_page_table_pointer)
    lui t2, %hi(KERNEL_VMA_OFFSET)
    addi t2, t2, %lo(KERNEL_VMA_OFFSET)
    sub t0, t0, t2
    lw t1, 0(t0)
    srli t1, t1, 12
    li t2, 1 << 31
    or t1, t1, t2
    lui t0, %hi(ap32_boot_virtual)
    addi t0, t0, %lo(ap32_boot_virtual)
    sfence.vma
    csrw satp, t1
    sfence.vma
    jr t0

.data
.balign 4
.global __ap_boot_page_table_pointer
__ap_boot_page_table_pointer:
    .word 0
.global __ap_boot_info_array_pointer
__ap_boot_info_array_pointer:
    .word 0
ap32_boot_cpu_id_tail:
    .word 1

.text
ap32_boot_virtual:
    lla t0, ap32_boot_cpu_id_tail
    li t1, 1
    amoadd.w.aq t1, t1, (t0)
    lla t2, __ap_boot_info_array_pointer
    lw t2, 0(t2)
    # PerApRawInfo contains two 32-bit pointers.
    addi t3, t1, -1
    slli t3, t3, 3
    add t3, t3, t2
    lw sp, 0(t3)
    lw gp, 4(t3)
    li fp, 0
    mv a0, t1
    mv a1, t4
    tail riscv_ap_early_entry
.option pop
