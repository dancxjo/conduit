//! A local producer owns semantic validity and freshness. This lease prevents a
//! dead producer's last Face remaining perceptible as a current live view.
use conduit_face::Face;
use serde::Deserialize;
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
const MAX_BYTES: u64 = 600 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publication {
    schema: String,
    boot_id: String,
    valid_until_boot_ms: u64,
    face: Face,
}

pub fn now_ms() -> Result<u64, String> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) } != 0 {
        return Err("boot clock unavailable".into());
    }
    Ok(time.tv_sec as u64 * 1000 + time.tv_nsec as u64 / 1_000_000)
}
fn validate(p: Publication, boot: &str, now: u64) -> Result<Face, String> {
    if p.schema != "conduit.local-face/leased@1" || p.boot_id != boot {
        return Err("Waiting for this boot's environmental Face".into());
    }
    if p.valid_until_boot_ms <= now || p.valid_until_boot_ms > now.saturating_add(30_000) {
        return Err("Environmental Face producer unavailable".into());
    }
    p.face
        .validate()
        .map_err(|_| "Environmental Face refused: invalid content".to_string())?;
    Ok(p.face)
}
pub fn read(path: &Path, boot: &str) -> Result<Face, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Waiting for the shared environmental Face".to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o600
        || metadata.len() > MAX_BYTES
    {
        return Err(
            "Environmental Face file refused: private bounded regular file required".into(),
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Environmental Face file exceeded bound".into());
    }
    let p: Publication = serde_json::from_slice(&bytes)
        .map_err(|_| "Environmental Face file refused: invalid publication".to_string())?;
    validate(p, boot, now_ms()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn publication() -> Publication {
        Publication {
            schema: "conduit.local-face/leased@1".into(),
            boot_id: "this-boot".into(),
            valid_until_boot_ms: 5000,
            face: crate::view::unavailable("Waiting"),
        }
    }
    #[test]
    fn preserves_the_exact_face() {
        let p = publication();
        let expected = p.face.clone();
        assert_eq!(validate(p, "this-boot", 100).unwrap(), expected);
    }
    #[test]
    fn reboot_expiry_and_unbounded_lease_refuse() {
        assert!(validate(publication(), "new-boot", 100).is_err());
        assert!(validate(publication(), "this-boot", 5000).is_err());
        let mut p = publication();
        p.valid_until_boot_ms = 40000;
        assert!(validate(p, "this-boot", 100).is_err());
    }
}
