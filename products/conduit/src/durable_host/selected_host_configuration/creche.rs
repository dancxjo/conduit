//! Open only the explicitly pinned prior Make inputs before advertising Crèche.
//! Configuration is intent; successful initialization retains actual file custody.
use conduit::creche_encounter::{
    CrecheEncounterFactory, CrecheEncounterInstallation, CrecheReviewedArtifact,
};
use conduit_body_make::JoiningProductionBounds;
use conduit_host_make::{BaseInitialization, HostImage};
use conduit_std_host::hosted_creche::CrecheLimits;
use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    slot_identity: String,
    maximum_evidence_bytes: u64,
    maximum_polls: u32,
    minimum_poll_interval_ms: u32,
    image_manifest: PathBuf,
    image_manifest_sha256: String,
    artifact: PathBuf,
    artifact_sha256: String,
    host_configuration: PathBuf,
    host_configuration_sha256: String,
    maximum_artifact_bytes: u64,
    maximum_manifest_bytes: usize,
}
fn parameters(initialization: &BaseInitialization) -> Result<Parameters, String> {
    let catalog = conduit_workspace_make::catalog();
    let contract = catalog
        .initialization_contracts
        .get(conduit_body_make::deployment_runtime_catalog::CRECHE_IMPLEMENTATION)
        .ok_or("installed Crèche initialization contract unavailable")?;
    initialization.checked_values(contract)?;
    let p: Parameters =
        serde_json::from_value(serde_json::to_value(initialization).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Crèche initialization parameters: {e}"))?;
    if ![&p.image_manifest, &p.artifact, &p.host_configuration]
        .iter()
        .all(|p| p.is_absolute())
        || ![
            &p.image_manifest_sha256,
            &p.artifact_sha256,
            &p.host_configuration_sha256,
        ]
        .iter()
        .all(|s| valid_digest(s))
    {
        return Err(
            "Crèche selection requires absolute Make input paths and exact SHA-256 identities"
                .into(),
        );
    }
    Ok(p)
}
fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn validate(initialization: &BaseInitialization) -> Result<(), String> {
    parameters(initialization).map(|_| ())
}
fn pinned_read(path: &Path, sha256: &str, maximum: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("open selected Make input: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err("selected Make input violates its finite file bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > maximum {
        return Err("selected Make input grew beyond its finite file bound".into());
    }
    if super::super::digest(&bytes).strip_prefix("sha256:") != Some(sha256) {
        return Err("selected Crèche Make input identity changed".into());
    }
    Ok(bytes)
}
pub(super) fn initialize(
    initialization: &BaseInitialization,
) -> Result<CrecheEncounterFactory, String> {
    let p = parameters(initialization)?;
    let image_manifest = pinned_read(
        &p.image_manifest,
        &p.image_manifest_sha256,
        p.maximum_manifest_bytes as u64,
    )?;
    let image: HostImage = serde_json::from_slice(&image_manifest)
        .map_err(|e| format!("selected Make Host image: {e}"))?;
    let source = pinned_read(
        &p.host_configuration,
        &p.host_configuration_sha256,
        super::MAXIMUM_SOURCE_BYTES,
    )?;
    let source = std::str::from_utf8(&source)
        .map_err(|_| "selected destination Host source is not UTF-8")?;
    // Destination may have another target; never apply the authoring Host's target check here.
    let configuration = conduit_host_make::parse_host_configuration_conduit(source)
        .map_err(|e| format!("destination Host source: {e:?}"))?;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let configuration =
        conduit_host_make::check_host_configuration(configuration, &catalog, &packages)
            .map_err(|e| format!("destination Host configuration: {e:?}"))?;
    let artifact = File::open(&p.artifact).map_err(|e| format!("selected Make artifact: {e}"))?;
    let metadata = artifact.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > p.maximum_artifact_bytes {
        return Err("selected Make artifact violates its admitted file bound".into());
    }
    let output = image.manifest.output.clone();
    let source_identity = image.manifest.source_identity.clone();
    CrecheEncounterFactory::new(
        catalog,
        packages,
        CrecheEncounterInstallation {
            encounter_id: crate::birth_identity::fresh_uuid()?,
            resource_pool: p.slot_identity.into(),
            limits: CrecheLimits {
                maximum_evidence_bytes: p.maximum_evidence_bytes,
                maximum_polls: p.maximum_polls,
                minimum_poll_interval_ms: p.minimum_poll_interval_ms,
            },
            production_bounds: JoiningProductionBounds {
                maximum_artifact_bytes: p.maximum_artifact_bytes,
                maximum_manifest_bytes: p.maximum_manifest_bytes,
                maximum_receipt_bytes: usize::try_from(
                    p.maximum_evidence_bytes.min(16 * 1024 * 1024),
                )
                .map_err(|_| "Crèche evidence bound exceeds this Host")?,
            },
            initial_source: Some(source.into()),
        },
        vec![CrecheReviewedArtifact {
            configuration,
            output,
            image,
            image_manifest,
            artifact,
            artifact_digest: format!("sha256:{}", p.artifact_sha256),
            artifact_bytes: metadata.len(),
            source_identity,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selection() -> BaseInitialization {
        serde_json::from_value(serde_json::json!({
            "slot_identity":"test/creche",
            "maximum_evidence_bytes":65536,"maximum_polls":1000,"minimum_poll_interval_ms":10,
            "image_manifest":"/explicit/missing-image.json","image_manifest_sha256":"a".repeat(64),
            "artifact":"/explicit/missing-image.bin","artifact_sha256":"b".repeat(64),
            "host_configuration":"/explicit/missing-destination.host.conduit","host_configuration_sha256":"c".repeat(64),
            "maximum_artifact_bytes":1048576,"maximum_manifest_bytes":65536,
        })).unwrap()
    }
    #[test]
    fn checked_creche_parameters_never_imply_opened_inputs_or_ready_provider() {
        let selected = selection();
        validate(&selected).unwrap();
        assert!(initialize(&selected).is_err());
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn uninitialized_creche_declaration_creates_no_provider_or_input_effect() {
        let source=include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit")
            .replace("  limits =", "  base = {kind: \"creche/encounter\", implementation: \"std/kernel-creche-prepare@1\"}\n  limits =");
        let selection =
            super::super::Selection::from_source(&source, "std/x86_64/computer").unwrap();
        assert!(selection.prepare_creche().unwrap().is_none());
    }
    #[test]
    fn creche_parameter_validation_refuses_relative_or_unpinned_inputs() {
        for (key, value) in [
            ("artifact", "relative.bin"),
            ("image_manifest_sha256", "a"),
            ("host_configuration_sha256", &"G".repeat(64)),
        ] {
            let mut selected = selection();
            selected.values.insert(key.into(), serde_json::json!(value));
            assert!(validate(&selected).is_err());
        }
    }
}
