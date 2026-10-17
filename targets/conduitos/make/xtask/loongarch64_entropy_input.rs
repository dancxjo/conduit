//! Fresh, finite Linux cryptographic input for the reviewed virtual machine.
//! The private file lives only while its guest runs; receipts never retain it.
use super::ConduitosError;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const BYTES: usize = 4096;
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Input(PathBuf);
impl Input {
    pub fn acquire(directory: &Path) -> Result<Self, ConduitosError> {
        let mut secret = Secret([0; BYTES]);
        fill(&mut secret.0).map_err(refusal)?;
        let nonce = NEXT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| refusal("input identity capacity exhausted"))?;
        let path = directory.join(format!(
            ".loongarch64-entropy-{}-{nonce}.bin",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(refusal)?;
        let input = Self(path);
        file.write_all(&secret.0).map_err(refusal)?;
        Ok(input)
    }
    pub fn argument(&self) -> Result<String, ConduitosError> {
        let path = self
            .0
            .to_str()
            .ok_or_else(|| refusal("non-UTF8 input path"))?;
        Ok(format!(
            "name=opt/conduitos/cryptographic-entropy@1,file={}",
            path.replace(',', ",,")
        ))
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct Secret([u8; BYTES]);
impl Drop for Secret {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}
#[cfg(target_os = "linux")]
fn fill(output: &mut [u8; BYTES]) -> io::Result<()> {
    let mut filled = 0;
    // Nonblocking acquisition refuses unavailable entropy. Short reads and
    // interruptions have a finite call budget, without any weaker substitute.
    for _ in 0..32 {
        let count = unsafe {
            libc::getrandom(
                output[filled..].as_mut_ptr().cast(),
                BYTES - filled,
                libc::GRND_NONBLOCK,
            )
        };
        if count > 0 {
            filled += count as usize;
            if filled == BYTES {
                return Ok(());
            }
        } else {
            if count == 0 {
                return Err(io::Error::other("getrandom produced no bytes"));
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    Err(io::Error::other(
        "cryptographic input call capacity exhausted",
    ))
}
#[cfg(not(target_os = "linux"))]
fn fill(_: &mut [u8; BYTES]) -> io::Result<()> {
    Err(io::Error::other(
        "this virtual entropy provider requires Linux getrandom",
    ))
}
fn refusal(detail: impl ToString) -> ConduitosError {
    ConduitosError::refusal(
        "loongarch64-cryptographic-input-unavailable",
        detail.to_string(),
    )
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn independent_inputs_are_private_fresh_and_removed_after_use() {
        use std::os::unix::fs::PermissionsExt;
        let first = Input::acquire(&std::env::temp_dir()).unwrap();
        let second = Input::acquire(&std::env::temp_dir()).unwrap();
        assert_ne!(first.0, second.0);
        assert_eq!(
            fs::metadata(&first.0).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::metadata(&first.0).unwrap().len(), BYTES as u64);
        assert!(fs::read(&first.0).unwrap() != fs::read(&second.0).unwrap());
        let path = first.0.clone();
        drop(first);
        assert!(!path.exists());
        assert!(second.0.exists());
    }
}
