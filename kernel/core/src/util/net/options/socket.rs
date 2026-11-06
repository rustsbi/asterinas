// SPDX-License-Identifier: MPL-2.0

use ostd::mm::VmIo;

use super::{RawSocketOption, impl_raw_sock_option_get_only, impl_raw_socket_option};
use crate::{
    context::current_userspace,
    net::socket::options::{
        AcceptConn, Broadcast, Error, KeepAlive, Linger, PassCred, PeerCred, PeerGroups, Priority,
        RecvBuf, RecvBufForce, RecvTimeout, ReuseAddr, ReusePort, SendBuf, SendBufForce,
        SendTimeout, SocketOption, SocketType,
    },
    prelude::*,
    process::Gid,
};

/// Socket level options.
///
/// The definition is from <https://elixir.bootlin.com/linux/v6.0.9/source/include/uapi/asm-generic/socket.h>.
#[expect(non_camel_case_types)]
#[expect(clippy::upper_case_acronyms)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, TryFromInt)]
enum CSocketOptionName {
    DEBUG = 1,
    REUSEADDR = 2,
    TYPE = 3,
    ERROR = 4,
    DONTROUTE = 5,
    BROADCAST = 6,
    SNDBUF = 7,
    RCVBUF = 8,
    KEEPALIVE = 9,
    OOBINLINE = 10,
    NO_CHECK = 11,
    PRIORITY = 12,
    LINGER = 13,
    BSDCOMPAT = 14,
    REUSEPORT = 15,
    PASSCRED = 16,
    PEERCRED = 17,
    RCVTIMEO_OLD = 20,
    SNDTIMEO_OLD = 21,
    ATTACH_FILTER = 26,
    DETACH_FILTER = 27,
    ACCPETCONN = 30,
    PEERSEC = 31,
    SNDBUFFORCE = 32,
    RCVBUFFORCE = 33,
    PEERGROUPS = 59,
    RCVTIMEO_NEW = 66,
    SNDTIMEO_NEW = 67,
}

pub(crate) fn new_socket_option(name: i32) -> Result<Box<dyn RawSocketOption>> {
    let name = CSocketOptionName::try_from(name).map_err(|_| Errno::ENOPROTOOPT)?;
    match name {
        CSocketOptionName::REUSEADDR => Ok(Box::new(ReuseAddr::new())),
        CSocketOptionName::TYPE => Ok(Box::new(SocketType::new())),
        CSocketOptionName::ERROR => Ok(Box::new(Error::new())),
        CSocketOptionName::BROADCAST => Ok(Box::new(Broadcast::new())),
        CSocketOptionName::SNDBUF => Ok(Box::new(SendBuf::new())),
        CSocketOptionName::RCVBUF => Ok(Box::new(RecvBuf::new())),
        CSocketOptionName::KEEPALIVE => Ok(Box::new(KeepAlive::new())),
        CSocketOptionName::PRIORITY => Ok(Box::new(Priority::new())),
        CSocketOptionName::LINGER => Ok(Box::new(Linger::new())),
        CSocketOptionName::RCVTIMEO_OLD => Ok(Box::new(RecvTimeout::new())),
        CSocketOptionName::SNDTIMEO_OLD => Ok(Box::new(SendTimeout::new())),
        #[cfg(target_pointer_width = "64")]
        CSocketOptionName::RCVTIMEO_NEW => Ok(Box::new(RecvTimeout::new())),
        #[cfg(target_pointer_width = "64")]
        CSocketOptionName::SNDTIMEO_NEW => Ok(Box::new(SendTimeout::new())),
        #[cfg(target_pointer_width = "32")]
        CSocketOptionName::RCVTIMEO_NEW => Ok(Box::new(Time64Timeout::Recv(RecvTimeout::new()))),
        #[cfg(target_pointer_width = "32")]
        CSocketOptionName::SNDTIMEO_NEW => Ok(Box::new(Time64Timeout::Send(SendTimeout::new()))),
        CSocketOptionName::REUSEPORT => Ok(Box::new(ReusePort::new())),
        CSocketOptionName::PASSCRED => Ok(Box::new(PassCred::new())),
        CSocketOptionName::PEERCRED => Ok(Box::new(PeerCred::new())),
        CSocketOptionName::ACCPETCONN => Ok(Box::new(AcceptConn::new())),
        CSocketOptionName::SNDBUFFORCE => Ok(Box::new(SendBufForce::new())),
        CSocketOptionName::RCVBUFFORCE => Ok(Box::new(RecvBufForce::new())),
        CSocketOptionName::PEERGROUPS => Ok(Box::new(PeerGroups::new())),
        _ => return_errno_with_message!(Errno::ENOPROTOOPT, "unsupported socket-level option"),
    }
}

impl_raw_socket_option!(ReuseAddr);
impl_raw_sock_option_get_only!(SocketType);
impl_raw_sock_option_get_only!(Error);
impl_raw_socket_option!(Broadcast);
impl_raw_socket_option!(SendBuf);
impl_raw_socket_option!(RecvBuf);
impl_raw_socket_option!(KeepAlive);
impl_raw_socket_option!(Priority);
impl_raw_socket_option!(Linger);
impl_raw_socket_option!(RecvTimeout);
impl_raw_socket_option!(SendTimeout);
impl_raw_socket_option!(ReusePort);
impl_raw_socket_option!(PassCred);
impl_raw_sock_option_get_only!(PeerCred);
impl_raw_sock_option_get_only!(AcceptConn);
impl_raw_socket_option!(SendBufForce);
impl_raw_socket_option!(RecvBufForce);

// SO_*TIMEO_NEW uses two signed 64-bit fields even for an ILP32 caller.
#[cfg(target_pointer_width = "32")]
#[derive(Debug)]
enum Time64Timeout {
    Recv(RecvTimeout),
    Send(SendTimeout),
}

#[cfg(target_pointer_width = "32")]
#[repr(C)]
#[derive(Clone, Copy, Pod)]
struct Timeval64 {
    sec: i64,
    usec: i64,
}

#[cfg(target_pointer_width = "32")]
impl SocketOption for Time64Timeout {
    fn as_any(&self) -> &dyn Any {
        self.as_sock_option().as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.as_sock_option_mut().as_any_mut()
    }
}

#[cfg(target_pointer_width = "32")]
impl RawSocketOption for Time64Timeout {
    fn read_from_user(&mut self, addr: Vaddr, max_len: u32) -> Result<()> {
        use core::time::Duration;

        use crate::net::socket::util::SocketTimeout;

        if (max_len as usize) < size_of::<Timeval64>() {
            return_errno_with_message!(Errno::EINVAL, "max_len is too short");
        }
        let value = current_userspace!().read_val::<Timeval64>(addr)?;
        let duration = if value.sec < 0 {
            None
        } else {
            if !(0..1_000_000).contains(&value.usec) {
                return_errno_with_message!(Errno::EDOM, "usec is out of range");
            }
            let duration = Duration::new(value.sec as u64, value.usec as u32 * 1_000);
            (!duration.is_zero()).then_some(duration)
        };
        let timeout = SocketTimeout::new(duration);
        match self {
            Self::Recv(option) => option.set(timeout),
            Self::Send(option) => option.set(timeout),
        }
        Ok(())
    }

    fn write_to_user(&self, addr: Vaddr, max_len: &mut u32) -> Result<usize> {
        let len = size_of::<Timeval64>();
        if (*max_len as usize) < len {
            return_errno_with_message!(Errno::EINVAL, "max_len is too short");
        }
        let timeout = match self {
            Self::Recv(option) => option.get().unwrap(),
            Self::Send(option) => option.get().unwrap(),
        };
        let duration = timeout.duration().unwrap_or_default();
        current_userspace!().write_val(
            addr,
            &Timeval64 {
                sec: duration.as_secs() as i64,
                usec: duration.subsec_micros() as i64,
            },
        )?;
        Ok(len)
    }

    fn as_sock_option_mut(&mut self) -> &mut dyn SocketOption {
        match self {
            Self::Recv(option) => option,
            Self::Send(option) => option,
        }
    }

    fn as_sock_option(&self) -> &dyn SocketOption {
        match self {
            Self::Recv(option) => option,
            Self::Send(option) => option,
        }
    }
}

// SO_PEERGROUPS is a read-only option. However, calling setsockopt on SO_PEERGROUPS will return EINVAL
// instead of ENOPROTOOPT like other options. Therefore, we manually implement `RawSocketOption` for it.
impl RawSocketOption for PeerGroups {
    fn read_from_user(&mut self, _addr: Vaddr, _max_len: u32) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "the option is getter-only");
    }

    fn write_to_user(&self, addr: Vaddr, buffer_len: &mut u32) -> Result<usize> {
        let groups = self.get().unwrap();

        let old_len = *buffer_len;
        *buffer_len = (groups.len() * size_of::<Gid>()) as u32;
        if old_len < *buffer_len {
            return_errno_with_message!(Errno::ERANGE, "the buffer is too small");
        }

        for (i, gid) in groups.iter().enumerate() {
            let dst = addr + i * size_of::<Gid>();
            current_userspace!().write_val(dst, gid)?;
        }

        Ok(*buffer_len as usize)
    }

    fn as_sock_option_mut(&mut self) -> &mut dyn SocketOption {
        self
    }

    fn as_sock_option(&self) -> &dyn SocketOption {
        self
    }
}
