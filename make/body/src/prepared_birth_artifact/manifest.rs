//! Exact resolved machinery and explicit native consumer declaration.
use super::*;
use conduit_body::PREPARED_BIRTH_MEDIA_SCHEMA;
use conduit_host_make::{BuildManifest, SporeOutputKind, BUILD_MANIFEST_SCHEMA, IMAGE_SCHEMA};
use serde_json::Value;

pub(super) struct BaseImage {
    pub image_id: String,
    pub sha256: String,
}

pub(super) fn verify_image_closure(image: &HostImage) -> Result<(), PreparedBirthArtifactRefusal> {
    use PreparedBirthArtifactRefusal as E;
    let bytes = serde_json::to_vec(image).map_err(|_| E::InvalidProfile)?;
    verify_image_binding(image, &bytes).map_err(|_| E::InvalidProfile)?;
    let m = &image.manifest;
    let p = &image.payload;
    if m.schema != BUILD_MANIFEST_SCHEMA
        || p.schema != IMAGE_SCHEMA
        || m.build_id.is_empty()
        || m.source_identity.is_empty()
        || m.toolchain_identity.is_empty()
        || p.profile_id != m.profile_id
        || p.build_id != m.build_id
        || p.target != m.target
        || p.source_configuration_id != m.source_configuration_id
        || p.make_descriptor != m.make_descriptor
        || p.profile_fragments != m.profile_fragments
        || p.implementations != m.implementations
        || p.host_calls != m.host_calls
        || p.resources != m.resources
        || p.resource_budgets != m.resource_budgets
        || p.bases != m.bases
        || p.base_selections != m.base_selections
        || p.drivers != m.drivers
        || p.driver_selections != m.driver_selections
        || p.lines != m.lines
        || p.presenters != m.presenters
        || p.facilities != m.facilities
        || p.bounds != m.bounds
    {
        return Err(E::InvalidProfile);
    }
    Ok(())
}

pub(super) fn verify(
    bytes: &[u8],
    image: &HostImage,
    media: &PreparedBirthMedia,
) -> Result<BaseImage, PreparedBirthArtifactRefusal> {
    use PreparedBirthArtifactRefusal as E;
    if bytes.len() > 1024 * 1024 {
        return Err(E::BoundExhausted);
    }
    verify_image_closure(image)?;
    let manifest: Value = serde_json::from_slice(bytes).map_err(|_| E::InvalidManifest)?;
    let m = &image.manifest;
    let binding = media.binding();
    if manifest["prepared_birth_media_schema"].as_str() != Some(PREPARED_BIRTH_MEDIA_SCHEMA) {
        return Err(E::UnsupportedConsumer);
    }
    let field = |name: &str, expected: &str| manifest[name].as_str() == Some(expected);
    let resolved: BuildManifest = serde_json::from_value(manifest["resolved_build"].clone())
        .map_err(|_| E::InvalidManifest)?;
    let sha = manifest["image"]["sha256"]
        .as_str()
        .ok_or(E::InvalidManifest)?;
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !field("schema", "conduit.host/target-build-manifest@3")
        || !field("artifact_role", "product-host")
        || !field("profile_id", &m.profile_id)
        || !field("build_id", &m.build_id)
        || !field("source_identity", &m.source_identity)
        || !field("target", &m.target)
        || !field("toolchain_identity", &m.toolchain_identity)
        || !field("resolved_description_binding", &m.image_id)
        || resolved != *m
        || m.output != SporeOutputKind::DiskImage
        || manifest["image"]["role"].as_str() != Some("final-bootable-image")
        || !field("image_id", &format!("image:sha256:{sha}"))
        || binding.profile_id != m.profile_id
        || binding.build_id != m.build_id
        || binding.target != m.target
        || Some(binding.configuration_id.as_str()) != m.source_configuration_id.as_deref()
    {
        return Err(E::InvalidManifest);
    }
    Ok(BaseImage {
        image_id: format!("image:sha256:{sha}"),
        sha256: format!("sha256:{sha}"),
    })
}
