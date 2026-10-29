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
    validate_view(p, boot, now, false)
}
fn validate_view(p: Publication, boot: &str, now: u64, historical: bool) -> Result<Face, String> {
    if p.schema != "conduit.local-face/leased@1" || p.boot_id != boot {
        return Err("Waiting for this boot's environmental Face".into());
    }
    if p.valid_until_boot_ms == 0
        || (!historical && p.valid_until_boot_ms <= now)
        || p.valid_until_boot_ms > now.saturating_add(30_000)
    {
        return Err("Environmental Face producer unavailable".into());
    }
    p.face
        .validate()
        .map_err(|_| "Environmental Face refused: invalid content".to_string())?;
    Ok(p.face)
}
pub fn read(path: &Path, boot: &str) -> Result<Face, String> {
    read_view(path, boot, false)
}
/// Recover only a privately published same-Linux-Boot validated historical view.
/// This does not grant a current producer lease or enable actions.
pub fn read_last_known(path: &Path, boot: &str) -> Result<Face, String> {
    read_view(path, boot, true)
}
fn read_view(path: &Path, boot: &str, historical: bool) -> Result<Face, String> {
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
    let now = now_ms()?;
    if historical {
        validate_view(p, boot, now, true)
    } else {
        validate(p, boot, now)
    }
}

/// One actually accepted same-Boot view. Retention grants no current lease or actions.
#[derive(Default)]
pub struct RetainedView {
    accepted: Option<(Face, u64)>,
}
impl RetainedView {
    pub fn accept(&mut self, face: Face, now: u64) {
        self.accepted = Some((face, now));
    }
    pub fn last(&self, now: u64) -> Option<(&Face, u64)> {
        self.accepted
            .as_ref()
            .and_then(|(face, at)| now.checked_sub(*at).map(|age| (face, age)))
    }
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

#[cfg(test)]
mod retained_tests {
    use super::*;
    #[test]
    fn retention_has_no_startup_value_and_preserves_exact_accepted_identity() {
        let mut retained = RetainedView::default();
        assert!(retained.last(100).is_none());
        let face = crate::view::unavailable("actual accepted fixture");
        retained.accept(face.clone(), 100);
        assert!(retained.last(99).is_none());
        let (last, age) = retained.last(6000).unwrap();
        assert_eq!(last, &face);
        assert_eq!(age, 5900);
    }
}

#[cfg(test)]
mod historical_startup_tests {
    use super::*;
    fn publication() -> Publication {
        Publication {
            schema: "conduit.local-face/leased@1".into(),
            boot_id: "same-linux-boot".into(),
            valid_until_boot_ms: 50,
            face: crate::view::unavailable("accepted historical fixture"),
        }
    }
    #[test]
    fn startup_history_accepts_expiry_but_never_foreign_boot_or_unbounded_future() {
        assert!(validate_view(publication(), "same-linux-boot", 100, false).is_err());
        assert!(validate_view(publication(), "same-linux-boot", 100, true).is_ok());
        assert!(validate_view(publication(), "foreign-linux-boot", 100, true).is_err());
        let mut unissued = publication();
        unissued.valid_until_boot_ms = 0;
        assert!(validate_view(unissued, "same-linux-boot", 100, true).is_err());
        let mut future = publication();
        future.valid_until_boot_ms = 100_000;
        assert!(validate_view(future, "same-linux-boot", 100, true).is_err());
    }
}
