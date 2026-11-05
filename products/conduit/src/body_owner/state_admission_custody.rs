//! Finite cold admission evidence published by the existing owner transaction.
use super::{bounded_read, digest, write_bytes_atomic};
use conduit_body::BodyLifecycleSession;
use conduit_body::{AdmissionManager, AdmissionReceipt, AdmissionReceiptRetirement};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const MAX_RECEIPTS: usize = 1024;
const MAX_BYTES: u64 = super::MAXIMUM_STATE / 4;
const SCHEMA: &str = "conduit.body/admission-custody@1";
// Exhausting either finite custody bound prevents safe hot receipt retirement.
// Keep the existing public pressure outcome through the browser's allowlist.
const CAPACITY_EXHAUSTED: &str = "admission-receipt-capacity-exhausted";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prepared {
    schema: String,
    host_id: String,
    body_id: conduit_body::BodyId,
    receipts: Vec<AdmissionReceipt>,
    sha256: String,
}

fn checksum(value: &Prepared) -> Result<String, String> {
    Ok(digest(
        &serde_json::to_vec(&(
            &value.schema,
            &value.host_id,
            &value.body_id,
            &value.receipts,
        ))
        .map_err(|e| e.to_string())?,
    ))
}

fn validate(value: &Prepared, host: &str, manager: &AdmissionManager) -> Result<(), String> {
    if value.schema != SCHEMA
        || value.host_id != host
        || value.body_id != manager.body_id
        || value.receipts.len() > MAX_RECEIPTS
        || value.receipts.len() as u64 != manager.retired_receipt_count()
        || checksum(value)? != value.sha256
    {
        return Err("admission custody scope, count, digest or capacity differs".into());
    }
    for (index, receipt) in value.receipts.iter().enumerate() {
        if receipt.credential.body_id != value.body_id
            || value.receipts[..index]
                .iter()
                .chain(manager.receipts.iter())
                .any(|other| {
                    other.admission_id == receipt.admission_id
                        || other.credential.credential_id == receipt.credential.credential_id
                })
        {
            return Err("admission custody contains conflicting or foreign evidence".into());
        }
    }
    encoded(value)?;
    Ok(())
}

fn encoded(value: &Prepared) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(CAPACITY_EXHAUSTED.into());
    }
    Ok(bytes)
}

fn path(root: &Path) -> std::path::PathBuf {
    root.join("body/admission-custody.json")
}

fn read(root: &Path) -> Result<Option<Prepared>, String> {
    match fs::symlink_metadata(path(root)) {
        Ok(meta) if meta.file_type().is_file() => {
            let bytes = bounded_read(&path(root), MAX_BYTES)?;
            Ok(Some(
                serde_json::from_slice(&bytes)
                    .map_err(|e| format!("invalid admission custody: {e}"))?,
            ))
        }
        Ok(_) => Err("admission custody is not an ordinary file".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub(super) fn verify(root: &Path, host: &str, manager: &AdmissionManager) -> Result<(), String> {
    match read(root)? {
        Some(value) => validate(&value, host, manager),
        None if manager.retired_receipt_count() == 0 => Ok(()),
        None => Err("retired admission evidence is missing".into()),
    }
}

/// Preparation reads at most 512 KiB and retains at most 1024 cold receipts.
/// The complete replacement also travels in the existing 2 MiB owner journal.
pub(super) fn prepare(
    root: &Path,
    host: &str,
    before: &AdmissionManager,
    retirement: &AdmissionReceiptRetirement,
) -> Result<(AdmissionManager, Prepared), String> {
    verify(root, host, before)?;
    let mut after = before.clone();
    after
        .acknowledge_receipt_custody(retirement)
        .map_err(|e| format!("admission custody selection: {e:?}"))?;
    let mut value = read(root)?.unwrap_or(Prepared {
        schema: SCHEMA.into(),
        host_id: host.into(),
        body_id: before.body_id.clone(),
        receipts: Vec::new(),
        sha256: String::new(),
    });
    if value.receipts.len() + retirement.receipts().len() > MAX_RECEIPTS {
        return Err(CAPACITY_EXHAUSTED.into());
    }
    value.receipts.extend_from_slice(retirement.receipts());
    value.sha256 = checksum(&value)?;
    validate(&value, host, &after)?;
    Ok((after, value))
}

pub(super) fn commit(
    root: &Path,
    host: &str,
    manager: &AdmissionManager,
    prepared: &Prepared,
) -> Result<(), String> {
    validate(prepared, host, manager)?;
    if let Some(prior) = read(root)? {
        // Prior bytes must still be intact even when replay follows a partial
        // commit. Exact equality also makes repeated replay idempotent.
        if checksum(&prior)? != prior.sha256
            || prior.schema != SCHEMA
            || prior.host_id != prepared.host_id
            || prior.body_id != prepared.body_id
            || !prepared.receipts.starts_with(&prior.receipts)
        {
            return Err("admission custody publication would replace historical evidence".into());
        }
    }
    write_bytes_atomic(&path(root), &encoded(prepared)?)
}

pub(super) fn verify_retained(root: &Path) -> Result<(), String> {
    let admission = root.join("body/admission.json");
    if !admission.exists() {
        if read(root)?.is_some() {
            return Err("admission custody has no authoritative manager".into());
        }
        return Ok(());
    }
    let manager: AdmissionManager =
        serde_json::from_slice(&bounded_read(&admission, super::MAXIMUM_STATE)?)
            .map_err(|e| e.to_string())?;
    let installation: super::Installation =
        serde_json::from_slice(&bounded_read(&root.join("installation.json"), 64 * 1024)?)
            .map_err(|e| e.to_string())?;
    if installation
        .body_state
        .as_ref()
        .is_none_or(|binding| binding.body_id != manager.body_id.as_str())
    {
        return Err("admission custody manager differs from installed Body".into());
    }
    verify(root, &installation.host_id, &manager)
}

pub(super) struct AdmissionUpdate<'a> {
    pub(super) manager: Option<&'a conduit_body::AdmissionManager>,
    pub(super) custody: Option<Prepared>,
}
impl<'a> From<Option<&'a conduit_body::AdmissionManager>> for AdmissionUpdate<'a> {
    fn from(manager: Option<&'a conduit_body::AdmissionManager>) -> Self {
        Self {
            manager,
            custody: None,
        }
    }
}

/// Retain the selected evidence and its smaller manager in one owner journal.
/// The caller replaces its live manager only after this returns successfully.
pub(super) fn retire(
    root: &Path,
    session: &mut BodyLifecycleSession,
    last_execution: Option<&serde_json::Value>,
    manager: &conduit_body::AdmissionManager,
) -> Result<conduit_body::AdmissionManager, String> {
    super::recover(root)?;
    let Some(selection) = manager
        .prepare_receipt_retirement()
        .map_err(|e| format!("admission retirement: {e:?}"))?
    else {
        return Err(CAPACITY_EXHAUSTED.into());
    };
    let installation = super::read_installation(&root.join("installation.json"))?;
    let (next, custody) = prepare(root, &installation.host_id, manager, &selection)?;
    super::retain_inner(
        root,
        session.evidence(),
        last_execution,
        AdmissionUpdate {
            manager: Some(&next),
            custody: Some(custody),
        },
        None.into(),
        None,
        session.pending_archives(),
    )?;
    if let Some(head) = session
        .pending_archives()
        .last()
        .map(|segment| segment.digest)
    {
        session
            .acknowledge_archives(head)
            .map_err(|e| format!("acknowledge committed biography archive: {e:?}"))?;
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_cold_profile_refuses_before_changing_retained_bytes() {
        let (root, _, manager) = super::super::admission_custody_tests::fixture();
        let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
        let (_, mut cold) = prepare(&root, "custody-host", &manager, &selection).unwrap();
        let template = serde_json::to_value(&cold.receipts[0]).unwrap();
        cold.receipts.clear();
        for index in 0..MAX_RECEIPTS {
            let mut row = template.clone();
            row["admission_id"] = format!("cold/admission/{index}").into();
            row["credential"]["credential_id"] = format!("cold/credential/{index}").into();
            cold.receipts.push(serde_json::from_value(row).unwrap());
        }
        let mut value = serde_json::to_value(&manager).unwrap();
        value["retired_receipt_count"] = (MAX_RECEIPTS as u64).into();
        let manager: AdmissionManager = serde_json::from_value(value).unwrap();
        cold.sha256 = checksum(&cold).unwrap();
        validate(&cold, "custody-host", &manager).unwrap();
        let bytes = encoded(&cold).unwrap();
        write_bytes_atomic(&path(&root), &bytes).unwrap();
        let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
        assert_eq!(
            prepare(&root, "custody-host", &manager, &selection)
                .err()
                .unwrap(),
            "admission-receipt-capacity-exhausted"
        );
        assert_eq!(fs::read(path(&root)).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cold_byte_pressure_is_public_and_preserves_both_retained_files() {
        let (root, _, manager) = super::super::admission_custody_tests::fixture();
        let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
        let (_, mut cold) = prepare(&root, "custody-host", &manager, &selection).unwrap();
        let template = serde_json::to_value(&cold.receipts[0]).unwrap();
        cold.receipts.clear();
        // Storage-only receipts with finite long Boot identities reach the byte
        // limit before the count limit. They do not claim authentication proof.
        for index in 0..MAX_RECEIPTS {
            let mut row = template.clone();
            row["admission_id"] = format!("cold/admission/{index}").into();
            row["credential"]["credential_id"] = format!("cold/credential/{index}").into();
            row["credential"]["boot_id"] = format!("boot/{index}/{}", "b".repeat(240)).into();
            cold.receipts.push(serde_json::from_value(row).unwrap());
            if serde_json::to_vec(&cold).unwrap().len() as u64 > MAX_BYTES {
                cold.receipts.pop();
                break;
            }
        }
        assert!(cold.receipts.len() + selection.receipts().len() < MAX_RECEIPTS);
        let mut value = serde_json::to_value(&manager).unwrap();
        value["retired_receipt_count"] = (cold.receipts.len() as u64).into();
        cold.sha256 = checksum(&cold).unwrap();
        let manager: AdmissionManager = serde_json::from_value(value).unwrap();
        validate(&cold, "custody-host", &manager).unwrap();
        let bytes = encoded(&cold).unwrap();
        write_bytes_atomic(&path(&root), &bytes).unwrap();
        write_bytes_atomic(
            &root.join("body/admission.json"),
            &serde_json::to_vec(&manager).unwrap(),
        )
        .unwrap();
        let hot = fs::read(root.join("body/admission.json")).unwrap();
        let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
        assert_eq!(
            prepare(&root, "custody-host", &manager, &selection)
                .err()
                .unwrap(),
            "admission-receipt-capacity-exhausted"
        );
        assert_eq!(fs::read(path(&root)).unwrap(), bytes);
        assert_eq!(fs::read(root.join("body/admission.json")).unwrap(), hot);
        fs::remove_dir_all(root).unwrap();
    }
}
