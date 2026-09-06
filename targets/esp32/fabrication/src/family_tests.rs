use conduit_host_fabrication::{
    build_default_host_image, validate_profile, BaseSelection, BuildInputs, DriverSelection,
    FabricationCatalog, FabricationContribution, FabricationPackageSet, HostFabricationPackage,
    HostProfile, PostBuildAction,
};

use crate::{esp32_descriptor_binding, Esp32FabricationPackage, Esp32FamilyTarget};

const HEADLESS: &str = include_str!("../../../conduitos/profiles/conduitos-headless.profile.json");

fn profile(target: Esp32FamilyTarget) -> HostProfile {
    let facts = target.facts();
    let mut profile: HostProfile = serde_json::from_str(HEADLESS).unwrap();
    profile.name = format!("esp32-family-{}", facts.selector);
    profile.target.family = "esp32".into();
    profile.target.architecture = facts.architecture.into();
    profile.target.machine = facts.machine.into();
    profile.target.fabrication_descriptor = Some(
        esp32_descriptor_binding(&target.board_descriptor()).expect("family descriptor must bind"),
    );
    profile.bases = vec![
        BaseSelection {
            id: "base/kernel".into(),
            kind: "kernel/signal".into(),
            driver: "esp32/kernel-signal@1".into(),
        },
        BaseSelection {
            id: "base/bluetooth-line".into(),
            kind: "line/bluetooth-le-gatt".into(),
            driver: "esp32/bluetooth-le-gatt@1".into(),
        },
    ];
    profile.drivers = vec![
        DriverSelection {
            id: "esp32/kernel-signal@1".into(),
            kind: "esp32/kernel-signal@1".into(),
        },
        DriverSelection {
            id: "esp32/bluetooth-le-gatt@1".into(),
            kind: "esp32/bluetooth-le-gatt@1".into(),
        },
    ];
    profile.lines.clear();
    profile.exclusions.clear();
    profile
}

#[test]
fn package_exposes_exact_finite_wroom_c3_s3_family() {
    let FabricationContribution::Anchor(anchor) = Esp32FabricationPackage.contribution() else {
        panic!("ESP32 family must remain an anchor package")
    };
    assert_eq!(anchor.targets.len(), Esp32FamilyTarget::ALL.len());
    for target in Esp32FamilyTarget::ALL {
        let facts = target.facts();
        let descriptor = anchor
            .targets
            .iter()
            .find(|candidate| {
                candidate.architecture == facts.architecture && candidate.machine == facts.machine
            })
            .expect("every accepted target must be package-owned");
        assert_eq!(descriptor.builder_adapter, facts.builder_adapter);
        assert_eq!(
            descriptor.deployment_adapter.as_deref(),
            facts.deployment_adapter
        );
        assert_eq!(
            descriptor.post_build_actions,
            [PostBuildAction::Flash, PostBuildAction::Boot]
        );
    }
    assert!("esp32-c6".parse::<Esp32FamilyTarget>().is_err());
}

#[test]
fn every_family_target_reaches_ordinary_profile_build_and_exact_provenance() {
    let packages = FabricationPackageSet::compose(&[&Esp32FabricationPackage]).unwrap();
    let catalog = FabricationCatalog::canonical().with_packages(&packages);
    for target in Esp32FamilyTarget::ALL {
        let checked = validate_profile(profile(target), &catalog).unwrap();
        let (image, bytes) = build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: "git:esp32-family-fixture".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        let facts = target.facts();
        assert_eq!(
            image.manifest.target,
            format!("esp32/{}/{}", facts.architecture, facts.machine)
        );
        assert_eq!(image.manifest.toolchain_identity, facts.toolchain_identity);
        assert_eq!(image.manifest.builder_adapter, facts.builder_adapter);
        assert!(!bytes.is_empty());
    }
}
