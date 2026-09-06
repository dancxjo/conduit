use conduit_host_fabrication::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, PostBuildAction, SporeOutputKind,
};
use std::path::Path;

fn build_configuration(source: &str) -> conduit_host_fabrication::HostImage {
    let packages = conduit_workspace_fabrication::package_set();
    let catalog = conduit_workspace_fabrication::catalog();
    let profile = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
    .into_profile();
    build_default_host_image(
        profile,
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "git:family-contract-proof".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}

#[test]
fn native_and_browser_are_not_flash_shaped() {
    let clock_host = build_configuration(include_str!(
        "../../../targets/std/profiles/linux-clock.host.conduit"
    ));
    let serial_host = build_configuration(include_str!(
        "../../../targets/std/profiles/linux-serial.host.conduit"
    ));
    let browser = build_configuration(include_str!(
        "../../../targets/browser/profiles/browser-page.host.conduit"
    ));

    assert_eq!(clock_host.manifest.output, SporeOutputKind::NativeBundle);
    assert_eq!(serial_host.manifest.output, SporeOutputKind::NativeBundle);
    assert_eq!(
        clock_host.manifest.post_build_actions,
        [PostBuildAction::Launch]
    );
    assert_eq!(
        serial_host.manifest.post_build_actions,
        [PostBuildAction::Launch]
    );
    assert_ne!(
        clock_host.manifest.base_selections, serial_host.manifest.base_selections,
        "one hosted package must preserve narrower construction choices"
    );
    assert_eq!(browser.manifest.output, SporeOutputKind::BrowserBundle);
    assert_eq!(
        browser.manifest.post_build_actions,
        [PostBuildAction::Load, PostBuildAction::Launch]
    );
    for image in [&clock_host, &serial_host, &browser] {
        assert!(!image
            .manifest
            .post_build_actions
            .contains(&PostBuildAction::Flash));
        assert!(!image
            .manifest
            .post_build_actions
            .contains(&PostBuildAction::Boot));
    }
}

#[test]
fn exact_conduitos_and_raspberry_pi_families_have_one_owner_each() {
    let packages = conduit_workspace_fabrication::package_set();
    let conduitos = [
        "conduitos/x86_64/pc",
        "conduitos/ia32/pc",
        "conduitos/aarch64/virt",
        "conduitos/riscv64/virt",
        "conduitos/loongarch64/virt",
    ];
    for target in conduitos {
        let descriptor = packages.target_descriptor(target).unwrap();
        assert_eq!(
            packages.anchor_for_target(target).unwrap().package_id,
            "conduitos-image@1"
        );
        assert_eq!(descriptor.default_output, SporeOutputKind::DiskImage);
        assert_eq!(descriptor.post_build_actions, [PostBuildAction::Boot]);
    }

    for target in [
        "conduitos/armv6/raspberry-pi-model-b-plus-v1.2",
        "conduitos/armv6/raspberry-pi-zero-v1",
        "conduitos/armv6/raspberry-pi-zero-w-v1.1",
        "conduitos/armv6/raspberry-pi-zero-wh-v1.1",
    ] {
        let descriptor = packages.target_descriptor(target).unwrap();
        assert_eq!(
            packages.anchor_for_target(target).unwrap().package_id,
            "conduit-host-raspberry-pi@1"
        );
        assert_eq!(descriptor.default_output, SporeOutputKind::SdImage);
        assert_eq!(
            descriptor.post_build_actions,
            [PostBuildAction::Flash, PostBuildAction::Boot]
        );
    }

    let pi_os = packages
        .target_descriptor("std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb")
        .unwrap();
    assert_eq!(
        packages
            .anchor_for_target("std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb")
            .unwrap()
            .package_id,
        "conduit-host-raspberry-pi@1"
    );
    assert_eq!(pi_os.os.as_deref(), Some("raspberry-pi-os-bookworm-64"));
    assert_eq!(pi_os.default_output, SporeOutputKind::NativeBundle);
    assert_eq!(pi_os.post_build_actions, [PostBuildAction::Launch]);
    assert!(!pi_os.post_build_actions.contains(&PostBuildAction::Flash));

    for target in [
        "std/aarch64/raspberry-pi-zero-2-w-rev-1.0",
        "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0",
    ] {
        let zero_2 = packages.target_descriptor(target).unwrap();
        assert_eq!(
            packages.anchor_for_target(target).unwrap().package_id,
            "conduit-host-raspberry-pi@1"
        );
        assert_eq!(zero_2.architecture, "aarch64");
        assert_eq!(zero_2.os.as_deref(), Some("raspberry-pi-os-bookworm-64"));
        assert_eq!(zero_2.default_output, SporeOutputKind::NativeBundle);
        assert_eq!(zero_2.post_build_actions, [PostBuildAction::Launch]);
    }
}

#[test]
fn orange_pi_5_is_exact_aarch64_conduitos_machinery() {
    let packages = conduit_workspace_fabrication::package_set();
    let target = "conduitos/aarch64/orange-pi-5-rk3588s";
    let descriptor = packages.target_descriptor(target).unwrap();
    assert_eq!(
        packages.anchor_for_target(target).unwrap().package_id,
        "conduit-host-orange-pi@1"
    );
    assert_eq!(descriptor.family, "conduitos");
    assert_eq!(descriptor.architecture, "aarch64");
    assert_eq!(descriptor.machine, "orange-pi-5-rk3588s");
    assert_eq!(descriptor.board.as_deref(), Some("orange-pi-5"));
    assert_eq!(descriptor.os, None);
    assert_eq!(descriptor.host_core, "host-core/conduitos@1");
    assert_eq!(descriptor.default_output, SporeOutputKind::SdImage);
    assert_eq!(
        descriptor.post_build_actions,
        [PostBuildAction::Flash, PostBuildAction::Boot]
    );
    assert!(packages
        .target_descriptor("conduitos/loongarch64/orange-pi-5-rk3588s")
        .is_none());
    assert!(packages
        .target_descriptor("std/aarch64/orange-pi-5-rk3588s")
        .is_none());
}

#[test]
fn exact_pro_micro_is_intel_hex_without_an_implemented_browser_flasher() {
    let packages = conduit_workspace_fabrication::package_set();
    let target = "avr/avr5/sparkfun-pro-micro-atmega32u4-5v-16mhz";
    let descriptor = packages.target_descriptor(target).unwrap();
    assert_eq!(
        packages.anchor_for_target(target).unwrap().package_id,
        "conduit-host-avr-promicro@1"
    );
    assert_eq!(descriptor.default_output, SporeOutputKind::IntelHex);
    assert_eq!(descriptor.deployment_adapter, None);
    assert_eq!(
        descriptor.post_build_actions,
        [PostBuildAction::Flash, PostBuildAction::Boot]
    );
}

#[test]
fn ordinary_package_set_excludes_the_proof_only_rp2040_audio_fixture() {
    let packages = conduit_workspace_fabrication::package_set();
    assert!(packages
        .offers_for_target("conduitos/thumbv6m/pico-w")
        .iter()
        .all(|offer| offer.offer.implementation_id != "example/rp2040-pio-audio@1"));
}

#[test]
fn ordinary_package_set_manifest_cannot_depend_on_fixture_paths() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("proof/fixtures/"),
        "ordinary workspace fabrication must not depend on proof fixtures"
    );
}

#[test]
fn target_family_fabrication_owners_are_not_firmware_children() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for owner in [
        repository.join("targets/avr/fabrication/Cargo.toml"),
        repository.join("targets/esp32/fabrication/Cargo.toml"),
        repository.join("targets/rp2040/fabrication/Cargo.toml"),
    ] {
        assert!(
            owner.is_file(),
            "missing target-family owner {}",
            owner.display()
        );
    }

    for historical_parent in [
        repository.join("targets/esp32/firmware/wroom-signal/fabrication-package"),
        repository.join("targets/rp2040/firmware/pico-w-signal/fabrication-package"),
    ] {
        assert!(
            !historical_parent.exists(),
            "target-family fabrication returned beneath firmware: {}",
            historical_parent.display()
        );
    }
}
