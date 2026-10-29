//! Verify a bounded native observation projection before durable owner custody.
//! This is an outgoing publication replay profile, not a general Body store.
use crate::{checked, planning::Prepared, Result};
use conduit_environment_runtime::observed_record::{self, ObservedRecord};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    time::Duration,
};
#[allow(dead_code)] // The shared native profile also supplies per-record/server APIs.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
pub(crate) mod wire;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    profile: String,
    scope: String,
    forms: String,
    after: u32,
    committed: u32,
    records: String,
    signature: String,
}
fn hex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("history hex encoding".into());
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
        .collect()
}
pub struct Collector {
    scope: [u8; 96],
    forms: [u8; 32],
    key: VerifyingKey,
    pub after: u32,
    observed_ms: u64,
}
/// Durability includes every newly created directory entry, not only its files.
pub(crate) fn create_durable_directory(directory: &Path) -> Result<()> {
    if directory.exists() {
        return Ok(());
    }
    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_durable_directory(parent)?;
    std::fs::DirBuilder::new().mode(0o700).create(directory)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
impl Collector {
    pub fn new(prepared: &Prepared, key: &[u8; 32]) -> Result<Self> {
        let mut scope = [0; 96];
        for (out, text) in scope.as_chunks_mut::<32>().0.iter_mut().zip([
            prepared.body_plan.body_id.as_str(),
            prepared.active.boot_id.as_str(),
            prepared.active.active_play_id.as_str(),
        ]) {
            out.copy_from_slice(&Sha256::digest(text.as_bytes()));
        }
        Self::from_authenticated_scope(scope, key)
    }
    /// The caller must obtain this exact Body/Boot/Play scope and producer key
    /// through authenticated membership admission, never from a history reply.
    pub fn from_authenticated_scope(scope: [u8; 96], key: &[u8; 32]) -> Result<Self> {
        let contract =
            checked(conduit_environment_runtime::observation::ObservationContract::prepare())?;
        let forms = Sha256::digest(checked(observed_record::descriptor(&contract))?).into();
        Ok(Self {
            scope,
            forms,
            key: checked(VerifyingKey::from_bytes(key))?,
            after: 0,
            observed_ms: 0,
        })
    }
    /// Restore only a complete, signed, contiguous durable prefix. Failure leaves
    /// this collector unchanged. Temporary files are never custody receipts.
    pub fn restore(&mut self, directory: &Path) -> Result<()> {
        let (after, clock, _) = self.durable_prefix(directory, false)?;
        self.after = after;
        self.observed_ms = clock;
        Ok(())
    }
    /// Reverify the complete immutable signed prefix and fsync its actual file
    /// inodes and directory entries before a consumer may sign custody. The
    /// canonical collector creates/syncs all missing ancestor directories.
    #[allow(dead_code)] // Typed custody remains dormant until the native offer route is proven.
    pub(crate) fn durable_summary(
        &self,
        directory: &Path,
        forms: &[u8; 32],
    ) -> Result<(u32, [u8; 32])> {
        if forms != &self.forms {
            return Err("custody Forms differ".into());
        }
        let (count, _, digest) = self.durable_prefix(directory, true)?;
        Ok((count, digest))
    }
    fn durable_prefix(&self, directory: &Path, sync: bool) -> Result<(u32, u64, [u8; 32])> {
        let mut replay = Self {
            after: 0,
            observed_ms: 0,
            ..*self
        };
        let mut digest = Sha256::new();
        if !directory.exists() {
            return Ok((0, 0, digest.finalize().into()));
        }
        let mut paths = std::fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("observed-history-") && name.ends_with(".json")
            })
        });
        paths.sort();
        for path in paths {
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&path)?;
            let mut bytes = Vec::new();
            (&mut file).take(2049).read_to_end(&mut bytes)?;
            let (after, clock) = replay.validate(&bytes)?;
            if after == replay.after
                || path.file_name().unwrap()
                    != std::ffi::OsStr::new(&format!(
                        "observed-history-{:04}-{after:04}.json",
                        replay.after
                    ))
            {
                return Err("history custody filename or empty batch".into());
            }
            let batch: Batch = serde_json::from_slice(&bytes)?;
            digest.update(hex(&batch.records)?);
            if sync {
                file.sync_all()?;
            }
            replay.after = after;
            replay.observed_ms = clock;
        }
        if sync {
            File::open(directory)?.sync_all()?;
            File::open(
                directory
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new(".")),
            )?
            .sync_all()?;
        }
        Ok((replay.after, replay.observed_ms, digest.finalize().into()))
    }
    fn validate(&self, bytes: &[u8]) -> Result<(u32, u64)> {
        if bytes.len() > 2048 {
            return Err("history batch bound".into());
        }
        let batch: Batch = serde_json::from_slice(bytes)?;
        let records = hex(&batch.records)?;
        if batch.profile != "observed-history@1"
            || hex(&batch.scope)? != self.scope
            || hex(&batch.forms)? != self.forms
            || batch.after != self.after
            || batch.committed > 960
            || batch.committed < self.after
            || records.len() > 4 * observed_record::BYTES
            || !records.len().is_multiple_of(observed_record::BYTES)
        {
            return Err("history scope, cursor or extent".into());
        }
        let expected = ((batch.committed - self.after).min(4) as usize) * observed_record::BYTES;
        if records.len() != expected {
            return Err("history omitted committed records".into());
        }
        checked(self.key.verify_strict(
            &wire::transcript(
                &self.scope,
                &self.forms,
                self.after,
                batch.committed,
                &records,
            ),
            &checked(Signature::from_slice(&hex(&batch.signature)?))?,
        ))?;
        let mut after = self.after;
        let mut observed_ms = self.observed_ms;
        for bytes in records.as_chunks::<{ observed_record::BYTES }>().0 {
            let record = checked(ObservedRecord::decode(bytes))?;
            after += 1;
            if record.sequence != u64::from(after) || record.observed_at_boot_ms < observed_ms {
                return Err("history sequence or Boot clock".into());
            }
            observed_ms = record.observed_at_boot_ms;
        }
        Ok((after, observed_ms))
    }
    /// Revalidate durable batches from the beginning before filling a Face gap.
    /// These are projections with original observation clocks, not raw receipts.
    pub fn replay_gap(
        &self,
        directory: &Path,
        face: &mut conduit_environment_runtime::retained_publication_face::RetainedPublicationFace,
        before: u64,
        now: u64,
    ) -> Result<()> {
        let mut replay = Self {
            scope: self.scope,
            forms: self.forms,
            key: self.key,
            after: 0,
            observed_ms: 0,
        };
        let mut paths = std::fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("observed-history-") && name.ends_with(".json")
            })
        });
        paths.sort();
        for path in paths {
            let bytes = std::fs::read(path)?;
            let (after, clock) = replay.validate(&bytes)?;
            let batch: Batch = serde_json::from_slice(&bytes)?;
            let records = hex(&batch.records)?;
            for bytes in records.as_chunks::<{ observed_record::BYTES }>().0 {
                let record = checked(ObservedRecord::decode(bytes))?;
                if record.sequence >= before {
                    return Ok(());
                }
                if record.sequence > face.last_source_sequence() {
                    checked(face.publish_projected(&record, now))?;
                }
            }
            replay.after = after;
            replay.observed_ms = clock;
        }
        Ok(())
    }
    pub fn collect(&mut self, address: SocketAddr, directory: &Path) -> Result<()> {
        self.collect_slot(address, directory, 0)
    }
    fn collect_slot(&mut self, address: SocketAddr, directory: &Path, slot: u8) -> Result<()> {
        self.collect_from(directory, slot, |cursor| fetch(address, cursor))
    }
    /// Collect bounded signed batches from an already admitted transport. The
    /// source receives the exact encoded epoch cursor; transport delivery is
    /// never durable custody. Scope, signature and ordering checks stay here,
    /// and only synced files advance the retained cursor.
    pub fn collect_from(
        &mut self,
        directory: &Path,
        slot: u8,
        mut source: impl FnMut(u32) -> Result<Vec<u8>>,
    ) -> Result<()> {
        if self.after == 0 {
            self.restore(directory)?;
        }
        // At most 240 batches cover the admitted 960-publication lifetime.
        for _ in 0..240 {
            let cursor = wire::epoch_cursor(slot, self.after).ok_or("history epoch cursor")?;
            let bytes = source(cursor)?;
            let (after, observed_ms) = self.validate(&bytes)?;
            if after == self.after {
                return Ok(());
            }
            create_durable_directory(directory)?;
            let path = directory.join(format!(
                "observed-history-{:04}-{after:04}.json",
                self.after
            ));
            persist_batch(&path, &bytes)?;
            File::open(directory)?.sync_all()?;
            // Advance only after file and directory custody is durable.
            self.after = after;
            self.observed_ms = observed_ms;
        }
        Ok(())
    }
    /// Follow only a directory signed by this collector's independently admitted
    /// producer key, within its already pinned Body and Forms. Discovery cannot
    /// introduce a different key or Body. Each scope gets separate fsynced files.
    #[allow(dead_code)] // Pending joined-sibling transport integration.
    pub fn collect_epochs(&self, address: SocketAddr, directory: &Path) -> Result<usize> {
        let bytes = fetch(address, wire::DIRECTORY_CURSOR)?;
        let epochs = self.verify_directory(&bytes)?;
        for (slot, scope) in &epochs {
            let mut collector = Self {
                scope: *scope,
                forms: self.forms,
                key: self.key,
                after: 0,
                observed_ms: 0,
            };
            let name = scope
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            collector.collect_slot(address, &directory.join(format!("epoch-{name}")), *slot)?;
        }
        Ok(epochs.len())
    }
    #[allow(dead_code)]
    fn verify_directory(&self, bytes: &[u8]) -> Result<Vec<(u8, [u8; 96])>> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Directory {
            profile: String,
            directory: String,
            signature: String,
        }
        let document: Directory = serde_json::from_slice(bytes)?;
        if document.profile != "observed-epochs@1" {
            return Err("history directory profile".into());
        }
        let raw = hex(&document.directory)?;
        let raw: &[u8; wire::DIRECTORY_BYTES] = raw
            .as_slice()
            .try_into()
            .map_err(|_| "history directory extent")?;
        let signature: [u8; 64] = hex(&document.signature)?
            .try_into()
            .map_err(|_| "history directory signature")?;
        checked(self.key.verify_strict(
            &wire::directory_transcript(raw),
            &Signature::from_bytes(&signature),
        ))?;
        let count = usize::from(raw[32]);
        if raw[..32] != self.scope[..32]
            || count > 5
            || raw[33 + count * 133..].iter().any(|byte| *byte != 0)
        {
            return Err("history directory Body or capacity".into());
        }
        let mut epochs: Vec<(u8, [u8; 96])> = Vec::with_capacity(count);
        for entry in raw[33..33 + count * 133].as_chunks::<133>().0 {
            let slot = entry[0];
            let scope: [u8; 96] = entry[1..97].try_into().unwrap();
            let committed = u32::from_le_bytes(entry[129..133].try_into().unwrap());
            if slot > 5
                || scope[..32] != self.scope[..32]
                || entry[97..129] != self.forms
                || committed == 0
                || committed > 960
                || epochs
                    .iter()
                    .any(|(old_slot, old_scope)| *old_slot == slot || old_scope == &scope)
            {
                return Err("history directory epoch scope".into());
            }
            epochs.push((slot, scope));
        }
        Ok(epochs)
    }
}
// Publish a synced inode without replacing any existing custody. A crash can
// leave a temporary inode, but can never expose a partially written final file.
fn persist_batch(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        if std::fs::read(path)? != bytes {
            return Err("history custody differs".into());
        }
        File::open(path)?.sync_all()?;
        return Ok(());
    }
    let mut nonce = 0u64;
    let (temporary, mut output) = loop {
        let temporary = path.with_extension(format!("pending-{}-{nonce}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
        {
            Ok(output) => break (temporary, output),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => nonce += 1,
            Err(error) => return Err(error.into()),
        }
    };
    let result = (|| -> Result<()> {
        output.write_all(bytes)?;
        output.sync_all()?;
        match std::fs::hard_link(&temporary, path) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read(path)? != bytes {
                    return Err("history custody differs".into());
                }
            }
            Err(error) => return Err(error.into()),
        }
        File::open(path.parent().ok_or("history directory")?)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}
fn fetch(address: SocketAddr, after: u32) -> Result<Vec<u8>> {
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    write!(
        socket,
        "GET /history?after={after} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        address.ip()
    )?;
    let mut bytes = Vec::new();
    socket.take(2049).read_to_end(&mut bytes)?;
    if bytes.len() > 2048 {
        return Err("history HTTP extent".into());
    }
    let boundary = bytes
        .windows(4)
        .position(|v| v == b"\r\n\r\n")
        .ok_or("history HTTP framing")?;
    if !bytes.starts_with(b"HTTP/1.0 200 OK\r\n") {
        return Err("history unavailable".into());
    }
    Ok(bytes[boundary + 4..].to_vec())
}
#[cfg(test)]
mod tests;
pub(crate) mod retained;
