//! Original ordinary Make IMAGE capture for host-side firmware preparation.
//! This reads the public description; private Seed/admission material is separate.
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf};
const MAXIMUM_BYTES: usize = 1024 * 1024;
pub fn from_environment(
    checked: &CheckedHostConfiguration,
    catalog: &MakeCatalog,
    image_id: &str,
    source_identity: &str,
) -> Result<HostImage, String> {
    for name in [
        "CONDUIT_HOST_IMAGE_DESCRIPTION",
        "CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let path = PathBuf::from(
        std::env::var_os("CONDUIT_HOST_IMAGE_DESCRIPTION")
            .ok_or("original Make IMAGE description required")?,
    );
    let expected = std::env::var("CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256")
        .map_err(|_| "original IMAGE description SHA256 required")?;
    println!("cargo:rerun-if-changed={}", path.display());
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| format!("original IMAGE: {e}"))?
        .take((MAXIMUM_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("original IMAGE: {e}"))?;
    decode(
        &bytes,
        &expected,
        checked,
        catalog,
        image_id,
        source_identity,
    )
}
pub fn decode(
    bytes: &[u8],
    expected_sha256: &str,
    checked: &CheckedHostConfiguration,
    catalog: &MakeCatalog,
    image_id: &str,
    source_identity: &str,
) -> Result<HostImage, String> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_BYTES {
        return Err("original IMAGE description bound".into());
    }
    if format!("{:x}", Sha256::digest(bytes)) != expected_sha256 {
        return Err("original IMAGE description SHA256 mismatch".into());
    }
    let image: HostImage =
        serde_json::from_slice(bytes).map_err(|e| format!("original IMAGE description: {e}"))?;
    if image.manifest.image_id != image_id || image.manifest.source_identity != source_identity {
        return Err("original IMAGE differs from selected fabrication identity".into());
    }
    conduit_host_make::verify_configuration_image(checked, &image, catalog)?;
    Ok(image)
}
