// SPDX-License-Identifier: MPL-2.0

use int_to_c_enum::TryFromInt;
use ostd::mm::VmIo;

use super::SyscallReturn;
use crate::{prelude::*, time::timeval_t};

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, TryFromInt)]
enum RusageTarget {
    ForSelf = 0,
    Children = -1,
    Thread = 1,
}

pub(super) fn sys_getrusage(
    target: i32,
    rusage_addr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let rusage_target = RusageTarget::try_from(target)?;

    debug!(
        "target = {:?}, rusage_addr = {}",
        rusage_target, rusage_addr,
    );

    if rusage_addr != 0 {
        let rusage = match rusage_target {
            RusageTarget::ForSelf => {
                let process = ctx.process.as_ref();
                rusage_t {
                    ru_utime: process.prof_clock().user_clock().read_time().into(),
                    ru_stime: process.prof_clock().kernel_clock().read_time().into(),
                    ..Default::default()
                }
            }
            RusageTarget::Thread => {
                let posix_thread = ctx.posix_thread;
                rusage_t {
                    ru_utime: posix_thread.prof_clock().user_clock().read_time().into(),
                    ru_stime: posix_thread.prof_clock().kernel_clock().read_time().into(),
                    ..Default::default()
                }
            }
            RusageTarget::Children => {
                let (user_time, kernel_time) = ctx.process.reaped_children_stats().lock().get();
                rusage_t {
                    ru_utime: user_time.into(),
                    ru_stime: kernel_time.into(),
                    ..Default::default()
                }
            }
        };

        ctx.user_space().write_val(rusage_addr, &rusage)?;
    }

    Ok(SyscallReturn::Return(0))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct rusage_t {
    /// user time used
    pub ru_utime: timeval_t,
    /// system time used
    pub ru_stime: timeval_t,
    /// maximum resident set size
    pub ru_maxrss: usize,
    /// integral shared memory size
    pub ru_ixrss: usize,
    /// integral unshared data size
    pub ru_idrss: usize,
    /// integral unshared stack size
    pub ru_isrss: usize,
    /// page reclaims
    pub ru_minflt: usize,
    /// page faults
    pub ru_majflt: usize,
    /// swaps
    pub ru_nswap: usize,
    /// block input operations
    pub ru_inblock: usize,
    /// block output operations
    pub ru_oublock: usize,
    /// messages sent
    pub ru_msgsnd: usize,
    /// messages received
    pub ru_msgrcv: usize,
    /// signals received
    pub ru_nsignals: usize,
    /// voluntary ctx switches
    pub ru_nvcsw: usize,
    /// involuntary
    pub ru_nivcsw: usize,
}
