//! Fresh ordinary Make/Source emit-only calibration. No target compilation,
//! runtime admission, grant issuance, provider initialization or physical IO.
use conduit_mcu_source_build as source_build;
#[allow(dead_code)]
#[path = "../fixtures/environmental_exchange_source.rs"]
mod environmental_source;
#[path = "../../../../rp2040/firmware/host/i2c_workset_build.rs"]
mod i2c_workset_build;
use conduit_host_make as h;
use sha2::{Digest, Sha256};
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("explicit repository"));
    let out = std::path::PathBuf::from(std::env::args_os().nth(2).expect("explicit output"));
    std::fs::create_dir_all(&out).unwrap();
    let packages = h::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let catalog = h::MakeCatalog::canonical().with_packages(&packages);
    let checked = h::check_host_configuration(
        h::parse_host_configuration_conduit(
            &std::fs::read_to_string(root.join(
                "targets/rp2040/profiles/pico-w-house-green-source-work-candidate.host.conduit",
            ))
            .unwrap(),
        )
        .unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    assert_eq!(checked.configuration().limits.heap_arena_bytes, 49152);
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&checked).unwrap();
    assert_eq!(
        (work.capacity_units, work.bookkeeping_bytes),
        (15117, 16384)
    );
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
    let result = i2c_workset_build::prepare(
        &checked,
        &catalog,
        &image.manifest.image_id,
        &identity,
        &out,
    );
    let (prepared, refusal) = match result {
        Ok(Some(residents)) => (residents.len(), None),
        Ok(None) => panic!("explicit Source disappeared"),
        Err(error) => (0, Some(error)),
    };
    println!(
        "{}",
        serde_json::json!({
            "scope":"fresh checked Host IMAGE and ordinary BME Source calibration; emit-only",
            "configuration":checked.configuration_id(), "image":image.manifest.image_id,
            "resident_work_units":work.capacity_units, "bookkeeping_bytes":work.bookkeeping_bytes,
            "heap_bytes":checked.configuration().limits.heap_arena_bytes,
            "prepared_resident_count":prepared, "calibration_refusal":refusal,
            "whole_runtime_factory_called":false, "runtime_admission":false, "hardware_effects":false,
        })
    );
}
