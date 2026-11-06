//! Fabricated bytes remain definitions; no fake provider or MCU decoder needed.
use conduit_host_make::*;
use conduit_host_rp2040::{checked_i2c_offer_image, Rp2040MakePackage};
use conduit_wire::{
    resolve_capability_offer_images, CapabilityOfferImage, CapabilityOfferImageLimits,
};
use sha2::{Digest, Sha256};
fn checked(source: &str) -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&Rp2040MakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
#[test]
fn native_image_round_trips_the_exact_definition_with_independent_bounds() {
    let selected = checked(include_str!(
        "../../profiles/pico-w-house-green-delegated.host.conduit"
    ));
    let artifact = "fixture/exact-checked-image";
    let bytes = checked_i2c_offer_image(&selected, artifact, 24 * 1024)
        .unwrap()
        .unwrap();
    assert_eq!(
        checked_i2c_offer_image(&selected, artifact, bytes.len()).unwrap(),
        Some(bytes.clone())
    );
    assert!(checked_i2c_offer_image(&selected, artifact, bytes.len() - 1).is_err());
    assert!(checked_i2c_offer_image(&selected, "", 24 * 1024).is_err());
    let image = CapabilityOfferImage {
        sha256: Sha256::digest(&bytes).into(),
        bytes: &bytes,
    };
    let mut scratch = vec![0; 24 * 1024];
    let expanded_limit = scratch.len();
    let resolved = resolve_capability_offer_images(
        &[image],
        &mut scratch,
        CapabilityOfferImageLimits {
            maximum_offers: 1,
            maximum_image_bytes: bytes.len(),
            maximum_expanded_offer_bytes: expanded_limit,
            maximum_total_image_bytes: bytes.len(),
            maximum_total_expanded_bytes: expanded_limit,
        },
    )
    .unwrap();
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare().unwrap();
    let expected = conduit_i2c_transaction::installation::capability_offer(
        &contract,
        &conduit_core::ArtifactId::from(artifact),
    )
    .unwrap();
    assert_eq!(resolved, vec![expected]);
    println!(
        "native definition image: {} flash bytes; no runtime readiness implied",
        bytes.len()
    );
}
#[test]
fn disabled_native_policy_fabricates_no_native_image() {
    let plain = checked(include_str!(
        "../../profiles/pico-w-house-green.host.conduit"
    ));
    assert_eq!(checked_i2c_offer_image(&plain, "", 0).unwrap(), None);
}
