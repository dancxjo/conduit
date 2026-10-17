//! Explicit host-local durable storage. No ambient corpus or artifact discovery.
use crate::adapter::hex;
use crate::{Cancellation, Error};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlobRef {
    pub identity: [u8; 32],
    pub bytes: u64,
}
impl BlobRef {
    pub fn of(data: &[u8]) -> Self {
        Self {
            identity: digest(data),
            bytes: data.len() as u64,
        }
    }
}

/// The caller admits this exact directory, retention and object bound during
/// host preparation. This library is cooperative storage, not filesystem confinement.
pub struct DirectoryCheckpointStore {
    root: PathBuf,
    maximum_bytes: u64,
    maximum_snapshots: usize,
    publication: Mutex<()>,
}

impl DirectoryCheckpointStore {
    pub fn new(
        root: impl AsRef<Path>,
        maximum_bytes: u64,
        maximum_snapshots: usize,
    ) -> Result<Self, Error> {
        if maximum_bytes == 0 || maximum_snapshots == 0 || maximum_snapshots > 4096 {
            return Err(Error::ResourceBound);
        }
        fs::create_dir_all(root.as_ref()).map_err(io)?;
        let root = fs::canonicalize(root).map_err(io)?;
        Ok(Self {
            root,
            maximum_bytes,
            maximum_snapshots,
            publication: Mutex::new(()),
        })
    }
    pub fn descriptor_path(&self, id: &[u8; 32]) -> PathBuf {
        self.root.join(format!("{}.json", hex(id)))
    }
    pub fn latest_identity(&self) -> Result<[u8; 32], Error> {
        let bytes = read_limited(&self.root.join("latest.json"), 1024)?;
        let pointer: Pointer =
            serde_json::from_slice(&bytes).map_err(|_| Error::CorruptCheckpoint)?;
        if pointer.schema != 1 || pointer.identity == [0; 32] {
            return Err(Error::CorruptCheckpoint);
        }
        // A pointer is only usable if its immutable descriptor is present and verified.
        self.read_descriptor(&pointer.identity)?;
        Ok(pointer.identity)
    }
    pub(crate) fn read_descriptor(&self, id: &[u8; 32]) -> Result<Vec<u8>, Error> {
        let bytes = read_limited(&self.descriptor_path(id), self.maximum_bytes.min(64 * 1024))?;
        if digest(&bytes) != *id {
            return Err(Error::CorruptCheckpoint);
        }
        Ok(bytes)
    }
    pub(crate) fn read_blob(&self, blob: &BlobRef) -> Result<Vec<u8>, Error> {
        if blob.bytes > self.maximum_bytes {
            return Err(Error::ResourceBound);
        }
        let bytes = read_limited(
            &self.root.join(format!("{}.blob", hex(&blob.identity))),
            blob.bytes,
        )?;
        if bytes.len() as u64 != blob.bytes || digest(&bytes) != blob.identity {
            return Err(Error::CorruptCheckpoint);
        }
        Ok(bytes)
    }
    pub(crate) fn publish(
        &self,
        descriptor: &[u8],
        blobs: &[&[u8]],
        cancel: &Cancellation,
    ) -> Result<[u8; 32], Error> {
        let _guard = self
            .publication
            .lock()
            .map_err(|_| Error::CheckpointIo("publication lock poisoned".into()))?;
        let total = blobs.iter().try_fold(descriptor.len() as u64, |n, b| {
            n.checked_add(b.len() as u64).ok_or(Error::ResourceBound)
        })?;
        if total > self.maximum_bytes || descriptor.len() > 64 * 1024 {
            return Err(Error::ResourceBound);
        }
        let id = digest(descriptor);
        // Bound retention before allocating more durable storage; no silent pruning.
        let mut snapshots = 0usize;
        let mut entries = 0usize;
        for entry in fs::read_dir(&self.root).map_err(io)? {
            let entry = entry.map_err(io)?;
            entries += 1;
            if entries > self.maximum_snapshots.saturating_mul(5).saturating_add(16) {
                return Err(Error::ResourceBound);
            }
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".json") && name != "latest.json" {
                snapshots += 1;
            }
        }
        if snapshots >= self.maximum_snapshots && !self.descriptor_path(&id).exists() {
            return Err(Error::ResourceBound);
        }
        for blob in blobs {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            immutable_write(
                &self.root.join(format!("{}.blob", hex(&digest(blob)))),
                blob,
            )?;
        }
        immutable_write(&self.descriptor_path(&id), descriptor)?;
        sync_directory(&self.root)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let pointer = serde_json::to_vec(&Pointer {
            schema: 1,
            identity: id,
        })
        .map_err(|_| Error::InvalidDescriptor)?;
        let temporary = self.root.join(format!(
            ".latest-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(io)?;
            file.write_all(&pointer).map_err(io)?;
            file.sync_all().map_err(io)?;
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let _publication = cancel.commit_guard()?;
            fs::rename(&temporary, self.root.join("latest.json")).map_err(io)?;
            // If this sync fails, publication may be visible; the caller must inspect latest.
            sync_directory(&self.root).map_err(|_| Error::DurabilityUncertain(id))?;
            Ok(id)
        })();
        if temporary.exists() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer {
    schema: u32,
    identity: [u8; 32],
}

fn immutable_write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if path.exists() {
        let prior = read_limited(path, bytes.len() as u64)?;
        return if prior == bytes {
            Ok(())
        } else {
            Err(Error::CorruptCheckpoint)
        };
    }
    let temporary = path.with_extension(format!(
        "{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let prior = read_limited(path, bytes.len() as u64)?;
                if prior == bytes {
                    Ok(())
                } else {
                    Err(Error::CorruptCheckpoint)
                }
            }
            Err(error) => Err(io(error)),
        }
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn read_limited(path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path).map_err(io)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(Error::CorruptCheckpoint);
    }
    let file = File::open(path).map_err(io)?;
    let mut bytes = Vec::new();
    file.take(maximum.checked_add(1).ok_or(Error::ResourceBound)?)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > maximum {
        return Err(Error::CorruptCheckpoint);
    }
    Ok(bytes)
}
fn sync_directory(path: &Path) -> Result<(), Error> {
    File::open(path).and_then(|f| f.sync_all()).map_err(io)
}
pub(crate) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn io(error: std::io::Error) -> Error {
    Error::CheckpointIo(error.to_string())
}
