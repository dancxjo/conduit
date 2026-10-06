//! Bounded installation decoding and identity validation before use or explicit repair.
use super::{
    bounded_read, digest, membership, selected_speech, valid_digest, Installation, INSTALL_SCHEMA,
};
use std::path::Path;

pub(super) fn read_installation(path: &Path) -> Result<Installation, String> {
    read(path, false)
}

pub(super) fn read_installation_for_equipment_change(
    path: &Path,
    change: &selected_speech::Change,
) -> Result<Installation, String> {
    read(
        path,
        matches!(
            change,
            selected_speech::Change::Replace(_) | selected_speech::Change::Remove
        ),
    )
}

fn read(path: &Path, explicit_reselection: bool) -> Result<Installation, String> {
    let bytes = bounded_read(path, 64 * 1024)?;
    let value: Installation =
        serde_json::from_slice(&bytes).map_err(|error| format!("installation state: {error}"))?;
    if value.schema != INSTALL_SCHEMA
        || value.host_id.is_empty()
        || !valid_digest(&value.release_bundle_sha256)
    {
        return Err("installation state is invalid".into());
    }
    if let Some(selection) = &value.selected_speech {
        if explicit_reselection {
            selection.validate_for_reselection()?;
        } else {
            selection.validate()?;
        }
    }
    if let Some(selection) = &value.selected_model {
        selection.validate()?;
    }
    if let Some(binding) = &value.body_state {
        if binding.body_id.is_empty()
            || !valid_digest(&binding.biography_sha256)
            || digest(&bounded_read(
                Path::new(&binding.biography_path),
                2 * 1024 * 1024,
            )?) != binding.biography_sha256
        {
            return Err("retained body biography identity is invalid or stale".into());
        }
    }
    if value.body_state.is_some() && value.joined_body_state.is_some() {
        return Err("installed host cannot own and join different Body state".into());
    }
    if let Some(binding) = &value.joined_body_state {
        if !valid_digest(&binding.credential_sha256) {
            return Err("retained body membership credential is invalid or stale".into());
        }
        membership::validate(binding, &value)?;
    }
    Ok(value)
}
