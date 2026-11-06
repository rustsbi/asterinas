// SPDX-License-Identifier: MPL-2.0

//! System call dispatch in the RISC-V architecture.

#[path = "./generic.rs"]
mod generic;

#[cfg(target_arch = "riscv32")]
#[path = "./riscv32.rs"]
mod rv32;
#[cfg(target_arch = "riscv32")]
pub(crate) use rv32::syscall_dispatch as syscall_dispatch_rv32;

use crate::{prelude::*, syscall::SyscallReturn};

generic::define_syscalls_with_generic_syscall_table! {
    SYS_RISCV_FLUSH_ICACHE = 259 => sys_riscv_flush_icache(args[..3]);
}

fn sys_riscv_flush_icache(
    _start: Vaddr,
    _end: Vaddr,
    flags: usize,
    _ctx: &Context,
) -> Result<SyscallReturn> {
    if flags & !1 != 0 {
        return_errno_with_message!(Errno::EINVAL, "invalid instruction cache flush flags");
    }
    // Synchronize every hart so subsequent migration also sees the new instructions.
    ostd::smp::inter_processor_call(
        &ostd::cpu::CpuSet::new_full(),
        ostd::arch::flush_instruction_cache,
    )
    .wait();
    Ok(SyscallReturn::Return(0))
}
