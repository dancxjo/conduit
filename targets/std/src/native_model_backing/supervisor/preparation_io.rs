//! One Host preparation-I/O deadline, not an execution/performance guarantee.
use super::NativeBackingRefusal;
use std::{
    cell::Cell,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    time::Instant,
};

pub(super) struct Deadline {
    expires: Instant,
    timed_out: Cell<bool>,
}
impl Deadline {
    pub(super) fn new(expires: Instant) -> Self {
        Self {
            expires,
            timed_out: Cell::new(false),
        }
    }
    pub(super) fn io<'a, T>(&'a self, inner: &'a mut T) -> BoundedIo<'a, T> {
        BoundedIo {
            deadline: self,
            inner,
        }
    }
    pub(super) fn classify(&self, error: NativeBackingRefusal) -> NativeBackingRefusal {
        if self.timed_out.get() {
            NativeBackingRefusal::PreparationTimedOut
        } else {
            error
        }
    }
    fn remaining(&self) -> io::Result<std::time::Duration> {
        let remaining = self.expires.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            self.timed_out.set(true);
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(remaining)
        }
    }
    fn wait(&self, fd: i32, events: i16) -> io::Result<()> {
        loop {
            let remaining = self.remaining()?;
            // Round upward: a sub-millisecond remainder is not a zero-time spin.
            let millis = remaining
                .as_millis()
                .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
                .min(i32::MAX as u128) as i32;
            let mut poll = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut poll, 1, millis) };
            if result > 0 {
                return Ok(());
            }
            if result == 0 {
                continue;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
pub(super) struct BoundedIo<'a, T> {
    deadline: &'a Deadline,
    inner: &'a mut T,
}
impl<T: Read + AsRawFd> Read for BoundedIo<'_, T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.deadline.remaining()?;
            match self.inner.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.deadline.wait(self.inner.as_raw_fd(), libc::POLLIN)?
                }
                result => return result,
            }
        }
    }
}
impl<T: Write + AsRawFd> Write for BoundedIo<'_, T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.deadline.remaining()?;
            match self.inner.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.deadline.wait(self.inner.as_raw_fd(), libc::POLLOUT)?
                }
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        loop {
            self.deadline.remaining()?;
            match self.inner.flush() {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    self.deadline.wait(self.inner.as_raw_fd(), libc::POLLOUT)?
                }
                result => return result,
            }
        }
    }
}
#[cfg(test)]
#[path = "preparation_io_tests.rs"]
mod tests;
