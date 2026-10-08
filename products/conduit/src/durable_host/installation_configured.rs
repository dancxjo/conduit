//! Review and persist exact installed equipment selections.
use super::*;

#[cfg(test)]
pub(super) fn install_configured(
    manifest_path: &Path,
    state_dir: &Path,
    speech_change: selected_speech::Change,
    model_change: selected_model::Change,
) -> Result<Installation, String> {
    install_configured_with_todo(
        manifest_path,
        state_dir,
        speech_change,
        model_change,
        selected_todo::Change::Preserve,
    )
}

pub(super) fn install_configured_with_todo(
    manifest_path: &Path,
    state_dir: &Path,
    speech_change: selected_speech::Change,
    model_change: selected_model::Change,
    todo_change: selected_todo::Change,
) -> Result<Installation, String> {
    let manifest_bytes = bounded_read(manifest_path, 256 * 1024)?;
    let manifest: ReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("release manifest: {error}"))?;
    validate_manifest(&manifest)?;
    let bundle_dir = manifest_path
        .parent()
        .ok_or_else(|| "release manifest has no bundle directory".to_string())?;
    for file in &manifest.files {
        verify_release_file(bundle_dir, file)?;
    }

    fs::create_dir_all(state_dir).map_err(|error| format!("create state directory: {error}"))?;
    restrict_directory(state_dir)?;
    crate::durable_host_control::ensure_secret(state_dir)?;
    let install_path = state_dir.join("installation.json");
    let existing = if install_path.exists() {
        Some(read_installation_for_equipment_change(
            &install_path,
            &speech_change,
            &todo_change,
        )?)
    } else {
        None
    };
    let executable = manifest
        .files
        .iter()
        .find(|file| file.path.starts_with("conduit-") && !file.path.contains("tour"))
        .ok_or_else(|| "release has no installed Conduit product executable".to_string())?;
    let release_dir = install_immutable_release(bundle_dir, state_dir, &manifest)?;
    let product_executable = release_dir.join(
        Path::new(&executable.path)
            .file_name()
            .ok_or_else(|| "product executable name is invalid".to_string())?,
    );
    make_executable(&product_executable)?;
    let host_id = existing
        .as_ref()
        .map(|value| value.host_id.clone())
        .unwrap_or_else(|| fresh_identity("host/installed", &manifest.bundle_sha256));
    let retained_selection = match speech_change {
        selected_speech::Change::Preserve => existing
            .as_ref()
            .and_then(|value| value.selected_speech.clone()),
        selected_speech::Change::Replace(selection) => {
            selection.validate()?;
            Some(selection)
        }
        selected_speech::Change::Remove => None,
    };
    let retained_model = match model_change {
        selected_model::Change::Preserve => existing
            .as_ref()
            .and_then(|value| value.selected_model.clone()),
        selected_model::Change::Replace(selection) => {
            selection.validate()?;
            Some(*selection)
        }
        selected_model::Change::Remove => None,
    };
    let retained_todo = match todo_change {
        selected_todo::Change::Preserve => existing
            .as_ref()
            .and_then(|value| value.selected_todo_checkpoint.clone()),
        selected_todo::Change::Replace(selection) => {
            selection.validate()?;
            Some(selection)
        }
        selected_todo::Change::Remove => None,
    };
    let installation = Installation {
        schema: INSTALL_SCHEMA.into(),
        host_id,
        release_source_identity: manifest.source_identity,
        release_bundle_sha256: manifest.bundle_sha256,
        product_executable: product_executable.display().to_string(),
        body_state: existing.as_ref().and_then(|value| value.body_state.clone()),
        joined_body_state: existing.and_then(|value| value.joined_body_state),
        selected_speech: retained_selection,
        selected_model: retained_model,
        selected_todo_checkpoint: retained_todo,
    };
    write_json_atomic(&install_path, &installation)?;
    write_service_definition(state_dir, &installation)?;
    Ok(installation)
}
