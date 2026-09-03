use super::*;

#[test]
fn existing_computer_targets_are_exact_package_owned_outputs() {
    let body_id = "a".repeat(64);
    for (target_id, architecture, machine, output) in [
        (
            STD_COMPUTER_TARGET_ID,
            "x86_64",
            "computer",
            SporeOutputKind::NativeBundle,
        ),
        (
            HOSTED_WINDOWS_X86_64_TARGET_ID,
            "x86_64",
            "windows-computer",
            SporeOutputKind::NativeBundle,
        ),
        (
            HOSTED_MACOS_AARCH64_TARGET_ID,
            "aarch64",
            "macos-computer",
            SporeOutputKind::NativeBundle,
        ),
        (
            BROWSER_PAGE_TARGET_ID,
            "wasm32",
            "page",
            SporeOutputKind::BrowserBundle,
        ),
    ] {
        let prepared = prepare(&body_id, "invitation/existing", target_id).unwrap();
        let profile = prepared.configuration.profile();
        assert_eq!(profile.target.architecture, architecture);
        assert_eq!(profile.target.machine, machine);
        assert_eq!(prepared.output, output);
    }
}

#[test]
fn conduitos_creche_targets_are_only_earned_product_hosts() {
    let body_id = "a".repeat(64);
    for (target_id, architecture, machine) in [
        (CONDUITOS_X86_64_TARGET_ID, "x86_64", "pc"),
        (CONDUITOS_AARCH64_TARGET_ID, "aarch64", "virt"),
        (CONDUITOS_IA32_TARGET_ID, "ia32", "pc"),
        (CONDUITOS_RISCV64_TARGET_ID, "riscv64", "virt"),
        (CONDUITOS_LOONGARCH64_TARGET_ID, "loongarch64", "virt"),
    ] {
        let prepared = prepare(&body_id, "invitation/conduitos", target_id).unwrap();
        let profile = prepared.configuration.profile();
        assert_eq!(profile.target.architecture, architecture);
        assert_eq!(profile.target.machine, machine);
        assert_eq!(prepared.output, SporeOutputKind::DiskImage);
    }
}

#[test]
fn broad_or_unknown_existing_computer_targets_are_not_inferred() {
    let body_id = "b".repeat(64);
    for target in ["std/*/*", "std/aarch64/server", "browser/wasm32/worker"] {
        let error = match prepare(&body_id, "invitation/existing", target) {
            Ok(_) => panic!("broad target {target:?} was inferred"),
            Err(error) => error,
        };
        assert!(error.contains("unsupported exact"));
    }
}

#[test]
fn exact_pro_micro_retains_intel_hex_and_package_identity() {
    let prepared = prepare(&"c".repeat(64), "invitation/avr", AVR_TARGET_ID).unwrap();
    assert_eq!(prepared.configuration.profile().target.key(), AVR_TARGET_ID);
    assert_eq!(prepared.output, SporeOutputKind::IntelHex);
    assert_eq!(
        prepared
            .packages
            .anchor_for_target(AVR_TARGET_ID)
            .unwrap()
            .package_id,
        AVR_PACKAGE_ID
    );
}

#[test]
fn raspberry_pi_os_and_bare_metal_are_distinct_exact_targets() {
    let body_id = "d".repeat(64);
    for target in [RASPBERRY_PI_OS_TARGET, ZERO_2_W_TARGET, ZERO_2_WH_TARGET] {
        let os = prepare(&body_id, "invitation/pi-os", target).unwrap();
        assert_eq!(os.output, SporeOutputKind::NativeBundle);
        assert_eq!(os.configuration.profile().target.key(), target);
    }
    for target in [B_PLUS_TARGET, ZERO_TARGET, ZERO_W_TARGET, ZERO_WH_TARGET] {
        let bare = prepare(&body_id, "invitation/pi-bare", target).unwrap();
        assert_eq!(bare.output, SporeOutputKind::SdImage);
        assert_eq!(bare.configuration.profile().target.key(), target);
    }
    for unsupported in [
        "std/aarch64/raspberry-pi-5",
        "conduitos/armv7/raspberry-pi-3",
        "conduitos/armv6/raspberry-pi-b-plus",
    ] {
        let error = match prepare(&body_id, "invitation/pi", unsupported) {
            Ok(_) => panic!("unsupported Raspberry Pi target was inferred"),
            Err(error) => error,
        };
        assert!(error.contains("unsupported exact"));
    }
}

#[test]
fn orange_pi_5_is_exact_aarch64_conduitos_machinery_not_loongarch() {
    let body_id = "e".repeat(64);
    let prepared = prepare(&body_id, "invitation/orange-pi-5", ORANGE_PI_5_TARGET).unwrap();
    let profile = prepared.configuration.profile();
    assert_eq!(profile.target.key(), ORANGE_PI_5_TARGET);
    assert_eq!(profile.target.architecture, "aarch64");
    assert_eq!(profile.target.machine, "orange-pi-5-rk3588s");
    assert_eq!(prepared.output, SporeOutputKind::SdImage);
    let descriptor = prepared
        .packages
        .target_descriptor(ORANGE_PI_5_TARGET)
        .unwrap();
    assert_eq!(descriptor.board.as_deref(), Some("orange-pi-5"));
    assert_eq!(descriptor.os, None);
    assert_eq!(descriptor.family, "conduitos");
    assert_eq!(
        prepared
            .packages
            .anchor_for_target(ORANGE_PI_5_TARGET)
            .unwrap()
            .package_id,
        ORANGE_PI_PACKAGE_ID
    );
    for unsupported in [
        "conduitos/loongarch64/orange-pi-5-rk3588s",
        "std/aarch64/orange-pi-5-rk3588s",
        "conduitos/aarch64/orange-pi-5b-rk3588s",
        "conduitos/aarch64/orange-pi-5-plus-rk3588",
    ] {
        assert!(prepare(&body_id, "invitation/orange-pi", unsupported).is_err());
    }
}
