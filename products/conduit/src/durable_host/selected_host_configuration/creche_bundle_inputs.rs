//! Initialize bundle inputs through existing Make/target proof, before Ready.
use super::*;
use conduit::creche_encounter::CrecheReviewedBundle;
use conduit_host_make::{HostReleaseBounds, ReleaseTargetProfile};
use std::collections::BTreeMap;

pub(super) fn initialize_bundle(
    p: Parameters,
    image: HostImage,
    image_manifest: Vec<u8>,
    configuration: conduit_host_make::CheckedHostConfiguration,
    source: &str,
    make: (
        conduit_host_make::MakeCatalog,
        conduit_host_make::MakePackageSet,
    ),
) -> Result<CrecheEncounterFactory, String> {
    let (catalog, packages) = make;
    // For aggregate outputs, artifact selects the existing release manifest;
    // its pinned bytes authorize only the declared, subsequently verified files.
    let bytes = pinned_read(
        &p.artifact,
        &p.artifact_sha256,
        p.maximum_manifest_bytes.min(256 * 1024) as u64,
    )?;
    let directory = p
        .artifact
        .parent()
        .ok_or("Crèche bundle manifest has no directory")?;
    let output = serde_json::to_value(&image.manifest.output).map_err(|e| e.to_string())?;
    let expected = ReleaseTargetProfile {
        target_id: &image.manifest.target,
        package_id: &image.manifest.make_package_id,
        output: output.as_str().ok_or("Crèche output is not named")?,
        builder_adapter: &image.manifest.builder_adapter,
        deployment_adapter: image
            .manifest
            .deployment_adapter
            .as_deref()
            .ok_or("Crèche bundle has no deployment adapter")?,
    };
    let bounds = HostReleaseBounds {
        maximum_manifest_bytes: p.maximum_manifest_bytes.min(256 * 1024),
        maximum_files: 16,
        maximum_file_bytes: p.maximum_artifact_bytes.min(64 * 1024 * 1024),
        maximum_bundle_bytes: p.maximum_artifact_bytes.min(64 * 1024 * 1024),
    };
    let mut files = BTreeMap::new();
    let release = conduit_host_make::verify_host_release_bytes(
        &bytes,
        &expected,
        &image.manifest.source_identity,
        bounds,
        |name| {
            let path = directory.join(name);
            if !std::fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .is_file()
            {
                return Err("Crèche bundle payload is not a regular selected file".into());
            }
            let file = File::open(path).map_err(|e| e.to_string())?;
            files.insert(
                name.to_owned(),
                file.try_clone().map_err(|e| e.to_string())?,
            );
            Ok(file)
        },
        || false,
    )?;
    let payload_bytes = release.payload_bytes();
    let bundle = if image.manifest.output == conduit_host_make::SporeOutputKind::BrowserBundle {
        let distribution_bytes = bounded_read(
            &directory.join("browser-page.json"),
            bounds.maximum_manifest_bytes,
        )
        .map_err(|e| e.to_string())?;
        if distribution_bytes.len() > bounds.maximum_manifest_bytes {
            return Err("Crèche distribution manifest exceeds admitted bound".into());
        }
        let distribution_expected = ReleaseTargetProfile {
            builder_adapter: "conduit-host-browser/build-wasm@1",
            ..expected
        };
        let distribution = conduit_host_make::verify_host_release_bytes(
            &distribution_bytes,
            &distribution_expected,
            &image.manifest.source_identity,
            bounds,
            |name| File::open(directory.join(name)).map_err(|e| e.to_string()),
            || false,
        )?;
        let browser_image = bounded_read(
            &directory.join("conduit-browser-image.json"),
            bounds.maximum_manifest_bytes,
        )
        .map_err(|e| e.to_string())?;
        if browser_image.len() > bounds.maximum_manifest_bytes {
            return Err("Crèche browser IMAGE exceeds admitted bound".into());
        }
        let proof = conduit_host_browser_make::verify_browser_bundle_image(
            &configuration,
            &image,
            release,
            distribution,
            &browser_image,
        )?;
        CrecheReviewedBundle::browser(proof, files, p.package_directory.clone())?
    } else {
        CrecheReviewedBundle::native(release, files, p.package_directory.clone())?
    };
    // Compatibility field retains the exact selected manifest, not a falsely
    // labelled executable. Bundle production uses only its retained payloads.
    let artifact = File::open(&p.artifact).map_err(|e| e.to_string())?;
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
                maximum_receipt_bytes: p.maximum_evidence_bytes.min(16 * 1024 * 1024) as usize,
            },
            initial_source: Some(source.into()),
        },
        vec![CrecheReviewedArtifact {
            configuration,
            output,
            image,
            image_manifest,
            bundle: Some(bundle),
            artifact,
            artifact_digest: format!("sha256:{}", p.artifact_sha256),
            artifact_bytes: payload_bytes,
            source_identity,
        }],
    )
}

fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err("Crèche bundle metadata exceeds admitted manifest bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("Crèche bundle metadata grew beyond admitted bound".into());
    }
    Ok(bytes)
}
