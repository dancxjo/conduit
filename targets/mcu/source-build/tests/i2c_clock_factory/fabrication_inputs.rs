//! Shared ordinary checked Host/IMAGE and exact Source capture for host probes.
use super::environmental_source;
use conduit_host_make as h;
use sha2::{Digest, Sha256};
use std::path::Path;
pub struct Inputs {
    pub checked: h::CheckedHostConfiguration,
    pub catalog: h::MakeCatalog,
    pub image: h::HostImage,
    pub identity: String,
}
pub fn prepare(out: &Path, profile: &Path) -> Inputs {
    std::fs::create_dir_all(out).unwrap();
    let packages = h::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = h::MakeCatalog::canonical().with_packages(&packages);
    let checked = h::check_host_configuration(
        h::parse_host_configuration_conduit(&std::fs::read_to_string(profile).unwrap()).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let fixture = environmental_source::selected_source();
    let identity = format!("cpu-checked-source:{}", fixture.source_sha256);
    let (image, _) = h::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &h::BuildInputs {
            source_identity: identity.clone(),
            toolchain_available: true,
        },
    )
    .unwrap();
    h::verify_configuration_image(&checked, &image, &catalog).unwrap();
    let source = out.join("source.conduit");
    let receipt = out.join("typed-selections.json");
    let description = out.join("image-description.json");
    let bytes = serde_json::to_vec(&image).unwrap();
    std::fs::write(&source, &fixture.source).unwrap();
    std::fs::write(&receipt, &fixture.typed_receipt).unwrap();
    std::fs::write(&description, &bytes).unwrap();
    for (key, value) in [
        (
            "CONDUIT_WORKSET_SOURCE",
            source.to_string_lossy().into_owned(),
        ),
        ("CONDUIT_WORKSET_SOURCE_SHA256", fixture.source_sha256),
        ("CONDUIT_WORKSET_ENTRIES", "[\"shared-environment\"]".into()),
        (
            "CONDUIT_WORKSET_SPECIALIZATIONS",
            receipt.to_string_lossy().into_owned(),
        ),
        (
            "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256",
            fixture.typed_receipt_sha256,
        ),
        (
            "CONDUIT_HOST_IMAGE_DESCRIPTION",
            description.to_string_lossy().into_owned(),
        ),
        (
            "CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256",
            format!("{:x}", Sha256::digest(&bytes)),
        ),
    ] {
        std::env::set_var(key, value);
    }
    Inputs {
        checked,
        catalog,
        image,
        identity,
    }
}
