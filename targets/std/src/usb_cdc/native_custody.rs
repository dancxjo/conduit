//! Local OS descriptor custody; never Body or physical-peer authority.
use super::{clear_dtr_fd, NativePathCdcLine, NativeUsbCdcError};

impl NativePathCdcLine {
    /// Provisioning claims the original descriptor before changing termios or
    /// modem control. Failed acquisition cannot configure another owner's TTY.
    pub(crate) fn open_exclusive_tty<P: AsRef<std::path::Path>>(
        path: P,
        maximum_frame_bytes: usize,
        controls_dtr: bool,
    ) -> Result<Self, NativeUsbCdcError> {
        Self::open_tty(path, maximum_frame_bytes, controls_dtr, true)
    }

    /// Claim this original descriptor, preserving its decoder and queued bytes.
    /// Uses cooperative flock and TTY exclusion; makes no modem-line changes.
    /// Existing noncooperating openers are not revoked, and privileged processes
    /// are outside this boundary. A forwarded TTY claim covers only the local
    /// endpoint, not its remote descriptor, physical device or peer identity.
    /// The claim lasts until this owner drops; it implies no Line Ready or Play.
    pub fn claim_local_exclusive(&mut self) -> Result<(), NativeUsbCdcError> {
        if self.local_exclusive {
            return Ok(());
        }
        if unsafe { libc::flock(self.fd.0, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(NativeUsbCdcError::TtyConfig(std::io::Error::last_os_error()));
        }
        if unsafe { libc::ioctl(self.fd.0, libc::TIOCEXCL as _) } != 0 {
            let error = std::io::Error::last_os_error();
            unsafe { libc::flock(self.fd.0, libc::LOCK_UN) };
            return Err(NativeUsbCdcError::TtyConfig(error));
        }
        self.local_exclusive = true;
        Ok(())
    }
}
impl Drop for NativePathCdcLine {
    fn drop(&mut self) {
        // Release only the exclusion this owner established. Descriptor close
        // releases its flock; no retry, flush or protocol retirement is implied.
        if self.local_exclusive {
            unsafe { libc::ioctl(self.fd.0, libc::TIOCNXCL as _) };
        }
        if self.controls_dtr {
            clear_dtr_fd(self.fd.0);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "exclusive_tests.rs"]
mod tests;
