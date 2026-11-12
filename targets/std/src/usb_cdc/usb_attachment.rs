//! Read current kernel attachment facts from the original claimed descriptor.
use super::{NativePathCdcLine, NativeUsbCdcError};
use std::{
    io::{self, Read},
    path::Path,
};
fn refusal(message: &str) -> NativeUsbCdcError {
    NativeUsbCdcError::TtyConfig(io::Error::new(io::ErrorKind::InvalidInput, message))
}
impl NativePathCdcLine {
    /// Match an independently installed USB serial pin on THIS descriptor.
    /// This checks local kernel attachment, not hardware authenticity, Body
    /// membership, peer key, installed firmware, Line Ready or execution grants.
    /// Forwarded TTYs deliberately cannot satisfy this direct-USB contract.
    /// No descriptor reopen, modem operation or framing read occurs here.
    pub fn verify_local_usb_serial(&self, expected: &str) -> Result<(), NativeUsbCdcError> {
        if !self.local_exclusive || !self.controls_dtr {
            return Err(refusal(
                "direct USB verification requires original local claimed carrier",
            ));
        }
        if !self.is_connected()? {
            return Err(NativeUsbCdcError::Disconnected);
        }
        let mut info: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(self.fd.0, &mut info) } != 0 {
            return Err(NativeUsbCdcError::TtyConfig(io::Error::last_os_error()));
        }
        if info.st_mode & libc::S_IFMT != libc::S_IFCHR {
            return Err(refusal("original carrier is not a character device"));
        }
        let path = std::path::PathBuf::from(format!(
            "/sys/dev/char/{}:{}/device",
            libc::major(info.st_rdev),
            libc::minor(info.st_rdev)
        ));
        verify_attachment(&path, expected)
    }
}
fn verify_attachment(device: &Path, expected: &str) -> Result<(), NativeUsbCdcError> {
    if expected.is_empty() || expected.len() > 128 || expected.chars().any(char::is_control) {
        return Err(refusal(
            "USB serial pin is empty, unbounded or contains control characters",
        ));
    }
    let device = device
        .canonicalize()
        .map_err(NativeUsbCdcError::TtyConfig)?;
    for parent in device.ancestors().take(32) {
        if parent.join("idVendor").is_file() && parent.join("idProduct").is_file() {
            let mut input =
                std::fs::File::open(parent.join("serial")).map_err(NativeUsbCdcError::TtyConfig)?;
            let mut bytes = [0; 130];
            let mut count = 0;
            while count < bytes.len() {
                let n = input
                    .read(&mut bytes[count..])
                    .map_err(NativeUsbCdcError::TtyConfig)?;
                if n == 0 {
                    break;
                }
                count += n;
            }
            if count == bytes.len() {
                return Err(refusal("USB serial attribute exceeds finite extent"));
            }
            let actual = std::str::from_utf8(&bytes[..count])
                .map_err(|_| refusal("USB serial is not UTF-8"))?;
            if actual.strip_suffix('\n').unwrap_or(actual) != expected {
                return Err(refusal(
                    "current kernel USB serial differs from installed pin",
                ));
            }
            return Ok(());
        }
    }
    Err(refusal(
        "original descriptor has no bounded USB parent attachment",
    ))
}
#[cfg(test)]
#[path = "usb_attachment_tests.rs"]
mod tests;
