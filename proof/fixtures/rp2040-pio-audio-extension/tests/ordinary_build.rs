use conduit_host_make::{
    build_host_image, BaseSelection, BuildInputs, DriverSelection, HostBounds, HostPolicy,
    HostProfile, MakeCatalog, MakePackageSet, SporeOutputKind, TargetSelection,
};
use conduit_host_rp2040::{Rp2040MakePackage, TARGET_ID};
use conduit_rp2040_pio_audio_extension::{Rp2040PioAudioExtension, IMPLEMENTATION_ID, PACKAGE_ID};

fn profile() -> HostProfile {
    HostProfile {
        schema: conduit_host_make::HOST_PROFILE_SCHEMA.into(),
        name: "rp2040-extension-proof".into(),
        source_configuration_id: Some("sha256:rp2040-extension-proof".into()),
        target: TargetSelection {
            family: "conduitos".into(),
            architecture: "thumbv6m".into(),
            machine: "pico-w".into(),
            build_profile: "release".into(),
            make_descriptor: None,
        },
        host_core: "host-core/conduitos@1".into(),
        fragments: Vec::new(),
        capabilities: Vec::new(),
        host_calls: Vec::new(),
        resources: Vec::new(),
        bases: vec![BaseSelection {
            id: "base/audio".into(),
            kind: "audio/pcm-output".into(),
            driver: IMPLEMENTATION_ID.into(),
        }],
        drivers: vec![DriverSelection {
            id: "driver/audio".into(),
            kind: IMPLEMENTATION_ID.into(),
        }],
        lines: Vec::new(),
        presenters: Vec::new(),
        facilities: Vec::new(),
        exclusions: Vec::new(),
        policy: HostPolicy {
            authority_profile: "authority/explicit@1".into(),
            trust_profile: "trust/local-explicit@1".into(),
            update_profile: "update/rebuild@1".into(),
            ambient_defaults: false,
        },
        bounds: HostBounds {
            static_memory_bytes: 64 * 1024,
            heap_arena_bytes: 64 * 1024,
            queue_items: 16,
            buffered_bytes: 4096,
            active_instances: 8,
            operation_slots: 8,
            timer_slots: 8,
            line_sessions: 1,
            evidence_items: 16,
        },
    }
}

#[test]
fn extension_is_absent_until_composed_then_survives_profile_build_image_provenance() {
    let anchor_only = MakePackageSet::compose(&[&Rp2040MakePackage]).unwrap();
    assert!(anchor_only
        .offers_for_target(TARGET_ID)
        .iter()
        .all(|offer| offer.offer.implementation_id != IMPLEMENTATION_ID));

    let packages =
        MakePackageSet::compose(&[&Rp2040MakePackage, &Rp2040PioAudioExtension]).unwrap();
    let profile = profile();
    let selection = packages
        .derive_build_selection(&profile, &SporeOutputKind::Uf2)
        .unwrap();
    assert_eq!(selection.features, ["base-pio-audio"]);
    assert_eq!(selection.implementation_packages.len(), 1);
    assert_eq!(selection.implementation_packages[0].package_id, PACKAGE_ID);

    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let (image, bytes) = build_host_image(
        profile,
        &catalog,
        &packages,
        &SporeOutputKind::Uf2,
        &BuildInputs {
            source_identity: "git:rp2040-extension-proof".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert_eq!(image.manifest.target, TARGET_ID);
    assert!(!bytes.is_empty());
    assert_eq!(selection.selected_base_implementations, [IMPLEMENTATION_ID]);
    assert_eq!(image.manifest.make_package_id, "conduit-host-rp2040@1");
    assert_eq!(
        image.manifest.implementation_packages[0].package_id,
        PACKAGE_ID
    );
}
