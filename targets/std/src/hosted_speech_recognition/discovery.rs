//! Exact bounded installed executable and model identity, before adapter preparation.
use super::WhisperFailure;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
const MAXIMUM_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_MODEL_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperDiscovery {
    pub(super) executable: PathBuf,
    pub(super) model: PathBuf,
    pub executable_sha256: String,
    pub model_sha256: String,
    pub model_bytes: u64,
    pub(super) coverage: Option<conduit_language::LanguageCoverage>,
}

impl WhisperDiscovery {
    /// Exact installed source identity; no model-name language inference.
    pub fn provider_identity(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"conduit-whisper-provider-v1\0");
        digest.update(self.executable_sha256.as_bytes());
        digest.update(self.model_sha256.as_bytes());
        format!("whisper/provider/{:x}", digest.finalize())
    }

    /// Supply reviewed Language metadata for this exact executable/model pair.
    /// A bare discovery advertises no Language coverage.
    pub fn declare_language_coverage(
        mut self,
        coverage: conduit_language::LanguageCoverage,
    ) -> Result<Self, WhisperFailure> {
        crate::hosted_language::declare(&self.provider_identity(), &coverage)
            .map_err(WhisperFailure::Language)?;
        self.coverage = Some(coverage);
        Ok(self)
    }

    pub(super) fn verify(&self) -> Result<(), WhisperFailure> {
        let current = Self::inspect(&self.executable, &self.model)
            .map_err(|_| WhisperFailure::ProviderChanged)?;
        if current.executable_sha256 != self.executable_sha256
            || current.model_sha256 != self.model_sha256
            || current.model_bytes != self.model_bytes
        {
            return Err(WhisperFailure::ProviderChanged);
        }
        Ok(())
    }

    pub fn inspect(
        executable: impl AsRef<Path>,
        model: impl AsRef<Path>,
    ) -> Result<Self, WhisperFailure> {
        let executable = exact_file(executable.as_ref(), MAXIMUM_EXECUTABLE_BYTES)?;
        let model = exact_file(model.as_ref(), MAXIMUM_MODEL_BYTES)?;
        if !is_executable(&executable)? {
            return Err(WhisperFailure::InvalidProvider);
        }
        let model_bytes = model
            .metadata()
            .map_err(|_| WhisperFailure::InvalidProvider)?
            .len();
        Ok(Self {
            executable_sha256: digest_file(&executable, MAXIMUM_EXECUTABLE_BYTES)?,
            model_sha256: digest_file(&model, MAXIMUM_MODEL_BYTES)?,
            model_bytes,
            executable,
            model,
            coverage: None,
        })
    }
}

fn exact_file(path: &Path, maximum: u64) -> Result<PathBuf, WhisperFailure> {
    let path = path
        .canonicalize()
        .map_err(|_| WhisperFailure::MissingProvider)?;
    let metadata = path
        .metadata()
        .map_err(|_| WhisperFailure::MissingProvider)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(WhisperFailure::InvalidProvider);
    }
    Ok(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool, WhisperFailure> {
    use std::os::unix::fs::PermissionsExt;
    Ok(path
        .metadata()
        .map_err(|_| WhisperFailure::InvalidProvider)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool, WhisperFailure> {
    Ok(false)
}

fn digest_file(path: &Path, maximum: u64) -> Result<String, WhisperFailure> {
    let file = File::open(path).map_err(|_| WhisperFailure::InvalidProvider)?;
    let expected = file
        .metadata()
        .map_err(|_| WhisperFailure::InvalidProvider)?
        .len();
    if expected == 0 || expected > maximum {
        return Err(WhisperFailure::InvalidProvider);
    }
    digest_reader(file, expected, maximum)
}
fn digest_reader(file: impl Read, expected: u64, maximum: u64) -> Result<String, WhisperFailure> {
    let mut file = file.take(maximum + 1);
    let mut total = 0u64;
    let mut digest = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut bytes)
            .map_err(|_| WhisperFailure::InvalidProvider)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > maximum {
            return Err(WhisperFailure::InvalidProvider);
        }
        digest.update(&bytes[..read]);
    }
    if total != expected {
        return Err(WhisperFailure::ProviderChanged);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_or_unending_provider_reads_refuse_within_the_byte_budget() {
        assert_eq!(
            digest_reader(std::io::repeat(0), 4, 8),
            Err(WhisperFailure::InvalidProvider)
        );
        assert_eq!(
            digest_reader(&b"short"[..], 8, 8),
            Err(WhisperFailure::ProviderChanged)
        );
        assert!(digest_reader(&b"exact"[..], 5, 8).is_ok());
    }
}
