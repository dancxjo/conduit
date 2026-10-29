//! Optional checked-provider identity, retained by the selected playback Back.
use super::{HostedPlaybackSelection, PlaybackFailure};
use sha2::{Digest, Sha256};
use std::io::Read;

impl HostedPlaybackSelection {
    /// Bind an explicitly checked provider digest. Legacy selections remain unchanged.
    pub fn with_executable_sha256(mut self, digest: String) -> Result<Self, String> {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("playback executable requires an exact lowercase SHA-256".into());
        }
        let mut bytes = [0u8; 32];
        for (index, pair) in digest.as_bytes().chunks_exact(2).enumerate() {
            let nibble = |b: u8| {
                if b.is_ascii_digit() {
                    b - b'0'
                } else {
                    b - b'a' + 10
                }
            };
            bytes[index] = nibble(pair[0]) * 16 + nibble(pair[1]);
        }
        self.executable_sha256 = Some(bytes);
        Ok(self)
    }

    pub(super) fn verify_executable_identity(&self) -> Result<(), PlaybackFailure> {
        let Some(expected) = &self.executable_sha256 else {
            return Ok(());
        };
        verify_file(std::path::Path::new("/usr/bin/aplay"), expected)
    }
}

fn verify_file(path: &std::path::Path, expected: &[u8; 32]) -> Result<(), PlaybackFailure> {
    const MAXIMUM_BYTES: u64 = 8 * 1024 * 1024;
    let mut file = std::fs::File::open(path).map_err(|_| PlaybackFailure::ProviderLost)?;
    if file
        .metadata()
        .map_err(|_| PlaybackFailure::ProviderLost)?
        .len()
        > MAXIMUM_BYTES
    {
        return Err(PlaybackFailure::ProviderLost);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut remaining = MAXIMUM_BYTES + 1;
    loop {
        let limit = buffer.len().min(remaining as usize);
        let n = file
            .read(&mut buffer[..limit])
            .map_err(|_| PlaybackFailure::ProviderLost)?;
        if n == 0 {
            break;
        }
        remaining -= n as u64;
        if remaining == 0 {
            return Err(PlaybackFailure::ProviderLost);
        }
        hasher.update(&buffer[..n]);
    }
    let observed: [u8; 32] = hasher.finalize().into();
    if observed != *expected {
        return Err(PlaybackFailure::ProviderLost);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_drift_and_missing_provider_refuse_without_executing() {
        let path =
            std::env::temp_dir().join(format!("conduit-playback-digest-{}", std::process::id()));
        std::fs::write(&path, b"metadata-only provider fixture").unwrap();
        let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).unwrap()).into();
        verify_file(&path, &digest).unwrap();
        std::fs::write(&path, b"changed provider").unwrap();
        assert_eq!(
            verify_file(&path, &digest),
            Err(PlaybackFailure::ProviderLost)
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            verify_file(&path, &digest),
            Err(PlaybackFailure::ProviderLost)
        );
    }
}
