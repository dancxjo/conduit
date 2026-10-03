//! Digest-bound producer outputs in one evidence run.
use super::owner::OwnerSnapshot;
use crate::evidence::{EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance};
use std::{fs, io::Write};

pub(super) fn retain_json(
    manifest: &mut EvidenceManifest,
    id: &str,
    path: &str,
    value: &impl serde::Serialize,
    run_id: &str,
    face: &OwnerSnapshot,
    proof_class: Option<&str>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    retain(
        manifest,
        id,
        path,
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &bytes,
        run_id,
        face,
        proof_class,
    )
}

// Every artifact carries the same exact run and Face provenance.
#[allow(clippy::too_many_arguments)]
pub(super) fn retain(
    manifest: &mut EvidenceManifest,
    id: &str,
    path: &str,
    kind: EvidenceKind,
    media_type: &str,
    bytes: &[u8],
    run_id: &str,
    face: &OwnerSnapshot,
    proof_class: Option<&str>,
) -> Result<(), String> {
    let path_buf = manifest.root().join(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path_buf)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    declare(
        manifest,
        id,
        path,
        kind,
        media_type,
        run_id,
        face,
        proof_class,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn declare(
    manifest: &mut EvidenceManifest,
    id: &str,
    path: &str,
    kind: EvidenceKind,
    media_type: &str,
    run_id: &str,
    face: &OwnerSnapshot,
    proof_class: Option<&str>,
) -> Result<(), String> {
    manifest.declare(EvidenceOutput {
        id: id.into(),
        kind,
        path: path.into(),
        media_type: media_type.into(),
        required: true,
        provenance: EvidenceProvenance {
            scenario_id: run_id.into(),
            presentation_id: Some(face.presentation.identity.as_str().into()),
            presentation_revision: Some(face.presentation.revision.to_string()),
            host_id: Some(face.advertisement.host_id.as_str().into()),
            boot_id: Some(face.advertisement.boot_id.as_str().into()),
            proof_class: proof_class.map(str::to_owned),
            ..EvidenceProvenance::default()
        },
    })
}
