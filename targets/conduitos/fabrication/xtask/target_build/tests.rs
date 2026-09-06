use conduit_host_fabrication::{build_default_host_image, BuildInputs, HostProfile};

use super::*;

fn resolved(source: &str) -> (BuildManifest, Vec<u8>) {
    let profile: HostProfile = serde_json::from_str(source).unwrap();
    build_default_host_image(
        profile,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
        &BuildInputs {
            source_identity: "test-source".into(),
            toolchain_available: true,
        },
    )
    .map(|(image, bytes)| (image.manifest, bytes))
    .unwrap()
}

#[test]
fn checked_native_profile_is_the_authority_for_the_first_target_lowering() {
    let (manifest, bytes) = resolved(include_str!(
        "../../../profiles/conduitos-native.profile.json"
    ));
    let built = build_profile_image(
        &manifest,
        &bytes,
        &GlobalOpts {
            dry_run: true,
            ..GlobalOpts::default()
        },
    )
    .unwrap();

    assert_eq!(manifest.target, "conduitos/x86_64/pc");
    assert_eq!(built.image_sha256, "dry-run");
}

#[test]
fn checked_headless_profile_enters_the_same_authoritative_target_lowering() {
    let (manifest, bytes) = resolved(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    ));
    let built = build_profile_image(
        &manifest,
        &bytes,
        &GlobalOpts {
            dry_run: true,
            ..GlobalOpts::default()
        },
    )
    .unwrap();

    assert_eq!(manifest.target, "conduitos/x86_64/pc");
    assert_eq!(built.image_sha256, "dry-run");
}

#[test]
fn checked_aarch64_profile_routes_to_the_distinct_product_artifact() {
    let (manifest, bytes) = resolved(include_str!(
        "../../../profiles/conduitos-aarch64-headless.profile.json"
    ));
    let built = build_profile_image(
        &manifest,
        &bytes,
        &GlobalOpts {
            dry_run: true,
            ..GlobalOpts::default()
        },
    )
    .unwrap();
    assert_eq!(manifest.target, "conduitos/aarch64/virt");
    assert_eq!(built.artifact_role, ArtifactRole::ProductHost);
    assert_eq!(built.image_sha256, "dry-run");
    assert!(arch_for_target("conduitos/aarch64/a3-proof").is_err());
}

#[test]
fn aarch64_product_sign_rejects_stale_bindings_and_false_capabilities() {
    let exact = serde_json::json!({
        "schema": "conduit.conduitos/aarch64-product@1",
        "status": "ready",
        "profile_id": "profile",
        "build_id": "build",
        "image_id": "image",
        "host_id": "host",
        "boot_id": "boot",
        "body_id": null,
        "interactive_local_control": false,
        "long_lived": true,
        "semantic_result": "HELLO, CONDUITOS",
        "presenter_implementation_id": "presenter/linear-serial@1"
    });
    assert!(validate_aarch64_product_sign(&exact, "profile", "build", "image").is_ok());
    for (field, stale) in [
        ("profile_id", "stale-profile"),
        ("build_id", "stale-build"),
        ("image_id", "stale-image"),
        (
            "presenter_implementation_id",
            "presenter/native-graphical@1",
        ),
    ] {
        let mut malformed = exact.clone();
        malformed[field] = stale.into();
        assert!(validate_aarch64_product_sign(&malformed, "profile", "build", "image").is_err());
    }
}

#[test]
fn incomplete_aarch64_product_sign_is_not_promoted_or_rejected_early() {
    assert_eq!(
        complete_aarch64_product_sign("firmware\0CONDUIT_AARCH64_PRODUCT {\"schema\":\"partial"),
        None
    );
    assert_eq!(
        complete_aarch64_product_sign(
            "firmware\0CONDUIT_AARCH64_PRODUCT {\"schema\":\"complete\"}\n"
        ),
        Some("{\"schema\":\"complete\"}")
    );
}
