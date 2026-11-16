//! Retain actual bytes returned by the selected JS browser producer.
//! Browser IMAGE construction/semantic validation remains in the existing
//! buildBrowserBundleImage/verifyBrowserBundleImage Host implementation.
use super::*;
use conduit_host_make::{verify_host_release_bytes, HostReleaseManifest, VerifiedHostReleaseBytes};
use std::io::Cursor;
pub(super) struct Derived {
    pub(super) image_bytes: Vec<u8>,
    pub(super) manifest: HostReleaseManifest,
    pub(super) verified: Option<VerifiedHostReleaseBytes>,
}
pub(super) fn retain(
    material: &staging::Material,
    configuration: &CheckedHostConfiguration,
    manifest_bytes: Vec<u8>,
    image_bytes: Vec<u8>,
) -> Result<Derived, String> {
    if material.verified.is_none()
        || manifest_bytes.is_empty()
        || manifest_bytes.len() > 256 * 1024
        || image_bytes.is_empty()
        || image_bytes.len() > 64 * 1024
    {
        return Err("Browser provider output exceeds its admitted input/IMAGE bounds".into());
    }
    let manifest: HostReleaseManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let review = crate::creche::womb_configuration_review(configuration)?;
    if manifest.source_identity != material.manifest.source_identity
        || manifest.files.len() != material.manifest.files.len() + 1
        || manifest.files[..material.manifest.files.len()] != material.manifest.files
        || manifest
            .target_metadata
            .get("distribution_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(material.manifest.bundle_sha256.as_str())
        || manifest.target_metadata.get("browser_configuration_id")
            != review.get("configuration_id")
        || manifest.target_metadata.get("browser_profile_id") != review.get("profile_id")
    {
        return Err(
            "Actual browser output differs from reserved configuration/distribution".into(),
        );
    }
    let image_file = manifest
        .files
        .last()
        .ok_or("Browser IMAGE output is absent")?;
    if image_file.path != "conduit-browser-image.json"
        || image_file.bytes != image_bytes.len() as u64
        || image_file.sha256 != format!("sha256:{:x}", Sha256::digest(&image_bytes))
    {
        return Err("Actual browser IMAGE bytes differ from derived release inventory".into());
    }
    let profile = conduit_host_make::ReleaseTargetProfile {
        builder_adapter: "conduit-host-browser/bind-prebuilt@1",
        ..staging::browser_profile()
    };
    let verified = verify_host_release_bytes(
        &manifest_bytes,
        &profile,
        &material.manifest.source_identity,
        staging::BOUNDS,
        |path| {
            if path == "conduit-browser-image.json" {
                return Ok(Cursor::new(image_bytes.as_slice()));
            }
            material
                .manifest
                .files
                .iter()
                .position(|file| file.path == path)
                .map(|index| Cursor::new(material.payloads[index].as_slice()))
                .ok_or_else(|| "Undeclared browser producer payload".into())
        },
        || false,
    )?;
    Ok(Derived {
        image_bytes,
        manifest,
        verified: Some(verified),
    })
}
