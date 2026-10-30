//! Deadline-aware finite pipe custody, including EOF after provider exit.
use super::*;
use std::process::ExitStatus;

pub(super) fn collect(
    child: &mut Child,
    output_limit: usize,
    diagnostic_limit: usize,
    started: Instant,
    timeout: Duration,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(ExitStatus, BoundedRead, BoundedRead), MicrophoneFailure> {
    let result = (|| {
        let mut output = child.stdout.take().ok_or(MicrophoneFailure::SpawnFailed)?;
        let mut diagnostics = child.stderr.take().ok_or(MicrophoneFailure::SpawnFailed)?;
        nonblocking(&output)?;
        nonblocking(&diagnostics)?;
        let mut audio = Pipe::new(output_limit);
        let mut diagnostic = Pipe::new(diagnostic_limit);
        let mut status = None;
        loop {
            if cancelled() {
                return Err(MicrophoneFailure::Cancelled);
            }
            if started.elapsed() >= timeout {
                return Err(MicrophoneFailure::Timeout);
            }
            // One bounded read from each pipe per poll: a noisy writer cannot
            // starve the cancellation or deadline checks.
            audio.poll(&mut output)?;
            diagnostic.poll(&mut diagnostics)?;
            if status.is_none() {
                status = child
                    .try_wait()
                    .map_err(|_| MicrophoneFailure::ProviderLost)?;
            }
            if let Some(status) = status {
                if audio.eof && diagnostic.eof {
                    return Ok((status, audio.value, diagnostic.value));
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    if result.is_err() {
        stop(child);
    }
    result
}

struct Pipe {
    value: BoundedRead,
    maximum: usize,
    eof: bool,
}
impl Pipe {
    fn new(maximum: usize) -> Self {
        Self {
            value: BoundedRead {
                bytes: Vec::with_capacity(maximum),
                overflowed: false,
            },
            maximum,
            eof: false,
        }
    }
    fn poll(&mut self, reader: &mut impl Read) -> Result<(), MicrophoneFailure> {
        if self.eof {
            return Ok(());
        }
        let mut buffer = [0_u8; 4096];
        match reader.read(&mut buffer) {
            Ok(0) => self.eof = true,
            Ok(count) => {
                let remaining = self.maximum.saturating_sub(self.value.bytes.len());
                if count > remaining {
                    return Err(MicrophoneFailure::OutputOverflow);
                }
                self.value.bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(MicrophoneFailure::ReadFailed),
        }
        Ok(())
    }
}

#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> Result<(), MicrophoneFailure> {
    let fd = pipe.as_raw_fd();
    // SAFETY: the owned live pipe supplies a valid descriptor; fcntl only
    // inspects/updates its flags, retaining ownership with the Child stream.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(MicrophoneFailure::ReadFailed);
    }
    Ok(())
}

#[cfg(not(unix))]
fn nonblocking<T>(_: &T) -> Result<(), MicrophoneFailure> {
    // ALSA capture has no supported non-Unix pipe implementation. Refuse before
    // reading rather than entering an unbounded blocking operation.
    Err(MicrophoneFailure::InvalidProvider)
}
