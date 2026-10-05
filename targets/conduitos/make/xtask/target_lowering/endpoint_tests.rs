use super::*;
use conduit_host_make::{build_default_host_image, BuildInputs, HostProfile};
#[test]
fn endpoint_appliance_selects_exact_instrumentation_without_changing_the_ordinary_profile() {
    let profile: HostProfile = serde_json::from_str(include_str!(
        "../../../proof/profiles/conduitos-usb-endpoint-read-proof.profile.json"
    ))
    .unwrap();
    let admitted = build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "endpoint-test-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
    .manifest;
    assert_eq!(admitted.bounds.heap_arena_bytes, 16 * 1024 * 1024);
    let lowered = lower(&admitted).unwrap();
    assert_eq!(
        lowered.cargo_features,
        [
            "native-compositor",
            "scripted-keyboard-proof",
            "usb-endpoint-read-proof"
        ]
    );
    assert_eq!(
        lowered.proof_instrumentation,
        conduitos::make::PROOF_SCRIPTED_KEYBOARD | conduitos::make::PROOF_USB_ENDPOINT_READ
    );
    let mut mixed = admitted.clone();
    mixed.profile_fragments.push(USB_CONFIGURATION_PROOF.into());
    assert!(lower(&mixed).is_err());
    let mut missing = admitted;
    missing
        .profile_fragments
        .retain(|item| item != SCRIPTED_KEYBOARD_PROOF);
    assert!(lower(&missing).is_err());
}
