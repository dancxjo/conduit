//! Read-only historical inspection of independently admitted, signed evidence.
//! This publishes no Body Face, current Play, recording commit, or custody ACK.
use super::*;
use std::{collections::VecDeque, fs, os::unix::fs::MetadataExt};

const MAXIMUM_POINTS: usize = 16;

#[derive(serde::Serialize)]
struct Series<'a> {
    schema: &'static str,
    custody: &'static str,
    body_id: &'a str,
    producer_host: &'a str,
    producer_boot: &'a str,
    scope_hex: String,
    forms_hex: String,
    root_public_key_hex: String,
    verified_through_sequence: u32,
    samples_outside_view: u32,
    records_hex: String,
}
fn encoded(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn private(path: &Path, directory: bool) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    // SAFETY: geteuid only reads the local process identity.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        return Err("historical inspection requires private owner files".into());
    }
    Ok(())
}

/// Revalidate each original batch exactly once. Only its validated immutable
/// bytes enter the bounded tail; a changed file cannot substitute between a
/// signature check and projection. Failure publishes no partial series.
fn tail(collector: &Collector, directory: &Path) -> Result<(u32, Vec<u8>)> {
    private(directory, true)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.file_name().is_some_and(|n| {
            let n = n.to_string_lossy();
            n.starts_with("observed-history-") && n.ends_with(".json")
        }) {
            // At most 960 nonempty batches can exist in this finite epoch.
            if names.len() == 960 {
                return Err("retained history file count".into());
            }
            names.push(path);
        }
    }
    names.sort();
    let mut replay = Collector {
        after: 0,
        observed_ms: 0,
        ..*collector
    };
    let mut records = VecDeque::with_capacity(MAXIMUM_POINTS);
    for path in names {
        private(&path, false)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let mut bytes = Vec::new();
        file.take(2049).read_to_end(&mut bytes)?;
        let (after, clock) = replay.validate(&bytes)?;
        if after == replay.after
            || path.file_name().unwrap()
                != std::ffi::OsStr::new(&format!(
                    "observed-history-{:04}-{after:04}.json",
                    replay.after
                ))
        {
            return Err("retained history filename or empty batch".into());
        }
        let batch: Batch = serde_json::from_slice(&bytes)?;
        for record in hex(&batch.records)?
            .as_chunks::<{ observed_record::BYTES }>()
            .0
        {
            if records.len() == MAXIMUM_POINTS {
                records.pop_front();
            }
            records.push_back(*record);
        }
        replay.after = after;
        replay.observed_ms = clock;
    }
    if records.is_empty() {
        return Err("no verified retained history".into());
    }
    Ok((replay.after, records.into_iter().flatten().collect()))
}

pub(crate) fn run(binding: &Path, directory: &Path, boot: &str, output: &Path) -> Result<()> {
    if !boot.starts_with("boot/") || boot.len() > 128 || boot.bytes().any(|b| b.is_ascii_control())
    {
        return Err("historical producer Boot".into());
    }
    let binding = crate::local_recorder::validated_binding(binding)?;
    let body = binding.credential().body_id.as_str();
    let name = directory
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("epoch directory")?;
    let scope: [u8; 96] = hex(name.strip_prefix("epoch-").ok_or("epoch directory")?)?
        .try_into()
        .map_err(|_| "epoch scope extent")?;
    if scope[..32] != Sha256::digest(body.as_bytes())[..]
        || scope[32..64] != Sha256::digest(boot.as_bytes())[..]
    {
        return Err("retained epoch Body/Boot pin".into());
    }
    let collector = Collector::from_authenticated_scope(scope, binding.root())?;
    let (verified_through_sequence, records) = tail(&collector, directory)?;
    let series = Series {
        schema: "conduit.environment/retained-observation-series@1",
        custody: "retained-signed-evidence",
        body_id: body,
        producer_host: binding.producer_host().as_str(),
        producer_boot: boot,
        scope_hex: encoded(&scope),
        forms_hex: encoded(&collector.forms),
        root_public_key_hex: encoded(binding.root()),
        verified_through_sequence,
        samples_outside_view: verified_through_sequence
            - (records.len() / observed_record::BYTES) as u32,
        records_hex: encoded(&records),
    };
    let bytes = serde_json::to_vec(&series)?;
    if bytes.len() > 4096 {
        return Err("retained series extent".into());
    }
    let parent = output.parent().ok_or("historical series parent")?;
    private(parent, true)?;
    if output.exists() {
        private(output, false)?;
    }
    let temporary = output.with_file_name(format!(".retained-series-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, output)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests;
