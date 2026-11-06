/* SPDX-License-Identifier: MPL-2.0 */

.text

# Atomically loads a 32-bit integer value. This function works with exception
# handling and can recover from a page fault.
#
# Returns the loaded value or `!0u64` if failed to load.
.global __atomic_load_fallible
.type __atomic_load_fallible, @function
__atomic_load_fallible: # (ptr: *const u32) -> u64;
    li t0, {SSTATUS_SUM}
    csrs sstatus, t0
load:
  .if {XLEN} == 64
    lwu a0, (a0)
  .else
    lw a0, (a0)
    li a1, 0
  .endif
    csrc sstatus, t0
    ret
load_fault:
    li a0, -1
  .if {XLEN} == 32
    li a1, -1
  .endif
    csrc sstatus, t0
    ret
.size __atomic_load_fallible, .-__atomic_load_fallible

.pushsection .ex_table, "a", @progbits
    .balign {XLEN} / 8
  .if {XLEN} == 64
    .quad load
    .quad load_fault
  .else
    .word load
    .word load_fault
  .endif
.popsection
