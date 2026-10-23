use super::*;
use std::{os::unix::net::UnixStream, time::Duration};
struct Partial {
    fd: UnixStream,
    bytes: [u8; 4],
    index: usize,
    calls: usize,
}
impl AsRawFd for Partial {
    fn as_raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }
}
impl Read for Partial {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        match self.calls {
            1 => return Err(io::ErrorKind::Interrupted.into()),
            2 => return Err(io::ErrorKind::WouldBlock.into()),
            _ => {}
        }
        if self.index == 4 {
            return Ok(0);
        }
        out[0] = self.bytes[self.index];
        self.index += 1;
        Ok(1)
    }
}
impl Write for Partial {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        match self.calls {
            1 => return Err(io::ErrorKind::Interrupted.into()),
            2 => return Err(io::ErrorKind::WouldBlock.into()),
            _ => {}
        }
        self.bytes[self.index] = bytes[0];
        self.index += 1;
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn future() -> Deadline {
    Deadline::new(Instant::now() + Duration::from_secs(1))
}
#[test]
fn partial_read_retries_interrupt_and_actual_ready_poll_without_losing_bytes() {
    let (fd, mut peer) = UnixStream::pair().unwrap();
    super::super::nonblocking(&fd).unwrap();
    peer.write_all(b"R").unwrap(); // Actual POLLIN readiness for the scripted partial reader.
    let mut partial = Partial {
        fd,
        bytes: *b"ABCD",
        index: 0,
        calls: 0,
    };
    let deadline = future();
    let mut out = [0; 4];
    deadline.io(&mut partial).read_exact(&mut out).unwrap();
    assert_eq!(out, *b"ABCD");
    assert_eq!(partial.calls, 6);
}
#[test]
fn partial_write_uses_same_deadline_and_actual_writable_poll() {
    let (fd, _peer) = UnixStream::pair().unwrap();
    super::super::nonblocking(&fd).unwrap();
    let mut partial = Partial {
        fd,
        bytes: [0; 4],
        index: 0,
        calls: 0,
    };
    let deadline = future();
    deadline.io(&mut partial).write_all(b"ABCD").unwrap();
    assert_eq!(partial.bytes, *b"ABCD");
    assert_eq!(partial.calls, 6);
}
#[test]
fn expired_deadline_is_distinct_and_never_attempts_io_or_refreshes_budget() {
    let (fd, _peer) = UnixStream::pair().unwrap();
    let mut partial = Partial {
        fd,
        bytes: [0; 4],
        index: 0,
        calls: 0,
    };
    let deadline = Deadline::new(Instant::now() - Duration::from_secs(1));
    assert_eq!(
        deadline.io(&mut partial).read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(
        deadline.io(&mut partial).write(b"G").unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(partial.calls, 0);
    assert_eq!(
        deadline.classify(NativeBackingRefusal::Io),
        NativeBackingRefusal::PreparationTimedOut
    );
}
#[test]
fn actual_nonblocking_closed_pipe_is_eof_not_a_timeout() {
    let (mut fd, peer) = UnixStream::pair().unwrap();
    super::super::nonblocking(&fd).unwrap();
    drop(peer);
    let deadline = future();
    assert_eq!(
        deadline
            .io(&mut fd)
            .read_exact(&mut [0])
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(
        deadline.classify(NativeBackingRefusal::Io),
        NativeBackingRefusal::Io
    );
}

#[test]
fn expired_preparation_guard_kills_and_reaps_its_owned_child() {
    use std::process::{Command, Stdio};
    let child = Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut guard = super::super::AdmissionChild { child };
    let pid = guard.child.id() as libc::pid_t;
    let mut input = guard.child.stdin.take().unwrap();
    super::super::nonblocking(&input).unwrap();
    let deadline = Deadline::new(Instant::now() - Duration::from_secs(1));
    assert_eq!(
        deadline.io(&mut input).write(b"G").unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    drop(input);
    drop(guard);
    let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    assert_eq!(result, -1);
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}
