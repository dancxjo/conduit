//! Local artifact review reuses the ordinary release inventory and verifier.
//! It never asserts a native compilation receipt, initialized Base or Source authority.
use super::*;
use conduit_host_make::{
    verify_configuration_image, verify_host_release_bytes, verify_image_binding, HostImage,
    HostReleaseBounds, HostReleaseManifest, ReleaseTargetProfile, SporeOutputKind,
};

pub(crate) fn review_artifact(
    configuration: &Path,
    image_path: &Path,
    executable: &Path,
    expected: &str,
    output: &Path,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("review requires an exact lower-case executable SHA256".into());
    }
    let checked = configuration_load::load_configuration(configuration)?;
    let image_metadata = fs::metadata(image_path)?;
    if !image_metadata.is_file() || image_metadata.len() > 64 * 1024 {
        return Err("reviewed IMAGE exceeds its finite manifest bound".into());
    }
    let bytes = fs::read(image_path)?;
    let image: HostImage = serde_json::from_slice(&bytes)?;
    verify_image_binding(&image, &bytes).map_err(|e| format!("reviewed IMAGE: {e:?}"))?;
    verify_configuration_image(&checked, &image, &conduit_workspace_make::catalog())
        .map_err(|e| format!("reviewed Host source/IMAGE: {e:?}"))?;
    if image.manifest.output != SporeOutputKind::NativeBundle {
        return Err("existing artifact review supports only native bundles".into());
    }
    let selection = conduit_workspace_make::package_set()
        .derive_build_selection(checked.profile(), &SporeOutputKind::NativeBundle)
        .map_err(|e| format!("reviewed Make selection: {e:?}"))?;
    let name = checked::native_artifact_name(&image.manifest.target, &selection)?;
    let metadata = fs::metadata(executable)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAXIMUM_NATIVE_FILE_BYTES {
        return Err("reviewed executable exceeds its finite file bound".into());
    }
    let expected = format!("sha256:{expected}");
    if sha256_file(executable)? != expected {
        return Err("reviewed executable differs from the operator-selected digest".into());
    }
    if output.exists() {
        return Err("review output already exists; refusing replacement".into());
    }
    if dry_run {
        return Ok(());
    }
    fs::create_dir(output)?;
    copy(executable, &output.join(name))?;
    if sha256_file(&output.join(name))? != expected {
        return Err("copied executable differs from the operator-selected digest".into());
    }
    let deployment = image
        .manifest
        .deployment_adapter
        .as_deref()
        .ok_or("native deployment adapter absent")?;
    seal(
        output,
        "reviewed-artifact.json",
        &image.manifest.target,
        &image.manifest.make_package_id,
        "native-bundle",
        &image.manifest.builder_adapter,
        deployment,
        &image.manifest.source_identity,
        &[(name, "application/vnd.conduit.host+executable")],
    )?;
    let path = output.join("reviewed-artifact.json");
    let mut manifest: HostReleaseManifest = serde_json::from_slice(&fs::read(&path)?)?;
    manifest.target_metadata.insert("operator_reviewed_artifact".into(), serde_json::json!({
        "schema": "conduit.diagnostic/local-artifact-review@1",
        "executable_sha256": expected,
        "configuration_id": checked.configuration_id(),
        "image_id": image.manifest.image_id,
        "checked_native_build": false,
        "scope": "operator-selected exact executable bytes and independently verified Make IMAGE; no compilation attestation, initialized Base, Source adoption, admission or execution authority",
    }));
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    verify_host_release_bytes(
        &bytes,
        &ReleaseTargetProfile {
            target_id: &image.manifest.target,
            package_id: &image.manifest.make_package_id,
            output: "native-bundle",
            builder_adapter: &image.manifest.builder_adapter,
            deployment_adapter: deployment,
        },
        &image.manifest.source_identity,
        HostReleaseBounds {
            maximum_manifest_bytes: 64 * 1024,
            maximum_files: 1,
            maximum_file_bytes: MAXIMUM_NATIVE_FILE_BYTES,
            maximum_bundle_bytes: MAXIMUM_NATIVE_FILE_BYTES,
        },
        |name| fs::File::open(output.join(name)).map_err(|e| e.to_string()),
        || false,
    )?;
    fs::write(path, bytes)?;
    println!(
        "Reviewed local artifact; no checked native build receipt was issued: {}",
        output.display()
    );
    Ok(())
}
