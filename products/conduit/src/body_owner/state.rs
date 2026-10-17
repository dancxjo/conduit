//! Recoverable publication into the installed Host's authoritative Body binding.
use super::super::{
    bounded_read, digest, read_installation, restrict_directory, write_bytes_atomic,
    write_json_atomic, BodyBinding, Installation,
};
use conduit_body::{BodyBiographyArchiveSegment, BodyBiographyEvidence, BodyLifecycleSession};
#[path = "state_archive.rs"]
mod archive;
#[cfg(test)]
use archive::archive_path;
use archive::{retain_archive_segments, validate_archive_segments, verify_retained_archive};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
pub(super) const MAXIMUM_STATE: u64 = 2 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    schema: String,
    biography: BodyBiographyEvidence,
    installation: Installation,
    #[serde(default)]
    last_execution: Option<serde_json::Value>,
    #[serde(default)]
    admissions: Option<conduit_body::AdmissionManager>,
    /// A checked source replacement travels with the matching biography.
    /// Older owner transactions did not carry source and remain replayable.
    #[serde(default)]
    source: Option<Vec<u8>>,
    #[serde(default)]
    archives: Vec<BodyBiographyArchiveSegment>,
}

pub(super) fn recover(root: &Path) -> Result<(), String> {
    let file = root.join("body/owner-transaction.json");
    if !file.exists() {
        return verify_retained_archive(root);
    }
    let transaction: Transaction = serde_json::from_slice(&bounded_read(&file, MAXIMUM_STATE)?)
        .map_err(|e| format!("owner transaction invalid: {e}"))?;
    // The biography may already be new while installation still names the old
    // digest. Validate independent installation identity before replaying the
    // journal, then the ordinary reader validates the committed pair.
    let current: Installation =
        serde_json::from_slice(&bounded_read(&root.join("installation.json"), 64 * 1024)?)
            .map_err(|e| e.to_string())?;
    if current.schema != super::super::INSTALL_SCHEMA
        || current.host_id.is_empty()
        || !super::super::valid_digest(&current.release_bundle_sha256)
    {
        return Err("owner recovery installation identity is invalid".into());
    }
    if transaction.schema != "conduit.body/owner-transaction@1"
        || current.host_id != transaction.installation.host_id
        || current.release_bundle_sha256 != transaction.installation.release_bundle_sha256
    {
        return Err("owner transaction belongs to another installed Host".into());
    }
    if current.joined_body_state.is_some()
        || current
            .body_state
            .as_ref()
            .is_some_and(|binding| binding.body_id != transaction.biography.body_id.as_str())
    {
        return Err("owner transaction would replace another retained Body".into());
    }
    commit(root, &transaction)?;
    read_installation(&root.join("installation.json"))?;
    verify_retained_archive(root)?;
    fs::remove_file(file).map_err(|e| e.to_string())
}
fn commit(root: &Path, transaction: &Transaction) -> Result<(), String> {
    transaction
        .biography
        .validate()
        .map_err(|e| format!("invalid retained biography: {e:?}"))?;
    let bytes = serde_json::to_vec_pretty(&transaction.biography).map_err(|e| e.to_string())?;
    let binding = transaction
        .installation
        .body_state
        .as_ref()
        .ok_or("owner transaction lacks binding")?;
    if binding.body_id != transaction.biography.body_id.as_str()
        || binding.biography_sha256 != digest(&bytes)
        || Path::new(&binding.biography_path) != root.join("body/biography.json")
    {
        return Err("owner transaction biography identity differs".into());
    }
    if let Some(source) = &transaction.source {
        validate_source(&transaction.biography, source)?;
        write_bytes_atomic(&root.join("body/source.conduit"), source)?;
    }
    retain_archive_segments(root, &transaction.biography, &transaction.archives)?;
    write_bytes_atomic(&root.join("body/biography.json"), &bytes)?;
    write_json_atomic(
        &root.join("body/owner-execution.json"),
        &serde_json::json!({
            "host_id": transaction.installation.host_id, "body_id": transaction.biography.body_id,
            "schema": "conduit.body/owner-execution@1",
            "sha256": digest(&serde_json::to_vec(&transaction.last_execution).map_err(|e| e.to_string())?),
            "last_execution": transaction.last_execution
        }),
    )?;
    if let Some(manager) = &transaction.admissions {
        // The installed Host's invitation entrance and its foreground owner
        // must consume the same single-use admission authority.
        write_json_atomic(&root.join("body/admission.json"), manager)?;
        let legacy = root.join("body/owner-admissions.json");
        if legacy.exists() {
            fs::remove_file(legacy).map_err(|error| error.to_string())?;
        }
    }
    write_json_atomic(&root.join("installation.json"), &transaction.installation)
}

#[cfg(test)]
pub(super) fn retain(
    root: &Path,
    biography: &BodyBiographyEvidence,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
) -> Result<(), String> {
    retain_with_source(root, biography, last_execution, admissions, None)
}

pub(super) fn retain_with_source(
    root: &Path,
    biography: &BodyBiographyEvidence,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
    source: Option<&[u8]>,
) -> Result<(), String> {
    retain_with_source_and_todo_selection(root, biography, last_execution, admissions, source, None)
}

#[cfg(test)]
pub(super) fn retain_with_archives(
    root: &Path,
    biography: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
) -> Result<(), String> {
    retain_inner(
        root,
        biography,
        last_execution,
        admissions,
        None,
        None,
        archives,
    )
}

/// Publish one lifecycle session and its archive obligation in the same
/// recoverable owner transaction. The in-memory obligation is acknowledged
/// only after the retained archive chain and active biography are durable.
pub(super) fn retain_session(
    root: &Path,
    session: &mut BodyLifecycleSession,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
    source: Option<&[u8]>,
    todo_selection: Option<&super::super::selected_todo::Selection>,
) -> Result<(), String> {
    let head = session
        .pending_archives()
        .last()
        .map(|segment| segment.digest);
    retain_inner(
        root,
        session.evidence(),
        last_execution,
        admissions,
        source,
        todo_selection,
        session.pending_archives(),
    )?;
    if let Some(head) = head {
        session
            .acknowledge_archives(head)
            .map_err(|error| format!("acknowledge committed biography archive: {error:?}"))?;
    }
    Ok(())
}

/// One owner transaction changes the resident Plot and installed Host's exact
/// Todo selection together. A partial replay cannot pair the next Source with
/// the previous generation or vice versa.
pub(super) fn retain_with_source_and_todo_selection(
    root: &Path,
    biography: &BodyBiographyEvidence,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
    source: Option<&[u8]>,
    todo_selection: Option<&super::super::selected_todo::Selection>,
) -> Result<(), String> {
    retain_inner(
        root,
        biography,
        last_execution,
        admissions,
        source,
        todo_selection,
        &[],
    )
}

fn retain_inner(
    root: &Path,
    biography: &BodyBiographyEvidence,
    last_execution: Option<&serde_json::Value>,
    admissions: Option<&conduit_body::AdmissionManager>,
    source: Option<&[u8]>,
    todo_selection: Option<&super::super::selected_todo::Selection>,
    archives: &[BodyBiographyArchiveSegment],
) -> Result<(), String> {
    validate_archive_segments(root, biography, archives)?;
    if let Some(source) = source {
        validate_source(biography, source)?;
    }
    if serde_json::to_vec(&last_execution)
        .map_err(|e| e.to_string())?
        .len()
        > 128 * 1024
    {
        return Err("owner execution receipt storage bound exhausted".into());
    }
    biography
        .validate()
        .map_err(|e| format!("invalid biography: {e:?}"))?;
    let bytes = serde_json::to_vec_pretty(biography).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAXIMUM_STATE / 2 {
        return Err("owner biography storage bound exhausted".into());
    }
    let mut installation = read_installation(&root.join("installation.json"))?;
    if let Some(selection) = todo_selection {
        selection.validate()?;
        let prior = installation
            .selected_todo_checkpoint
            .as_ref()
            .ok_or("owner transaction has no installed Todo selection")?;
        if prior.root() != selection.root()
            || prior.content().identity != selection.content().identity
            || prior.content().version == selection.content().version
        {
            return Err("owner transaction changed the wrong Todo residence".into());
        }
        installation.selected_todo_checkpoint = Some(selection.clone());
    }
    if installation.joined_body_state.is_some()
        || installation
            .body_state
            .as_ref()
            .is_some_and(|b| b.body_id != biography.body_id.as_str())
    {
        return Err("installed Host already belongs to another Body".into());
    }
    let directory = root.join("body");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    restrict_directory(&directory)?;
    installation.body_state = Some(BodyBinding {
        body_id: biography.body_id.as_str().into(),
        biography_sha256: digest(&bytes),
        biography_path: directory.join("biography.json").display().to_string(),
    });
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: biography.clone(),
        installation,
        last_execution: last_execution.cloned(),
        admissions: admissions.cloned(),
        source: source.map(Vec::from),
        archives: archives.to_vec(),
    };
    if serde_json::to_vec(&transaction)
        .map_err(|e| e.to_string())?
        .len() as u64
        > MAXIMUM_STATE
    {
        return Err("owner transaction storage bound exhausted".into());
    }
    write_json_atomic(&directory.join("owner-transaction.json"), &transaction)?;
    recover(root)
}

fn validate_source(biography: &BodyBiographyEvidence, source: &[u8]) -> Result<(), String> {
    if source.len() > super::MAXIMUM_SOURCE as usize {
        return Err("owner source storage bound exhausted".into());
    }
    let text = std::str::from_utf8(source)
        .map_err(|_| "owner transaction source is not UTF-8".to_string())?;
    let checked = crate::plot_source::parse(text)?.expand_entry_for_authoring()?;
    let resident = conduit_body::ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    if !biography.body.workset.plots().contains(&resident) {
        return Err("owner transaction source differs from the retained workset".into());
    }
    Ok(())
}
pub(super) fn load(root: &Path) -> Result<Option<BodyBiographyEvidence>, String> {
    recover(root)?;
    let installation = read_installation(&root.join("installation.json"))?;
    if installation.joined_body_state.is_some() {
        return Err("a joined Host cannot birth or own another Body".into());
    }
    let Some(binding) = installation.body_state else {
        return Ok(None);
    };
    let bytes = bounded_read(Path::new(&binding.biography_path), MAXIMUM_STATE)?;
    if digest(&bytes) != binding.biography_sha256 {
        return Err("retained biography digest differs".into());
    }
    let biography: BodyBiographyEvidence =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    biography
        .validate()
        .map_err(|e| format!("invalid retained biography: {e:?}"))?;
    if biography.body_id.as_str() != binding.body_id {
        return Err("retained Body identity differs".into());
    }
    Ok(Some(biography))
}

/// Retained execution is historical evidence, never an instruction to restart a Play.
pub(super) fn execution(root: &Path) -> Result<Option<serde_json::Value>, String> {
    let file = root.join("body/owner-execution.json");
    if !file.exists() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_slice(&bounded_read(&file, 192 * 1024)?)
        .map_err(|e| format!("invalid owner execution receipt: {e}"))?;
    let installation = read_installation(&root.join("installation.json"))?;
    if value["schema"] != "conduit.body/owner-execution@1"
        || value["sha256"].as_str()
            != Some(
                digest(&serde_json::to_vec(&value["last_execution"]).map_err(|e| e.to_string())?)
                    .as_str(),
            )
        || value["host_id"].as_str() != Some(installation.host_id.as_str())
        || value["body_id"].as_str()
            != installation
                .body_state
                .as_ref()
                .map(|body| body.body_id.as_str())
    {
        return Err("execution receipt belongs to another Host or Body".into());
    }
    match value.get("last_execution") {
        Some(serde_json::Value::Null) => Ok(None),
        Some(receipt) if receipt.is_object() => Ok(Some(receipt.clone())),
        _ => Err("execution receipt is malformed".into()),
    }
}

/// Continuity keys are retained with the same recoverable biography transaction.
pub(super) fn admissions(
    root: &Path,
    body: &conduit_body::BodyId,
) -> Result<Option<conduit_body::AdmissionManager>, String> {
    let path = root.join("body/admission.json");
    let legacy = root.join("body/owner-admissions.json");
    if !path.exists() && !legacy.exists() {
        return Ok(None);
    }
    let read = |path: &Path| -> Result<conduit_body::AdmissionManager, String> {
        serde_json::from_slice(&bounded_read(path, MAXIMUM_STATE)?).map_err(|e| e.to_string())
    };
    let manager = if path.exists() {
        let current = read(&path)?;
        if legacy.exists() && read(&legacy)? != current {
            return Err("owner and installed Host admission authorities conflict".into());
        }
        current
    } else {
        read(&legacy)?
    };
    if &manager.body_id != body {
        return Err("retained admissions belong to another Body".into());
    }
    Ok(Some(manager))
}

#[cfg(test)]
#[path = "state_archive_tests.rs"]
mod archive_tests;

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
