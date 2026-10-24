use super::super::tests::manifest;
use super::*;

fn graphical() -> BuildManifest {
    manifest(include_str!(
        "../../../profiles/conduitos-loongarch64-graphical.host.conduit"
    ))
}

#[test]
fn real_graphical_profile_admits_scanout_without_local_input_or_proof_inventory() {
    let lowered = lower_loongarch64_virt(&graphical()).unwrap();
    assert_eq!(
        lowered.cargo_features,
        ["loongarch64-product", "native-compositor-core"]
    );
    assert_eq!(
        lowered.bases,
        conduitos::make::BASE_DISPLAY_SCANOUT | conduitos::make::BASE_SERIAL_TEXT
    );
    assert_eq!(
        lowered.drivers,
        conduitos::make::DRIVER_LINEAR_FRAMEBUFFER | conduitos::make::DRIVER_LOONGARCH64_UART
    );
    assert_eq!(lowered.face_surface_slots, 4);
    assert_eq!(lowered.face_surface_bytes, 8388608);
    assert_eq!(lowered.proof_instrumentation, 0);
    assert_eq!(
        lowered.implementations
            & (conduitos::make::IMPL_KEYBOARD
                | conduitos::make::IMPL_POINTER
                | conduitos::make::IMPL_PC_SPEAKER
                | conduitos::make::IMPL_OPL2),
        0
    );
}

#[test]
fn graphical_closure_refuses_mismatched_or_unbounded_resources_and_drivers() {
    for case in 0..6 {
        let mut manifest = graphical();
        match case {
            0 => manifest.resource_budgets.clear(),
            1 => manifest.resource_budgets[0].bytes = 0,
            2 => manifest.base_selections[0].driver = "other-framebuffer".into(),
            3 => manifest.facilities.clear(),
            4 => manifest.profile_fragments.push("unselected-proof".into()),
            _ => manifest
                .resource_budgets
                .push(manifest.resource_budgets[0].clone()),
        }
        assert!(lower_loongarch64_virt(&manifest)
            .unwrap_err()
            .to_string()
            .contains("loongarch64-"));
    }
}
