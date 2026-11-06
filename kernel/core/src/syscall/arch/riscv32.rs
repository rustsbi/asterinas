// SPDX-License-Identifier: MPL-2.0

//! Linux RV32 uses the generic ILP32 table with time64 system calls.
//!
//! Scalar 64-bit arguments occupy consecutive low/high registers without padding.
//! See Linux `include/uapi/asm-generic/unistd.h` and musl `arch/riscv32/syscall_arch.h`.

use ostd::{arch::cpu::context::UserContext, mm::VmIo};

use crate::{
    fs::file::{SeekFrom, file_table::get_file_fast},
    prelude::*,
    syscall::{SyscallReturn, clock_gettime::sys_clock_getres},
};

pub(crate) fn syscall_dispatch(
    number: u64,
    mut args: [u64; 6],
    ctx: &Context,
    user_ctx: &mut UserContext,
) -> Result<SyscallReturn> {
    let number = match number {
        // RV32 has no legacy time32 or legacy stat entry points.
        72 | 73 | 79 | 80 | 86 | 87 | 88 | 98 | 101 | 108 | 110 | 112 | 113 | 114 | 115 | 127
        | 137 | 163 | 164 | 169 | 170 | 192 => {
            return_errno_with_message!(Errno::ENOSYS, "RV32 requires the time64/statx ABI");
        }
        403 => 113, // clock_gettime64
        406 => return sys_clock_getres(args[0] as _, args[1] as _, ctx),
        407 => 115, // clock_nanosleep_time64
        408 => 108, // timer_gettime64
        409 => 110, // timer_settime64
        410 => 87,  // timerfd_gettime64
        411 => 86,  // timerfd_settime64
        412 => 88,  // utimensat_time64
        413 => 72,  // pselect6_time64
        414 => 73,  // ppoll_time64
        420 => 192, // semtimedop_time64
        421 => 137, // rt_sigtimedwait_time64
        422 => 98,  // futex_time64
        25 => {
            // The existing flock layout is Linux flock64, including its alignment.
            args[1] = match args[1] {
                12 => 5,
                13 => 6,
                14 => 7,
                5..=7 => return_errno_with_message!(Errno::ENOSYS, "use the 64-bit file lock ABI"),
                command => command,
            };
            number
        }
        43 | 44 => {
            if args[1] as usize != crate::syscall::statfs::STATFS_SIZE {
                return_errno_with_message!(Errno::EINVAL, "invalid statfs64 structure size");
            }
            args[1] = args[2];
            number
        }
        45 | 46 => {
            args[1] = signed_word(pair(args[1], args[2]))?;
            number
        }
        47 => {
            args[2] = unsigned_word(pair(args[2], args[3]))?;
            args[3] = unsigned_word(pair(args[4], args[5]))?;
            number
        }
        62 => return llseek(args, ctx),
        67 | 68 => {
            args[3] = unsigned_word(pair(args[3], args[4]))?;
            number
        }
        69 | 70 | 286 | 287 => {
            let offset = pair(args[3], args[4]);
            args[3] = if offset == u64::MAX && number >= 286 {
                offset // preadv2/pwritev2 accept -1 to use the current file position.
            } else {
                unsigned_word(offset)?
            };
            number
        }
        220 => {
            // RISC-V clone has TLS before child_tid; the shared handler has them reversed.
            args.swap(3, 4);
            number
        }
        222 => {
            args[5] = unsigned_word(args[5] << 12)?; // mmap2 offsets are in 4-KiB units.
            number
        }
        223 => {
            let offset = unsigned_word(pair(args[1], args[2]))?;
            let len = unsigned_word(pair(args[3], args[4]))?;
            args[1] = offset;
            args[2] = len;
            args[3] = args[5];
            number
        }
        _ => number,
    };
    super::syscall_dispatch(number, args, ctx, user_ctx)
}

fn pair(low: u64, high: u64) -> u64 {
    low | (high << 32)
}

// The current VFS uses native-sized offsets. Reject unrepresentable values instead of
// truncating them and operating on a different part of the file.
fn unsigned_word(value: u64) -> Result<u64> {
    usize::try_from(value)
        .map(|value| value as u64)
        .map_err(|_| Error::with_message(Errno::EOVERFLOW, "file offset exceeds the VFS limit"))
}

fn signed_word(value: u64) -> Result<u64> {
    isize::try_from(value as i64)
        .map(|value| value as u64)
        .map_err(|_| Error::with_message(Errno::EOVERFLOW, "file offset exceeds the VFS limit"))
}

fn llseek(args: [u64; 6], ctx: &Context) -> Result<SyscallReturn> {
    let offset = pair(args[2], args[1]);
    let from = match args[4] {
        0 => SeekFrom::Start(unsigned_word(offset)? as usize),
        1 => SeekFrom::Current(signed_word(offset)? as isize),
        2 => SeekFrom::End(signed_word(offset)? as isize),
        _ => return_errno_with_message!(Errno::EINVAL, "invalid seek origin"),
    };
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, (args[0] as i32).try_into()?);
    let offset = file.seek(from)? as u64;
    ctx.user_space().write_val(args[3] as Vaddr, &offset)?;
    Ok(SyscallReturn::Return(0))
}
