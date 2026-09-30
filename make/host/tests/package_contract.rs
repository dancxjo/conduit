use conduit_host_make::{
    HostBounds, HostMakePackage, ImplementationOffer, MakeAnchor, MakeContribution, MakeExtension,
    MakePackageSet, PackageCompositionDiagnostic, PostBuildAction, SporeOutputKind,
    TargetDescriptor,
};

fn maxima() -> HostBounds {
    HostBounds {
        static_memory_bytes: 262_144,
        heap_arena_bytes: 0,
        queue_items: 32,
        buffered_bytes: 4096,
        active_instances: 16,
        operation_slots: 8,
        timer_slots: 8,
        line_sessions: 2,
        evidence_items: 32,
    }
}

fn offer(kind: &str, implementation: &str, feature: &str) -> ImplementationOffer {
    ImplementationOffer {
        base_kind: kind.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec!["conduitos/thumbv6m/pico-w".into()],
        prerequisites: Vec::new(),
        build_feature: Some(feature.into()),
    }
}

fn pico_anchor() -> MakeAnchor {
    MakeAnchor {
        package_id: "conduit-host-rp2040@1".into(),
        package_revision: 1,
        catalog: Default::default(),
        targets: vec![TargetDescriptor {
            label: "Pico W".into(),
            family: "conduitos".into(),
            architecture: "thumbv6m".into(),
            machine: "pico-w".into(),
            board: Some("pico-w".into()),
            os: None,
            host_core: "host-core/conduitos@1".into(),
            presenter: None,
            host_calls: Vec::new(),
            toolchain_identity: "rustc:stable+thumbv6m-none-eabi".into(),
            builder_adapter: "conduit-host-rp2040/build@1".into(),
            strategy: conduit_host_make::MakeStrategy::DeterministicSpecializedBuild,
            deployment_adapter: Some("conduit-host-rp2040/flash-uf2@1".into()),
            outputs: vec![SporeOutputKind::Uf2],
            default_output: SporeOutputKind::Uf2,
            post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
            make_descriptors: Vec::new(),
            maxima: maxima(),
        }],
        offers: vec![offer("serial/text", "pico/usb-cdc@1", "line-usb-cdc")],
    }
}

fn audio_extension(package_id: &str, implementation: &str) -> MakeExtension {
    MakeExtension {
        package_id: package_id.into(),
        package_revision: 1,
        catalog: Default::default(),
        compatible_target_patterns: vec!["conduitos/thumbv6m/*".into()],
        offers: vec![offer("audio/pcm-output", implementation, "base-pio-audio")],
    }
}

struct Pico;
impl HostMakePackage for Pico {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(pico_anchor())
    }
}

struct Audio;
impl HostMakePackage for Audio {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Extension(audio_extension(
            "example-rp2040-audio@1",
            "example/rp2040-pio-audio@1",
        ))
    }
}

#[test]
fn independently_authored_extension_adds_an_explicit_offer() {
    let anchor_only = MakePackageSet::compose(&[&Pico]).unwrap();
    assert_eq!(
        anchor_only
            .offers_for_target("conduitos/thumbv6m/pico-w")
            .len(),
        1
    );
    let composed = MakePackageSet::compose(&[&Audio, &Pico]).unwrap();
    let offers = composed.offers_for_target("conduitos/thumbv6m/pico-w");
    assert_eq!(
        offers
            .iter()
            .map(|item| (
                item.package_id.as_str(),
                item.offer.implementation_id.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("example-rp2040-audio@1", "example/rp2040-pio-audio@1"),
            ("conduit-host-rp2040@1", "pico/usb-cdc@1"),
        ]
    );
}

#[test]
fn duplicate_identity_refuses_independently_of_composition_order() {
    let conflict = audio_extension("conflicting-package@1", "pico/usb-cdc@1");
    let left = MakePackageSet::from_contributions([
        MakeContribution::Anchor(pico_anchor()),
        MakeContribution::Extension(conflict.clone()),
    ])
    .unwrap_err();
    let right = MakePackageSet::from_contributions([
        MakeContribution::Extension(conflict),
        MakeContribution::Anchor(pico_anchor()),
    ])
    .unwrap_err();
    assert_eq!(left, right);
    assert!(left.iter().any(|item| matches!(
        item,
        PackageCompositionDiagnostic::DuplicateImplementationIdentity {
            implementation_id,
            ..
        } if implementation_id == "pico/usb-cdc@1"
    )));
}

#[test]
fn lightweight_inspection_exposes_only_adapter_and_toolchain_identities() {
    let packages = MakePackageSet::compose(&[&Pico]).unwrap();
    let anchor = packages
        .anchor_for_target("conduitos/thumbv6m/pico-w")
        .unwrap();
    assert_eq!(
        anchor.targets[0].builder_adapter,
        "conduit-host-rp2040/build@1"
    );
    assert_eq!(anchor.targets[0].outputs, [SporeOutputKind::Uf2]);
}
