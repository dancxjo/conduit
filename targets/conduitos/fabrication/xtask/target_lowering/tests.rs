use conduit_host_fabrication::{build_default_host_image, BuildInputs, HostProfile};

use super::*;

fn manifest(source: &str) -> BuildManifest {
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
    .unwrap()
    .0
    .manifest
}

#[test]
fn resolved_profiles_lower_to_distinct_exact_product_inputs() {
    let native = lower(&manifest(include_str!(
        "../../../profiles/conduitos-native.profile.json"
    )))
    .unwrap();
    let headless = lower(&manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    )))
    .unwrap();
    assert_eq!(native.cargo_features, ["native-compositor"]);
    assert!(headless.cargo_features.is_empty());
    assert_ne!(native.implementations, headless.implementations);
    assert_eq!(headless.presenters, 0);
    assert_eq!(headless.presentation_surface_slots, 0);
    assert_eq!(headless.presentation_surface_bytes, 0);
    assert_eq!(native.presentation_surface_slots, 2);
    assert_eq!(native.presentation_surface_bytes, 4_194_304);
    assert_eq!(native.proof_instrumentation, 0);
}

#[test]
fn riscv64_product_profile_lowers_exactly_and_rejects_foreign_bindings() {
    let mut checked = manifest(include_str!(
        "../../../profiles/conduitos-riscv64-headless.profile.json"
    ));
    let lowered = lower(&checked).unwrap();
    assert_eq!(lowered.cargo_features, ["riscv64-product"]);
    assert_eq!(
        lowered.drivers,
        conduitos::fabrication::DRIVER_RISCV64_SBI_CONSOLE
    );
    assert_eq!(lowered.proof_instrumentation, 0);

    checked.presenters[0] = IA32_LINEAR_PRESENTER.into();
    let error = lower(&checked).unwrap_err().to_string();
    assert!(error.contains("riscv64-product-closure-mismatch"));

    checked.presenters[0] = RISCV64_LINEAR_PRESENTER.into();
    checked.driver_selections[0].kind = PL011_DRIVER.into();
    let error = lower(&checked).unwrap_err().to_string();
    assert!(error.contains("riscv64-product-closure-mismatch"));
}

#[test]
fn ia32_product_profile_selects_only_its_linear_runtime_closure() {
    let ia32 = lower(&manifest(include_str!(
        "../../../profiles/conduitos-ia32-headless.profile.json"
    )))
    .unwrap();
    assert_eq!(ia32.cargo_features, ["ia32-product"]);
    assert_eq!(ia32.facilities, 0);
    assert_eq!(ia32.resources, 0);
    assert_eq!(ia32.bases, conduitos::fabrication::BASE_SERIAL_TEXT);
    assert_eq!(
        ia32.drivers,
        conduitos::fabrication::DRIVER_IA32_DEBUGCON_SERIAL
    );
    assert_eq!(
        ia32.presenters,
        conduitos::fabrication::PRESENTER_LINEAR_SERIAL
    );
    assert_eq!(ia32.proof_instrumentation, 0);
}

#[test]
fn ia32_product_lowering_rejects_foreign_presenter_and_driver_bindings() {
    let ia32 = manifest(include_str!(
        "../../../profiles/conduitos-ia32-headless.profile.json"
    ));

    let mut foreign_presenter = ia32.clone();
    foreign_presenter.presenters = vec![LINEAR_SERIAL_PRESENTER.into()];
    let presenter_error = lower(&foreign_presenter).unwrap_err().to_string();
    assert!(presenter_error.contains("ia32-product-closure-mismatch"));
    assert!(presenter_error.contains(IA32_LINEAR_PRESENTER));

    let mut foreign_driver = ia32;
    foreign_driver.driver_selections[0].kind = PL011_DRIVER.into();
    let driver_error = lower(&foreign_driver).unwrap_err().to_string();
    assert!(driver_error.contains("ia32-product-closure-mismatch"));
    assert!(driver_error.contains(IA32_DEBUGCON_DRIVER));
}

#[test]
fn http_profile_selects_exact_native_closure_and_headless_omits_it() {
    let http = manifest(include_str!(
        "../../../profiles/conduitos-http-client.profile.json"
    ));
    let lowered = lower(&http).unwrap();
    assert_eq!(lowered.cargo_features, ["native-http-client"]);
    assert_ne!(
        lowered.implementations & conduitos::fabrication::IMPL_HTTP_CLIENT,
        0
    );
    assert_eq!(
        lowered.facilities,
        conduitos::fabrication::FACILITY_HTTP_CLIENT
    );
    assert_eq!(
        lowered.resources,
        conduitos::fabrication::RESOURCE_HTTP_CLIENT
    );
    assert_eq!(lowered.bases, conduitos::fabrication::BASE_HTTP_NETWORK);
    assert_eq!(lowered.drivers, conduitos::fabrication::DRIVER_HTTP_NETWORK);
    assert_eq!(http.resource_budgets.len(), 4);
    assert_eq!(http.bounds.heap_arena_bytes, 0);

    let headless = lower(&manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    )))
    .unwrap();
    assert_eq!(
        headless.implementations & conduitos::fabrication::IMPL_HTTP_CLIENT,
        0
    );
    assert_eq!(
        headless.facilities & conduitos::fabrication::FACILITY_HTTP_CLIENT,
        0
    );
    assert!(!headless.cargo_features.contains(&"native-http-client"));
}

#[test]
fn http_lowering_rejects_each_missing_prerequisite_and_every_leak() {
    let http = manifest(include_str!(
        "../../../profiles/conduitos-http-client.profile.json"
    ));
    for missing in 0..8 {
        let mut incomplete = http.clone();
        match missing {
            0 => incomplete.host_operations.clear(),
            1..=4 => {
                incomplete.resource_budgets.remove(missing - 1);
            }
            5 => incomplete.facilities.clear(),
            6 => incomplete.base_selections.clear(),
            7 => incomplete.driver_selections.clear(),
            _ => unreachable!(),
        }
        assert!(lower(&incomplete)
            .unwrap_err()
            .to_string()
            .contains("http-profile-prerequisite-missing"));
    }

    let headless = manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    ));
    let mut leaked = headless.clone();
    leaked.host_operations.push(http.host_operations[0].clone());
    assert!(lower(&leaked)
        .unwrap_err()
        .to_string()
        .contains("http-machinery-leaked"));
}

#[test]
fn lowering_rejects_missing_and_leaked_graphical_closure() {
    let native = manifest(include_str!(
        "../../../profiles/conduitos-native.profile.json"
    ));
    for remove in 0..5 {
        let mut incomplete = native.clone();
        match remove {
            0 => incomplete.host_operations.clear(),
            1 => incomplete.facilities.clear(),
            2 => incomplete.resource_budgets.clear(),
            3 => incomplete.base_selections.clear(),
            4 => incomplete.driver_selections.clear(),
            _ => unreachable!(),
        }
        assert!(lower(&incomplete)
            .unwrap_err()
            .to_string()
            .contains("profile-lowering-prerequisite-missing"));
    }

    let headless = manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    ));
    for leak in 0..5 {
        let mut leaked = headless.clone();
        match leak {
            0 => leaked.host_operations.push(PRESENT_OPERATION.into()),
            1 => leaked.facilities.push(NATIVE_COMPOSITOR.into()),
            2 => leaked
                .resource_budgets
                .push(native.resource_budgets[0].clone()),
            3 => leaked
                .base_selections
                .push(native.base_selections[0].clone()),
            4 => leaked
                .driver_selections
                .push(native.driver_selections[0].clone()),
            _ => unreachable!(),
        }
        assert!(lower(&leaked)
            .unwrap_err()
            .to_string()
            .contains("headless-graphical-machinery-leaked"));
    }
}

#[test]
fn unrelated_bounds_do_not_select_graphics_and_wrong_targets_fail_before_cargo() {
    let mut headless = manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    ));
    let original = lower(&headless).unwrap();
    headless.bounds.queue_items += 1;
    assert_eq!(lower(&headless).unwrap(), original);

    headless.target = "conduitos/aarch64/virt".into();
    assert!(lower(&headless)
        .unwrap_err()
        .to_string()
        .contains("aarch64-product-closure-mismatch"));
}

#[test]
fn duplicate_graphical_resource_ceiling_fails_before_codegen() {
    let mut native = manifest(include_str!(
        "../../../profiles/conduitos-native.profile.json"
    ));
    native
        .resource_budgets
        .push(native.resource_budgets[0].clone());
    assert!(lower(&native)
        .unwrap_err()
        .to_string()
        .contains("profile-lowering-resource-ambiguous"));
}

#[test]
fn proof_profiles_are_checked_distinct_and_normal_products_stay_clean() {
    let normal = lower(&manifest(include_str!(
        "../../../profiles/conduitos-native.profile.json"
    )))
    .unwrap();
    let proof = lower(&manifest(include_str!(
        "../../../proof/profiles/conduitos-proof.profile.json"
    )))
    .unwrap();
    let hotplug = lower(&manifest(include_str!(
        "../../../proof/profiles/conduitos-hotplug-proof.profile.json"
    )))
    .unwrap();
    assert_eq!(normal.proof_instrumentation, 0);
    assert_eq!(
        proof.cargo_features,
        ["native-compositor", "scripted-keyboard-proof"]
    );
    assert_eq!(
        proof.proof_instrumentation,
        conduitos::fabrication::PROOF_SCRIPTED_KEYBOARD
    );
    assert_eq!(
        hotplug.cargo_features,
        [
            "native-compositor",
            "hotplug-proof",
            "scripted-keyboard-proof"
        ]
    );
    assert_eq!(
        hotplug.proof_instrumentation,
        conduitos::fabrication::ALL_KNOWN_PROOF_INSTRUMENTATION
    );
}

#[test]
fn proof_instrumentation_without_native_closure_refuses() {
    let mut headless = manifest(include_str!(
        "../../../profiles/conduitos-headless.profile.json"
    ));
    headless
        .profile_fragments
        .push(SCRIPTED_KEYBOARD_PROOF.into());
    assert!(lower(&headless)
        .unwrap_err()
        .to_string()
        .contains("proof-profile-prerequisite-missing"));
}

#[test]
fn aarch64_virt_lowers_to_a_distinct_linear_product_inventory() {
    let manifest = manifest(include_str!(
        "../../../profiles/conduitos-aarch64-headless.profile.json"
    ));
    let lowered = lower(&manifest).unwrap();
    assert_eq!(lowered.cargo_features, ["aarch64-product"]);
    assert_eq!(lowered.facilities, 0);
    assert_eq!(lowered.resources, 0);
    assert_ne!(lowered.bases, conduitos::fabrication::BASE_DISPLAY_SCANOUT);
    assert_ne!(
        lowered.drivers,
        conduitos::fabrication::DRIVER_LINEAR_FRAMEBUFFER
    );
    assert_ne!(
        lowered.presenters,
        conduitos::fabrication::PRESENTER_NATIVE_GRAPHICAL
    );
    assert_eq!(lowered.proof_instrumentation, 0);
    assert_eq!(lowered.presentation_surface_slots, 0);
    assert_eq!(lowered.presentation_surface_bytes, 0);
    assert_eq!(
        lowered.implementations
            & (conduitos::fabrication::IMPL_KEYBOARD
                | conduitos::fabrication::IMPL_PC_SPEAKER
                | conduitos::fabrication::IMPL_OPL2),
        0
    );
}

#[test]
fn aarch64_virt_rejects_x86_leaks_and_incomplete_serial_closure() {
    let source = include_str!("../../../profiles/conduitos-aarch64-headless.profile.json");
    let exact = manifest(source);

    let mut x86_presenter = exact.clone();
    x86_presenter.presenters[0] = NATIVE_PRESENTER.into();
    assert!(lower(&x86_presenter)
        .unwrap_err()
        .to_string()
        .contains("aarch64-product-closure-mismatch"));

    let mut wrong_driver = exact.clone();
    wrong_driver.base_selections[0].driver = FRAMEBUFFER_DRIVER.into();
    assert!(lower(&wrong_driver)
        .unwrap_err()
        .to_string()
        .contains("aarch64-serial-driver-mismatch"));

    let mut proof_leak = exact;
    proof_leak
        .profile_fragments
        .push(SCRIPTED_KEYBOARD_PROOF.into());
    assert!(lower(&proof_leak)
        .unwrap_err()
        .to_string()
        .contains("aarch64-product-closure-mismatch"));
}
