//! Save exact prepared bytes through caller-admitted handles. No paths, device
//! acquisition, deployment, boot or join authority is supplied by this module.
use super::CrecheProducedImages;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Debug)]
pub enum CrecheSaveRefusal {
    UnknownHost,
    InvalidOutput,
    BoundExhausted,
    Cancelled,
    SourceChanged,
    ReadbackMismatch,
    Io(std::io::Error),
}
/// Only successful source verification, copying, synchronization and independent
/// readback construct this receipt. It records a saved artifact, not deployment.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CrecheSavedReceipt {
    schema: &'static str,
    request: [u8; 32],
    body_id: String,
    host_entry: String,
    target: String,
    artifact_sha256: String,
    artifact_bytes: u64,
    artifact_saved: bool,
    carrier_realized: bool,
    boot_observed: bool,
    join_observed: bool,
}
impl CrecheSavedReceipt {
    pub fn request(&self) -> [u8; 32] {
        self.request
    }
    pub fn body_id(&self) -> &str {
        &self.body_id
    }
    pub fn host_entry(&self) -> &str {
        &self.host_entry
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn artifact_sha256(&self) -> &str {
        &self.artifact_sha256
    }
    pub fn artifact_bytes(&self) -> u64 {
        self.artifact_bytes
    }
}
impl CrecheProducedImages {
    /// The caller already owns save authority for this empty read/write regular
    /// file. On refusal its handle and any partial bytes remain caller-owned.
    /// Concurrent file writers are not confined; verification covers these reads.
    pub fn save_host(
        &mut self,
        host: &str,
        output: &mut File,
        maximum_bytes: u64,
        cancelled: &AtomicBool,
    ) -> Result<CrecheSavedReceipt, CrecheSaveRefusal> {
        self.save_host_checked(host, output, maximum_bytes, &mut || {
            if cancelled.load(Ordering::Acquire) {
                Err(CrecheSaveRefusal::Cancelled)
            } else {
                Ok(())
            }
        })
    }
    fn save_host_checked(
        &mut self,
        host: &str,
        output: &mut File,
        maximum_bytes: u64,
        check: &mut impl FnMut() -> Result<(), CrecheSaveRefusal>,
    ) -> Result<CrecheSavedReceipt, CrecheSaveRefusal> {
        use CrecheSaveRefusal as E;
        check()?;
        let artifact = self
            .job
            .artifacts
            .iter_mut()
            .find(|a| a.receipt.host_entry == host)
            .ok_or(E::UnknownHost)?;
        let receipt = &artifact.receipt;
        if receipt.artifact_bytes == 0 || receipt.artifact_bytes > maximum_bytes {
            return Err(E::BoundExhausted);
        }
        let metadata = output.metadata().map_err(E::Io)?;
        if !metadata.is_file() || metadata.len() != 0 {
            return Err(E::InvalidOutput);
        }
        output.seek(SeekFrom::Start(0)).map_err(E::Io)?;
        // One-byte read on known empty media validates readable access; truncating
        // to its already-verified zero length validates writable access.
        #[cfg(unix)]
        let readable = std::os::unix::fs::FileExt::read_at(output, &mut [0u8; 1], 0);
        #[cfg(not(unix))]
        let readable = output.read(&mut [0u8; 1]);
        if !matches!(readable, Ok(0)) || output.set_len(0).is_err() {
            return Err(E::InvalidOutput);
        }
        verify_source(
            &mut artifact.file,
            receipt.artifact_bytes,
            &receipt.artifact_sha256,
            check,
        )?;
        artifact.file.seek(SeekFrom::Start(0)).map_err(E::Io)?;
        output.seek(SeekFrom::Start(0)).map_err(E::Io)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            check()?;
            let n = artifact.file.read(&mut buffer).map_err(E::Io)?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .filter(|n| *n <= receipt.artifact_bytes)
                .ok_or(E::SourceChanged)?;
            output.write_all(&buffer[..n]).map_err(E::Io)?;
            hash.update(&buffer[..n]);
        }
        if bytes != receipt.artifact_bytes
            || format!("sha256:{:x}", hash.finalize()) != receipt.artifact_sha256
        {
            return Err(E::SourceChanged);
        }
        check()?;
        output.flush().map_err(E::Io)?;
        output.sync_all().map_err(E::Io)?;
        if hash_file(output, receipt.artifact_bytes, check)? != receipt.artifact_sha256 {
            return Err(E::ReadbackMismatch);
        }
        // Detect source mutation during copying or output verification, including an
        // already-copied source byte; do not infer custody from output digest alone.
        verify_source(
            &mut artifact.file,
            receipt.artifact_bytes,
            &receipt.artifact_sha256,
            check,
        )?;
        check()?;
        Ok(CrecheSavedReceipt {
            schema: "conduit.creche/saved-image-receipt@1",
            request: self.job.request.0,
            body_id: self.job.prepared.birth().evidence().body_id.as_str().into(),
            host_entry: host.into(),
            target: receipt.target.clone(),
            artifact_sha256: receipt.artifact_sha256.clone(),
            artifact_bytes: bytes,
            artifact_saved: true,
            carrier_realized: false,
            boot_observed: false,
            join_observed: false,
        })
    }
}
fn verify_source(
    file: &mut File,
    bytes: u64,
    expected: &str,
    check: &mut impl FnMut() -> Result<(), CrecheSaveRefusal>,
) -> Result<(), CrecheSaveRefusal> {
    if hash_file(file, bytes, check)? != expected {
        return Err(CrecheSaveRefusal::SourceChanged);
    }
    Ok(())
}
fn hash_file(
    file: &mut File,
    expected_bytes: u64,
    check: &mut impl FnMut() -> Result<(), CrecheSaveRefusal>,
) -> Result<String, CrecheSaveRefusal> {
    use CrecheSaveRefusal as E;
    let metadata = file.metadata().map_err(E::Io)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(E::SourceChanged);
    }
    file.seek(SeekFrom::Start(0)).map_err(E::Io)?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        check()?;
        let n = file.read(&mut buffer).map_err(E::Io)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .filter(|n| *n <= expected_bytes)
            .ok_or(E::SourceChanged)?;
        hash.update(&buffer[..n]);
    }
    if bytes != expected_bytes {
        return Err(E::SourceChanged);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}
#[cfg(test)]
mod tests;
