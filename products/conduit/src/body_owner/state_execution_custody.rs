//! Finite historical execution receipts in the existing recoverable owner transaction.
//! These records confer no authority and do not imply a Play is still live.
use super::{bounded_read, digest, restrict_directory, write_bytes_atomic, write_json_atomic};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

pub(super) const MAX_RECEIPTS: usize = 64;
pub(super) const MAX_RECEIPT_BYTES: u64 = 128 * 1024;
const MAX_RECORD_BYTES: u64 = MAX_RECEIPT_BYTES + 1024;
const INDEX_SCHEMA: &str = "conduit.body/owner-execution@2";
const RECORD_SCHEMA: &str = "conduit.body/execution-custody@1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Index {
    schema: String,
    host_id: String,
    body_id: String,
    sha256: String,
    #[serde(default)]
    history: Vec<String>,
    #[serde(default)]
    legacy_prefix_unavailable: bool,
    last_execution: Option<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    host_id: String,
    body_id: String,
    receipt: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prepared {
    index: Index,
    additions: Vec<Record>,
}
fn record(host: &str, body: &str, receipt: Value) -> Result<Record, String> {
    if !receipt.is_object()
        || serde_json::to_vec(&receipt)
            .map_err(|e| e.to_string())?
            .len() as u64
            > MAX_RECEIPT_BYTES
        || receipt
            .get("body_id")
            .is_some_and(|id| id.as_str() != Some(body))
    {
        return Err("execution custody receipt is oversized or belongs to another Body".into());
    }
    Ok(Record {
        schema: RECORD_SCHEMA.into(),
        host_id: host.into(),
        body_id: body.into(),
        receipt,
    })
}
fn bytes(record: &Record) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("execution custody record storage bound exhausted".into());
    }
    Ok(bytes)
}
fn checksum(index: &Index) -> Result<String, String> {
    Ok(digest(
        &serde_json::to_vec(&(
            &index.host_id,
            &index.body_id,
            &index.history,
            index.legacy_prefix_unavailable,
            &index.last_execution,
        ))
        .map_err(|e| e.to_string())?,
    ))
}
fn validate(index: &Index, host: &str, body: &str) -> Result<(), String> {
    let framing = Record {
        schema: RECORD_SCHEMA.into(),
        host_id: host.into(),
        body_id: body.into(),
        receipt: Value::Null,
    };
    if serde_json::to_vec(&framing)
        .map_err(|e| e.to_string())?
        .len() as u64
        > MAX_RECORD_BYTES - MAX_RECEIPT_BYTES
    {
        return Err("execution custody identity framing exceeds its profile".into());
    }
    let unique: BTreeSet<_> = index.history.iter().collect();
    if index.host_id != host
        || index.body_id != body
        || index.history.len() > MAX_RECEIPTS
        || unique.len() != index.history.len()
        || index
            .history
            .iter()
            .any(|id| !super::super::super::valid_digest(id))
    {
        return Err("execution custody index has invalid scope or capacity".into());
    }
    let expected = if index.schema == "conduit.body/owner-execution@1"
        && index.history.is_empty()
        && !index.legacy_prefix_unavailable
    {
        digest(&serde_json::to_vec(&index.last_execution).map_err(|e| e.to_string())?)
    } else if index.schema == INDEX_SCHEMA {
        checksum(index)?
    } else {
        return Err("unsupported execution custody index".into());
    };
    if index.sha256 != expected {
        return Err("execution custody index digest differs".into());
    }
    if let Some(value) = &index.last_execution {
        record(host, body, value.clone())?;
    }
    Ok(())
}
fn index(root: &Path, host: &str, body: &str, replay: bool) -> Result<Index, String> {
    let path = root.join("body/owner-execution.json");
    match fs::symlink_metadata(&path) {
        Ok(meta) if !meta.file_type().is_file() => {
            Err("execution custody index is not an ordinary file".into())
        }
        Ok(_) => {
            let value: Index = serde_json::from_slice(&bounded_read(&path, 192 * 1024)?)
                .map_err(|e| e.to_string())?;
            validate(&value, host, body)?;
            Ok(value)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !replay {
                inventory(root, &[], false)?;
            }
            let mut value = Index {
                schema: INDEX_SCHEMA.into(),
                host_id: host.into(),
                body_id: body.into(),
                sha256: String::new(),
                history: Vec::new(),
                legacy_prefix_unavailable: false,
                last_execution: None,
            };
            value.sha256 = checksum(&value)?;
            validate(&value, host, body)?;
            Ok(value)
        }
        Err(error) => Err(error.to_string()),
    }
}
fn path(root: &Path, hash: &str) -> std::path::PathBuf {
    root.join("body/execution-custody")
        .join(format!("{hash}.json"))
}
fn read_record(root: &Path, hash: &str, host: &str, body: &str) -> Result<Record, String> {
    let directory = root.join("body/execution-custody");
    if !fs::symlink_metadata(&directory)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_dir()
        || !fs::symlink_metadata(path(root, hash))
            .map_err(|e| e.to_string())?
            .file_type()
            .is_file()
    {
        return Err("execution custody residence is not an ordinary directory/file".into());
    }
    let data = bounded_read(&path(root, hash), MAX_RECORD_BYTES)?;
    if digest(&data) != hash {
        return Err("execution custody record digest differs".into());
    }
    let value: Record = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if value.schema != RECORD_SCHEMA || value.host_id != host || value.body_id != body {
        return Err("execution custody record belongs to another Host or Body".into());
    }
    record(host, body, value.receipt.clone())?;
    Ok(value)
}
fn verify_history(root: &Path, index: &Index) -> Result<(), String> {
    for hash in &index.history {
        read_record(root, hash, &index.host_id, &index.body_id)?;
    }
    if index.schema == INDEX_SCHEMA {
        if !index.history.is_empty() && index.last_execution.is_none() {
            return Err("execution custody index lost its latest receipt".into());
        }
        if let Some(receipt) = &index.last_execution {
            let hash = digest(&bytes(&record(
                &index.host_id,
                &index.body_id,
                receipt.clone(),
            )?)?);
            if !index.history.contains(&hash) {
                return Err("latest execution is absent from retained custody".into());
            }
        }
    }
    Ok(())
}

pub(super) fn prepare(
    root: &Path,
    host: &str,
    body: &str,
    latest: Option<&Value>,
) -> Result<Prepared, String> {
    let mut current = index(root, host, body, false)?;
    inventory(root, &current.history, false)?;
    verify_history(root, &current)?;
    let mut additions = Vec::with_capacity(2);
    // Migrate only the known legacy latest receipt; never invent earlier history.
    for value in [current.last_execution.clone(), latest.cloned()]
        .into_iter()
        .flatten()
    {
        let entry = record(host, body, value)?;
        let hash = digest(&bytes(&entry)?);
        if !current.history.contains(&hash) {
            if current.history.len() == MAX_RECEIPTS {
                return Err("execution custody capacity exhausted".into());
            }
            current.history.push(hash);
            additions.push(entry);
        }
    }
    current.legacy_prefix_unavailable |= current.schema != INDEX_SCHEMA;
    current.schema = INDEX_SCHEMA.into();
    if let Some(value) = latest {
        current.last_execution = Some(value.clone());
    }
    current.sha256 = checksum(&current)?;
    Ok(Prepared {
        index: current,
        additions,
    })
}

pub(super) fn commit(
    root: &Path,
    prepared: &Prepared,
    host: &str,
    body: &str,
) -> Result<(), String> {
    validate(&prepared.index, host, body)?;
    if prepared.index.schema != INDEX_SCHEMA {
        return Err("execution custody transaction cannot downgrade its schema".into());
    }
    if prepared.additions.len() > 2 {
        return Err("execution custody transaction capacity exceeded".into());
    }
    let prior = index(root, host, body, true)?;
    inventory(root, &prepared.index.history, true)?;
    if !prepared.index.history.starts_with(&prior.history) {
        return Err("execution custody transaction would erase historical outcomes".into());
    }
    let directory = root.join("body/execution-custody");
    match fs::symlink_metadata(&directory) {
        Ok(meta) if !meta.file_type().is_dir() => {
            return Err("execution custody residence is not a directory".into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    restrict_directory(&directory)?;
    for value in &prepared.additions {
        if value.schema != RECORD_SCHEMA || value.host_id != host || value.body_id != body {
            return Err("execution custody transaction scope differs".into());
        }
        record(host, body, value.receipt.clone())?;
        let data = bytes(value)?;
        let hash = digest(&data);
        if !prepared.index.history.contains(&hash) {
            return Err("execution custody addition is not indexed".into());
        }
        match fs::symlink_metadata(path(root, &hash)) {
            Ok(_) => {
                let existing = read_record(root, &hash, host, body)?;
                if bytes(&existing)? != data {
                    return Err("execution custody slot changed".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_bytes_atomic(&path(root, &hash), &data)?
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    verify_history(root, &prepared.index)?;
    write_json_atomic(&root.join("body/owner-execution.json"), &prepared.index)
}

pub(super) fn latest(root: &Path, host: &str, body: &str) -> Result<Option<Value>, String> {
    let current = index(root, host, body, false)?;
    inventory(root, &current.history, false)?;
    verify_history(root, &current)?;
    Ok(current.last_execution)
}
pub(super) fn history(root: &Path, host: &str, body: &str) -> Result<Value, String> {
    let prepared = prepare(root, host, body, None)?;
    // Legacy latest remains available without pretending an archive was committed.
    let mut values = Vec::with_capacity(prepared.index.history.len());
    for hash in &prepared.index.history {
        if let Some(value) = prepared
            .additions
            .iter()
            .find(|r| bytes(r).is_ok_and(|b| digest(&b) == *hash))
        {
            values.push(value.receipt.clone());
        } else {
            values.push(read_record(root, hash, host, body)?.receipt);
        }
    }
    Ok(
        serde_json::json!({"schema":"conduit.body/execution-history@1", "coverage":"this-installed-owner-root", "historical_only":true, "custodian_host_id":host,
        "body_id":body, "capacity":MAX_RECEIPTS, "maximum_receipt_bytes":MAX_RECEIPT_BYTES,
        "legacy_prefix_unavailable":prepared.index.legacy_prefix_unavailable,
        "archive_records_committed":prepared.additions.is_empty(), "receipt_digests":prepared.index.history, "receipts":values}),
    )
}
pub(super) fn require_capacity(root: &Path, host: &str, body: &str) -> Result<(), String> {
    let prepared = prepare(root, host, body, None)?;
    if prepared.index.history.len() >= MAX_RECEIPTS {
        return Err("execution custody capacity exhausted before Play".into());
    }
    Ok(())
}

// Normal reads refuse unindexed files rather than silently starting an empty
// history. Only replay of the retained transaction may complete its own slots.
fn inventory(root: &Path, allowed: &[String], replay: bool) -> Result<(), String> {
    let directory = root.join("body/execution-custody");
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Ok(meta) if meta.file_type().is_dir() => {}
        _ => return Err("execution custody residence is not a directory".into()),
    }
    for (ordinal, entry) in fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if ordinal >= MAX_RECEIPTS + 2 {
            return Err("execution custody directory exceeds its finite bound".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            return Err("execution custody contains a non-file slot".into());
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or("execution custody slot name is not UTF-8")?;
        let hash = name
            .strip_suffix(".json")
            .or_else(|| replay.then(|| name.strip_suffix(".tmp")).flatten())
            .ok_or("execution custody contains an unknown slot")?;
        if !allowed.iter().any(|id| id == hash) {
            return Err("execution custody contains an unindexed outcome".into());
        }
    }
    Ok(())
}

impl Prepared {
    pub(super) fn require_latest(&self, latest: Option<&Value>) -> Result<(), String> {
        if latest.is_some() && self.index.last_execution.as_ref() != latest {
            return Err("execution custody transaction differs from its terminal receipt".into());
        }
        Ok(())
    }
}
