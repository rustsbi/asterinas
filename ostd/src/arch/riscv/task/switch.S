/* SPDX-License-Identifier: MPL-2.0 */

.macro SWITCH_STORE reg, index
  .if {XLEN} == 64
    sd \reg, \index * 8(a1)
  .else
    sw \reg, \index * 4(a1)
  .endif
.endm

.macro SWITCH_LOAD reg, index
  .if {XLEN} == 64
    ld \reg, \index * 8(a0)
  .else
    lw \reg, \index * 4(a0)
  .endif
.endm

.text

.global context_switch
.type context_switch, @function
context_switch: # (nxt: *const TaskContext, cur: *mut TaskContext)
  # Save cur's register
  SWITCH_STORE ra, 13
  SWITCH_STORE sp, 0
  SWITCH_STORE s0, 1
  SWITCH_STORE s1, 2
  SWITCH_STORE s2, 3
  SWITCH_STORE s3, 4
  SWITCH_STORE s4, 5
  SWITCH_STORE s5, 6
  SWITCH_STORE s6, 7
  SWITCH_STORE s7, 8
  SWITCH_STORE s8, 9
  SWITCH_STORE s9, 10
  SWITCH_STORE s10, 11
  SWITCH_STORE s11, 12
  # Fallthrough

.global first_context_switch
.type first_context_switch, @function
first_context_switch: # (nxt: *const TaskContext)
  # Restore nxt's registers
  SWITCH_LOAD ra, 13
  SWITCH_LOAD sp, 0
  SWITCH_LOAD s0, 1
  SWITCH_LOAD s1, 2
  SWITCH_LOAD s2, 3
  SWITCH_LOAD s3, 4
  SWITCH_LOAD s4, 5
  SWITCH_LOAD s5, 6
  SWITCH_LOAD s6, 7
  SWITCH_LOAD s7, 8
  SWITCH_LOAD s8, 9
  SWITCH_LOAD s9, 10
  SWITCH_LOAD s10, 11
  SWITCH_LOAD s11, 12

  ret
.size context_switch, .-context_switch
.size first_context_switch, .-first_context_switch

.global kernel_task_entry_wrapper
.type kernel_task_entry_wrapper, @function
kernel_task_entry_wrapper:
  .cfi_startproc
  .cfi_undefined ra # mark return address as undefined to indicate end of call stack
  call kernel_task_entry
  .cfi_endproc
.size kernel_task_entry_wrapper, .-kernel_task_entry_wrapper
