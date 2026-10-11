// SPDX-License-Identifier: MPL-2.0

// RV32 uses the Linux time64 syscall ABI. These entry points intentionally
// use system calls rather than depending on Linux's private vvar layout.
// https://github.com/torvalds/linux/blob/v6.12/include/uapi/asm-generic/unistd.h
.option norvc
.option norelax

.section .text.sigreturn, "ax"

.global __vdso_rt_sigreturn
.balign 4
.type __vdso_rt_sigreturn, @function
__vdso_rt_sigreturn:
    li a7, 139
    ecall
.size __vdso_rt_sigreturn, . - __vdso_rt_sigreturn

.text

.global __vdso_clock_gettime64
.balign 4
.type __vdso_clock_gettime64, @function
__vdso_clock_gettime64:
    li a7, 403
    ecall
    ret
.size __vdso_clock_gettime64, . - __vdso_clock_gettime64

.global __vdso_clock_getres_time64
.balign 4
.type __vdso_clock_getres_time64, @function
__vdso_clock_getres_time64:
    li a7, 406
    ecall
    ret
.size __vdso_clock_getres_time64, . - __vdso_clock_getres_time64

.global __vdso_getcpu
.balign 4
.type __vdso_getcpu, @function
__vdso_getcpu:
    li a7, 168
    ecall
    ret
.size __vdso_getcpu, . - __vdso_getcpu

.global __vdso_flush_icache
.balign 4
.type __vdso_flush_icache, @function
__vdso_flush_icache:
    li a7, 259
    ecall
    ret
.size __vdso_flush_icache, . - __vdso_flush_icache

.section .note.GNU-stack, "", @progbits
