//! One cancellable, nonblocking connection to the already admitted exact peer.
//! No DNS, retry, redirect, additional buffer or independent timeout is added.
use super::{check, OpenMeteoHttpRefusal as Error, POLL_WAIT};
use std::{
    io,
    net::{SocketAddr, TcpStream},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    time::Instant,
};
type Result<T> = std::result::Result<T, Error>;

pub(super) fn connect(
    endpoint: SocketAddr,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<TcpStream> {
    check(deadline, cancelled)?;
    let family = if endpoint.is_ipv4() {
        libc::AF_INET
    } else {
        libc::AF_INET6
    };
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let flags = libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let flags = libc::SOCK_STREAM;
    // SAFETY: Internet TCP socket with no borrowed storage or ownership input.
    let raw = unsafe { libc::socket(family, flags, libc::IPPROTO_TCP) };
    if raw < 0 {
        return Err(Error::Connect);
    }
    // SAFETY: socket just returned this uniquely owned descriptor. Every later
    // refusal drops the owner; successful conversion transfers it exactly once.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    let stream = TcpStream::from(owned);
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        stream.set_nonblocking(true).map_err(|_| Error::Connect)?;
        // SAFETY: the descriptor remains owned/live; integer-only fcntl flags.
        let flags = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) }
                < 0
        {
            return Err(Error::Connect);
        }
    }
    check(deadline, cancelled)?;
    let result = match endpoint {
        SocketAddr::V4(peer) => {
            // SAFETY: zero is a valid initial representation, including padding.
            let mut address: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            address.sin_family = libc::AF_INET as libc::sa_family_t;
            address.sin_port = peer.port().to_be();
            address.sin_addr.s_addr = u32::from_ne_bytes(peer.ip().octets());
            #[cfg(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly"
            ))]
            {
                address.sin_len = std::mem::size_of_val(&address) as u8;
            }
            // SAFETY: pointer/length describe one initialized IPv4 socket address;
            // connect borrows it only for this nonblocking syscall.
            unsafe {
                libc::connect(
                    stream.as_raw_fd(),
                    (&address as *const libc::sockaddr_in).cast(),
                    std::mem::size_of_val(&address) as libc::socklen_t,
                )
            }
        }
        SocketAddr::V6(peer) => {
            // SAFETY: zero is valid for address storage and platform padding.
            let mut address: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
            address.sin6_family = libc::AF_INET6 as libc::sa_family_t;
            address.sin6_port = peer.port().to_be();
            address.sin6_flowinfo = peer.flowinfo();
            address.sin6_scope_id = peer.scope_id();
            address.sin6_addr.s6_addr = peer.ip().octets();
            #[cfg(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly"
            ))]
            {
                address.sin6_len = std::mem::size_of_val(&address) as u8;
            }
            // SAFETY: pointer/length describe one initialized IPv6 socket address;
            // connect borrows it only for this nonblocking syscall.
            unsafe {
                libc::connect(
                    stream.as_raw_fd(),
                    (&address as *const libc::sockaddr_in6).cast(),
                    std::mem::size_of_val(&address) as libc::socklen_t,
                )
            }
        }
    };
    if result != 0 {
        let error = io::Error::last_os_error().raw_os_error();
        if ![
            Some(libc::EINPROGRESS),
            Some(libc::EALREADY),
            Some(libc::EINTR),
        ]
        .contains(&error)
        {
            return Err(Error::Connect);
        }
        wait_connected(&stream, deadline, cancelled)?;
    }
    check(deadline, cancelled)?;
    Ok(stream)
}
fn wait_connected(
    stream: &TcpStream,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
) -> Result<()> {
    loop {
        check(deadline, cancelled)?;
        let mut fd = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        let millis = deadline
            .saturating_duration_since(Instant::now())
            .min(POLL_WAIT)
            .as_millis()
            .max(1) as i32;
        // SAFETY: one initialized pollfd borrows the still-owned live descriptor.
        let ready = unsafe { libc::poll(&mut fd, 1, millis) };
        check(deadline, cancelled)?;
        if ready < 0 {
            if io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(Error::Connect);
        }
        if ready == 0 {
            continue;
        }
        if fd.revents & libc::POLLNVAL != 0 {
            return Err(Error::Connect);
        }
        let mut error: libc::c_int = 0;
        let mut length = std::mem::size_of_val(&error) as libc::socklen_t;
        // SAFETY: SO_ERROR writes at most the exact writable integer length;
        // both pointers remain live and the descriptor is owned throughout.
        let status = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&mut error as *mut libc::c_int).cast(),
                &mut length,
            )
        };
        if status != 0
            || length as usize != std::mem::size_of_val(&error)
            || error != 0
            || fd.revents & libc::POLLOUT == 0
        {
            return Err(Error::Connect);
        }
        return Ok(());
    }
}

#[cfg(test)]
#[path = "connect/tests.rs"]
mod tests;
