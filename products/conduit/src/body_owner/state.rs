//! Recoverable publication into the installed Host's authoritative Body binding.
use super::super::{
    bounded_read, digest, read_installation, restrict_directory, write_bytes_atomic,
    write_json_atomic, BodyBinding, Installation,
};
use conduit_body::BodyBiographyEvidence;
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
}

pub(super) fn recover(root: &Path) -> Result<(), String> {
    let file = root.join("body/owner-transaction.json");
    if !file.exists() {
        return Ok(());
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
mod tests {
    use super::*;
    #[test]
    fn checked_source_and_biography_replay_as_one_owner_transaction() {
        use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, ResidentPlot};
        use conduit_core::bind_sign;

        const SOURCE: &str =
            "plot retained-clock {\n clock: time/every(2s)\n clock >> presentation/tick\n}\n";
        let root = std::env::temp_dir().join(super::super::super::fresh_identity(
            "owner-source-journal",
            "checked-clock",
        ));
        fs::create_dir_all(&root).unwrap();
        let installation = Installation {
            schema: super::super::super::INSTALL_SCHEMA.into(),
            host_id: "host/source-journal".into(),
            release_source_identity: "source/test".into(),
            release_bundle_sha256: digest(b"bundle/test"),
            product_executable: "fixture-unused".into(),
            body_state: None,
            joined_body_state: None,
        };
        write_json_atomic(&root.join("installation.json"), &installation).unwrap();
        let checked = crate::plot_source::parse(SOURCE)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        let body = Body::born(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
            1,
            bind_sign(
                &"host/source-journal".into(),
                &"boot/source-journal".into(),
                None,
                1,
            )
            .sign_id,
        )
        .unwrap();
        let evidence = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Clock".into(),
        )
        .unwrap();
        let wrong_source = include_bytes!("../../../../plots/clock/main.conduit");
        retain_with_source(&root, &evidence, None, None, Some(SOURCE.as_bytes())).unwrap();
        let committed = read_installation(&root.join("installation.json")).unwrap();
        let transaction = Transaction {
            schema: "conduit.body/owner-transaction@1".into(),
            biography: evidence.clone(),
            installation: committed,
            last_execution: None,
            admissions: None,
            source: Some(SOURCE.as_bytes().to_vec()),
        };
        write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
        fs::write(root.join("body/source.conduit"), wrong_source).unwrap();
        fs::write(root.join("body/biography.json"), b"interrupted").unwrap();
        recover(&root).unwrap();
        assert_eq!(
            fs::read(root.join("body/source.conduit")).unwrap(),
            SOURCE.as_bytes()
        );
        assert_eq!(load(&root).unwrap().unwrap(), evidence);
        assert!(!root.join("body/owner-transaction.json").exists());

        let wrong = crate::plot_source::parse(std::str::from_utf8(wrong_source).unwrap())
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        assert_ne!(
            ResidentPlot::new(
                wrong.expanded.source_document_id,
                wrong.expanded.checked_plot_id
            ),
            body.workset.plots()[0]
        );
        assert!(
            retain_with_source(&root, &evidence, None, None, Some(wrong_source))
                .unwrap_err()
                .contains("source differs")
        );
        assert_eq!(load(&root).unwrap().unwrap(), evidence);
        assert_eq!(
            fs::read(root.join("body/source.conduit")).unwrap(),
            SOURCE.as_bytes()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_publication_recovers_and_corruption_never_becomes_birth() {
        use conduit_body::{Body, BodyMembership};
        let root = std::env::temp_dir().join(super::super::super::fresh_identity(
            "owner-state-test",
            "transaction",
        ));
        fs::create_dir_all(&root).unwrap();
        let installation = Installation {
            schema: super::super::super::INSTALL_SCHEMA.into(),
            host_id: "host/state-test".into(),
            release_source_identity: "source/test".into(),
            release_bundle_sha256: digest(b"bundle/test"),
            product_executable: "fixture-unused".into(),
            body_state: None,
            joined_body_state: None,
        };
        write_json_atomic(&root.join("installation.json"), &installation).unwrap();
        let body = Body::born(
            "source/test".into(),
            "checked/test".into(),
            1,
            "sign/born".into(),
        )
        .unwrap();
        let evidence = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Retained".into(),
        )
        .unwrap();
        let manager = conduit_body::AdmissionManager::new(body.body_id.clone()).unwrap();
        retain(&root, &evidence, None, Some(&manager)).unwrap();
        assert!(root.join("body/admission.json").exists());
        assert!(!root.join("body/owner-admissions.json").exists());
        assert_eq!(
            admissions(&root, &body.body_id).unwrap(),
            Some(manager.clone())
        );
        write_json_atomic(&root.join("body/owner-admissions.json"), &manager).unwrap();
        assert_eq!(
            admissions(&root, &body.body_id).unwrap(),
            Some(manager.clone())
        );
        assert!(
            super::super::super::invitation::issue_body_invitation_document(&root, 60, None)
                .err()
                .unwrap()
                .contains("legacy owner admission authority")
        );
        let mut conflicting = manager.clone();
        conflicting
            .issue_spawn_invitation(
                conduit_body::SpawnInvitationSecret::from_csprng_bytes([13; 32]).unwrap(),
                [17; 32],
                1_000,
                2_000,
            )
            .unwrap();
        write_json_atomic(&root.join("body/owner-admissions.json"), &conflicting).unwrap();
        assert!(admissions(&root, &body.body_id).is_err());
        write_json_atomic(&root.join("body/owner-admissions.json"), &manager).unwrap();
        retain(&root, &evidence, None, Some(&manager)).unwrap();
        assert!(!root.join("body/owner-admissions.json").exists());
        let retained_installation = read_installation(&root.join("installation.json")).unwrap();
        write_json_atomic(
            &root.join("body/owner-transaction.json"),
            &Transaction {
                schema: "conduit.body/owner-transaction@1".into(),
                biography: evidence.clone(),
                installation: retained_installation,
                last_execution: None,
                admissions: Some(manager.clone()),
                source: None,
            },
        )
        .unwrap();
        fs::write(root.join("body/biography.json"), b"interrupted write").unwrap();
        let invited =
            super::super::super::invitation::issue_body_invitation_document(&root, 60, None)
                .unwrap();
        assert_eq!(invited.claim.body_id, body.body_id);
        assert_eq!(load(&root).unwrap().unwrap().body_id, body.body_id);
        assert!(!root.join("body/owner-transaction.json").exists());
        assert_ne!(admissions(&root, &body.body_id).unwrap(), Some(manager));
        fs::write(root.join("body/biography.json"), b"corrupt").unwrap();
        assert!(load(&root).is_err());
        let raw: Installation =
            serde_json::from_slice(&fs::read(root.join("installation.json")).unwrap()).unwrap();
        assert!(raw.body_state.is_some());
        fs::remove_dir_all(root).unwrap();
    }
}
