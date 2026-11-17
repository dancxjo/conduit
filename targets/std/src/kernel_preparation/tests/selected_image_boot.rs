//! Actual selected Host construction belongs to the owning Std test surface.
use conduit_core::OfferGeneration;

#[test]
fn actual_profile_boot_preserves_zero_unused_line_capacity() {
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    const SOURCE: &str = include_str!("../../../profiles/raspberry-pi-3-tick-workset.host.conduit");
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    // A directly authored PROFILE is legal independently of construction Source.
    // Do not retain the checked Source identity after changing that PROFILE.
    let mut profile = checked.into_profile();
    profile.source_configuration_id = None;
    profile.bounds.line_sessions = 0;
    let image = build_default_host_image(
        profile,
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/zero-line-profile".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let initialized = crate::StdHost::from_selected_timer_image(
        crate::StdHostConfig {
            host_id: "fixture/pi3".into(),
            boot_id: "boot/no-lines".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        initialized
            .kernel_resources
            .image_bounds
            .as_ref()
            .unwrap()
            .line_sessions,
        0
    );
    assert_eq!(initialized.advertisement.capabilities.len(), 1);
    assert!(initialized.kernel_resources.is_idle());
}
