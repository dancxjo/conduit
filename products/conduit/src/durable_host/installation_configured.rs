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
    install_configured_with_image(
        manifest_path,
        state_dir,
        speech_change,
        model_change,
        todo_change,
        None,
    )
}

pub(super) fn install_configured_with_image(
    manifest_path: &Path,
    state_dir: &Path,
    speech_change: selected_speech::Change,
    model_change: selected_model::Change,
    todo_change: selected_todo::Change,
    image_path: Option<&Path>,
) -> Result<Installation, String> {
    install_configured_with_replacement(
        manifest_path,
        state_dir,
        speech_change,
        model_change,
        todo_change,
        image_path,
        None,
    )
}

/// One verified local software/IMAGE replacement; caller holds original Host and
/// Body locks until the existing installation transaction has completed.
#[allow(clippy::too_many_arguments)]
pub(super) fn install_configured_with_replacement(
    manifest_path: &Path,
    state_dir: &Path,
    speech_change: selected_speech::Change,
    model_change: selected_model::Change,
    todo_change: selected_todo::Change,
    image_path: Option<&Path>,
    configuration_path: Option<&Path>,
) -> Result<Installation, String> {
    if configuration_path.is_some() && image_path.is_none() {
        return Err("replacement configuration requires --host-image".into());
    }
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

    let install_path = state_dir.join("installation.json");
    let existing = if install_path.exists() {
        Some(installation_read::read_for_release_replacement(
            &install_path,
            &speech_change,
            &todo_change,
            configuration_path.is_some(),
        )?)
    } else {
        None
    };
    if configuration_path.is_some() && existing.is_none() {
        return Err("configuration replacement requires an existing installed Host".into());
    }
    // Validate the replacement before copying a release or committing installation state.
    let retained_configuration = existing
        .as_ref()
        .and_then(|value| value.selected_host_configuration.clone());
    let retained_configuration = selected_host_configuration::replacement_for_release(
        retained_configuration,
        configuration_path,
        image_path,
        &manifest.source_identity,
        &manifest_bytes,
    )?;
    // An original pending invitation authorizes only its exact joining Seed.
    // Historical completed packages are not active provisions and stay untouched.
    match existing.as_ref() {
        Some(existing) if pending_seed::has_pending_seed(state_dir)? => {
            let identity = |selected: &Option<selected_host_configuration::Selection>| {
                selected.as_ref().map(|selected| {
                    (
                        selected.configuration_id().to_owned(),
                        selected
                            .host_image()
                            .map(|image| image.manifest.image_id.clone()),
                    )
                })
            };
            if identity(&retained_configuration) != identity(&existing.selected_host_configuration)
            {
                return Err("pending joining Seed requires its original configuration and IMAGE until admission completes".into());
            }
        }
        _ => {}
    }
    let executable = manifest
        .files
        .iter()
        .find(|file| file.path.starts_with("conduit-") && !file.path.contains("tour"))
        .ok_or_else(|| "release has no installed Conduit product executable".to_string())?;
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
            Some(*selection)
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
    if let Some(configuration) = &retained_configuration {
        configuration.validate_speech_overlap(retained_selection.as_ref())?;
    }
    if configuration_path.is_some() && retained_todo.is_some() {
        return Err(
            "scoped Todo checkpoint Host cannot initialize additional checked equipment".into(),
        );
    }
    // Derive the existing immutable install destination without writing. The
    // complete replacement record must fit its reader before any release copy.
    let release_dir = state_dir.join("releases").join(
        manifest
            .bundle_sha256
            .strip_prefix("sha256:")
            .ok_or("invalid release digest")?,
    );
    let product_executable = release_dir.join(
        Path::new(&executable.path)
            .file_name()
            .ok_or_else(|| "product executable name is invalid".to_string())?,
    );
    let installation = Installation {
        schema: INSTALL_SCHEMA.into(),
        host_id,
        release_source_identity: manifest.source_identity.clone(),
        release_bundle_sha256: manifest.bundle_sha256.clone(),
        product_executable: product_executable.display().to_string(),
        body_state: existing.as_ref().and_then(|value| value.body_state.clone()),
        joined_body_state: existing.and_then(|value| value.joined_body_state),
        selected_speech: retained_selection,
        selected_model: retained_model,
        selected_todo_checkpoint: retained_todo,
        selected_host_configuration: retained_configuration,
    };
    let installation_bytes = bounded_installation_bytes(&installation)?;
    // Preflight only pure path/template constraints. Canonicalization requires
    // the real installed directory/executable and remains in the service writer.
    #[cfg(target_os = "linux")]
    {
        systemd_exec_argument(state_dir)?;
        systemd_exec_argument(Path::new(&installation.product_executable))?;
    }
    #[cfg(target_os = "macos")]
    {
        xml_text(state_dir)?;
        xml_text(Path::new(&installation.product_executable))?;
    }
    fs::create_dir_all(state_dir).map_err(|error| format!("create state directory: {error}"))?;
    restrict_directory(state_dir)?;
    crate::durable_host_control::ensure_secret(state_dir)?;
    install_immutable_release(bundle_dir, state_dir, &manifest)?;
    make_executable(Path::new(&installation.product_executable))?;
    write_bytes_atomic(&install_path, &installation_bytes)?;
    write_service_definition(state_dir, &installation)?;
    Ok(installation)
}

pub(super) fn bounded_installation_bytes(installation: &Installation) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec_pretty(installation)
        .map_err(|error| format!("encode installed Host state: {error}"))?;
    if bytes.len() > 64 * 1024 {
        return Err("installed Host state exceeds its 64 KiB read bound".into());
    }
    Ok(bytes)
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[path = "installation_configured/replacement_tests.rs"]
mod replacement_tests;
