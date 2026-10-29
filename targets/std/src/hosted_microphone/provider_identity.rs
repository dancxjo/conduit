//! Exact canonical provider identity and byte-bounded executable hashing.
use super::{MicrophoneFailure, MAXIMUM_EXECUTABLE_BYTES};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn exact_file(path: &Path) -> Result<PathBuf, MicrophoneFailure> {
    let path = path
        .canonicalize()
        .map_err(|_| MicrophoneFailure::MissingProvider)?;
    let metadata = path
        .metadata()
        .map_err(|_| MicrophoneFailure::MissingProvider)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_EXECUTABLE_BYTES {
        return Err(MicrophoneFailure::InvalidProvider);
    }
    if !is_executable(&path)? {
        return Err(MicrophoneFailure::InvalidProvider);
    }
    Ok(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool, MicrophoneFailure> {
    use std::os::unix::fs::PermissionsExt;
    Ok(path
        .metadata()
        .map_err(|_| MicrophoneFailure::InvalidProvider)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool, MicrophoneFailure> {
    Ok(false)
}

pub(super) fn digest_file(path: &Path) -> Result<String, MicrophoneFailure> {
    let file = std::fs::File::open(path).map_err(|_| MicrophoneFailure::InvalidProvider)?;
    if file
        .metadata()
        .map_err(|_| MicrophoneFailure::InvalidProvider)?
        .len()
        > MAXIMUM_EXECUTABLE_BYTES
    {
        return Err(MicrophoneFailure::InvalidProvider);
    }
    let mut file = file.take(MAXIMUM_EXECUTABLE_BYTES + 1);
    let mut observed = 0u64;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| MicrophoneFailure::InvalidProvider)?;
        if count == 0 {
            break;
        }
        observed += count as u64;
        if observed > MAXIMUM_EXECUTABLE_BYTES {
            return Err(MicrophoneFailure::InvalidProvider);
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
