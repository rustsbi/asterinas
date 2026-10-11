// SPDX-License-Identifier: MPL-2.0

use bitflags::bitflags;
use ostd::mm::VmIo;

use super::SyscallReturn;
use crate::{
    fs::{
        file::file_table::{RawFileDesc, get_file_fast},
        utils::PATH_MAX,
        vfs::{
            file_system::FsFlags,
            path::{FsPath, Mount, PerMountFlags},
        },
    },
    prelude::*,
};

pub(super) fn sys_statfs(
    path_ptr: Vaddr,
    statfs_buf_ptr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let user_space = ctx.user_space();
    let path_name = user_space.read_cstring(path_ptr, PATH_MAX)?;
    debug!(
        "path = {:?}, statfs_buf_ptr = 0x{:x}",
        path_name, statfs_buf_ptr,
    );

    let statfs = {
        let path_name = path_name.to_string_lossy();
        let fs_path = FsPath::try_from(path_name.as_ref())?;
        let path = ctx
            .thread_local
            .borrow_fs()
            .resolver()
            .read()
            .lookup(&fs_path)?;
        Statfs::new(path.mount_node())
    };
    user_space.write_val(statfs_buf_ptr, &statfs)?;
    Ok(SyscallReturn::Return(0))
}

pub(super) fn sys_fstatfs(
    raw_fd: RawFileDesc,
    statfs_buf_ptr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    debug!(
        "raw_fd = {}, statfs_buf_addr = 0x{:x}",
        raw_fd, statfs_buf_ptr
    );

    let statfs = {
        let mut file_table = ctx.thread_local.borrow_file_table_mut();
        let file = get_file_fast!(&mut file_table, raw_fd.try_into()?);
        Statfs::new(file.path().mount_node())
    };
    ctx.user_space().write_val(statfs_buf_ptr, &statfs)?;
    Ok(SyscallReturn::Return(0))
}

/// FS Stat
#[padding_struct]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
struct Statfs {
    /// Type of filesystem
    f_type: usize,
    /// Optimal transfer block size
    f_bsize: usize,
    /// Total data blocks in filesystem
    f_blocks: u64,
    /// Free blocks in filesystem
    f_bfree: u64,
    /// Free blocks available to unprivileged user
    f_bavail: u64,
    /// Total inodes in filesystem
    f_files: u64,
    /// Free inodes in filesystem
    f_ffree: u64,
    /// Filesystem ID
    f_fsid: u64,
    /// Maximum length of filenames
    f_namelen: usize,
    /// Fragment size
    f_frsize: usize,
    /// Mount flags of filesystem
    f_flags: usize,
    /// Padding bytes reserved for future use
    f_spare: [usize; 4],
}

#[cfg(target_arch = "riscv32")]
pub(super) const STATFS_SIZE: usize = size_of::<Statfs>();

impl Statfs {
    fn new(mount: &Mount) -> Self {
        let sb = mount.fs().sb();
        // TODO: Make `SuperBlock` correctly implement and maintain `FsFlags`,
        // so they can be retrieved directly here.
        let statfs_flags =
            StatfsFlags::new(mount.flags(), FsFlags::from_bits_truncate(sb.flags as u32));
        Self {
            f_type: sb.magic as usize,
            f_bsize: sb.bsize,
            f_blocks: sb.blocks as u64,
            f_bfree: sb.bfree as u64,
            f_bavail: sb.bavail as u64,
            f_files: sb.files as u64,
            f_ffree: sb.ffree as u64,
            f_fsid: sb.fsid,
            f_namelen: sb.namelen,
            f_frsize: sb.frsize,
            f_flags: statfs_flags.bits() as usize,
            f_spare: [0; 4],
            ..Default::default()
        }
    }
}

bitflags! {
    /// User-visible flags in [`Statfs`].
    ///
    /// Reference: <https://elixir.bootlin.com/linux/v6.16.5/source/include/linux/statfs.h#L31>
    struct StatfsFlags: u32 {
        /// Mount read-only.
        const ST_RDONLY = 1 << 0;
        /// Ignore suid and sgid bits.
        const ST_NOSUID = 1 << 1;
        /// Disallow access to device special files.
        const ST_NODEV = 1 << 2;
        /// Disallow program execution.
        const ST_NOEXEC = 1 << 3;
        /// Writes are synced at once.
        const ST_SYNCHRONOUS = 1 << 4;
        /// Allow mandatory locks on an FS.
        const ST_MANDLOCK = 1 << 6;
        /// Do not update access times.
        const ST_NOATIME = 1 << 10;
        /// Do not update directory access times.
        const ST_NODIRATIME = 1 << 11;
        /// Update atime relative to mtime/ctime.
        const ST_RELATIME = 1 << 12;
        /// Do not follow symlinks.
        const ST_NOSYMFOLLOW = 1 << 13;
    }
}

impl StatfsFlags {
    fn new(per_mount_flags: PerMountFlags, fs_flags: FsFlags) -> Self {
        let mut statfs_flags = StatfsFlags::from_bits_truncate(per_mount_flags.bits());
        statfs_flags |= StatfsFlags::from_bits_truncate(fs_flags.bits());

        // Some bits in `StatfsFlags` do not correspond directly to bits in
        // `PerMountFlags` and `FsFlags`, so we need to populate them manually.
        // These manually added bits do not exist in `PerMountFlags` or `FsFlags`,
        // so they will not be populated incorrectly before.
        if per_mount_flags.contains(PerMountFlags::NOSYMFOLLOW) {
            statfs_flags |= StatfsFlags::ST_NOSYMFOLLOW;
        }
        if per_mount_flags.contains(PerMountFlags::RELATIME) {
            statfs_flags |= StatfsFlags::ST_RELATIME;
        }

        statfs_flags
    }
}
