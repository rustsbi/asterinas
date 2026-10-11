/* SPDX-License-Identifier: MPL-2.0 */

OUTPUT_FORMAT(elf32-littleriscv)
OUTPUT_ARCH(riscv)

PHDRS
{
    text PT_LOAD FILEHDR PHDRS FLAGS(5);
    dynamic PT_DYNAMIC FLAGS(4);
}

SECTIONS
{
    . = SIZEOF_HEADERS;
    .hash : { *(.hash) } :text
    .gnu.hash : { *(.gnu.hash) }
    .dynsym : { *(.dynsym) }
    .dynstr : { *(.dynstr) }
    .gnu.version : { *(.gnu.version) }
    .gnu.version_d : { *(.gnu.version_d) }
    .dynamic : { *(.dynamic) } :text :dynamic

    /* Keep the signal restorer offset synchronized with kernel/core/src/vdso.rs. */
    .text 0x800 : { KEEP(*(.text.sigreturn)) *(.text .text.*) } :text
    ASSERT(__vdso_rt_sigreturn == 0x800, "unexpected RV32 signal restorer offset")
    ASSERT(. <= 0xfff, "RV32 vDSO exceeds one page")
    .image_end 0xfff : { BYTE(0) } :text

    /DISCARD/ : { *(.comment) *(.note.*) *(.riscv.attributes) }
}

/* RV32 libc uses LINUX_5.4, unlike RV64's LINUX_4.15. */
VERSION
{
    LINUX_5.4 {
        global:
            __vdso_rt_sigreturn;
            __vdso_clock_gettime64;
            __vdso_clock_getres_time64;
            __vdso_getcpu;
            __vdso_flush_icache;
        local: *;
    };
}
