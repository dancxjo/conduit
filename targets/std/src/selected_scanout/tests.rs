//! Actual SDL software lifetime diagnostics; no KMSDRM/HDMI readiness proof.
use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration,
};
use conduit_linux_display::Scanout;

fn source() -> String {
    include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit").replace(
        "  capability =",
        "  base = {kind: \"display/scanout\", implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 640, height: 480}}\n  need = {id: \"display/main\", class: \"face/surface\", slots: 1, bytes: 1228800}\n  capability =",
    )
}
fn checked(source: &str) -> CheckedHostConfiguration {
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn image(checked: &CheckedHostConfiguration) -> HostImage {
    build_default_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/selected-scanout".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}
fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "fixture/scanout".into(),
        boot_id: "boot/scanout".into(),
        offer_generation: OfferGeneration(1),
    }
}
fn parameters(checked: &CheckedHostConfiguration) -> CheckedScanoutInitialization {
    CheckedScanoutInitialization::from_configuration(checked).unwrap()
}
#[test]
fn checked_physical_scanout_is_retained_before_offers_without_inventing_renderer() {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("selected scanout CPU diagnostic", 640, 480)
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let token = scanout.initialized_display().unwrap();
    let checked_source = checked(&source());
    let image = image(&checked_source);
    let catalog = conduit_workspace_make::catalog();
    assert!(StdHost::from_selected_timer_and_scanout_image(
        config(),
        &image,
        &catalog,
        &checked_source,
        parameters(&checked_source),
        token.clone()
    )
    .err()
    .unwrap()
    .contains("KMSDRM"));
    assert!(
        token.is_live(),
        "production backend refusal consumes no custody"
    );
    let foreign = checked(&source().replace("width: 640", "width: 641"));
    assert!(StdHost::from_selected_scanout_diagnostic(
        config(),
        &image,
        &catalog,
        &checked_source,
        parameters(&foreign),
        token.clone()
    )
    .err()
    .unwrap()
    .contains("another checked Source"));
    assert!(token.is_live());
    let wrong_geometry = checked(&source().replace("width: 640", "width: 641"));
    let wrong_geometry_image = self::image(&wrong_geometry);
    assert!(StdHost::from_selected_scanout_diagnostic(
        config(),
        &wrong_geometry_image,
        &catalog,
        &wrong_geometry,
        parameters(&wrong_geometry),
        token.clone(),
    )
    .err()
    .unwrap()
    .contains("geometry"));
    assert!(
        token.is_live(),
        "geometry refusal precedes physical custody"
    );
    let too_small = checked(&source().replace("bytes: 1228800", "bytes: 1228799"));
    let too_small_image = self::image(&too_small);
    assert!(StdHost::from_selected_scanout_diagnostic(
        config(),
        &too_small_image,
        &catalog,
        &too_small,
        parameters(&too_small),
        token.clone(),
    )
    .err()
    .unwrap()
    .contains("finite surface"));
    assert!(
        token.is_live(),
        "insufficient pool precedes physical custody"
    );
    let mut undersized = image.clone();
    for budget in &mut undersized.manifest.resource_budgets {
        if budget.class == "face/surface" {
            budget.bytes = 1;
        }
    }
    undersized.payload.resource_budgets = undersized.manifest.resource_budgets.clone();
    assert!(
        StdHost::from_selected_scanout_diagnostic(
            config(),
            &undersized,
            &catalog,
            &checked_source,
            parameters(&checked_source),
            token.clone()
        )
        .is_err(),
        "copied Source ID does not authorize changed capacity"
    );
    assert!(token.is_live());
    let mut host = StdHost::from_selected_scanout_diagnostic(
        config(),
        &image,
        &catalog,
        &checked_source,
        parameters(&checked_source),
        token.clone(),
    )
    .unwrap();
    let ready = host.ready_selected_scanout().unwrap();
    assert_eq!(
        ready.entry().implementation_id.as_str(),
        "display/linear-framebuffer@1"
    );
    assert_eq!(ready.entry().mechanism_family.as_str(), "display/scanout");
    assert!(ready.entry().capabilities.is_empty());
    assert_eq!(ready.entry().resources[0].pool_id.as_str(), "display/main");
    assert_eq!(ready.entry().resources[0].class_id.as_str(), "face/surface");
    assert_eq!(
        host.advertisement().capabilities.len(),
        1,
        "only installed timer is offered"
    );
    assert!(
        host.ready_native_display().is_none(),
        "physical pixels do not mint Show custody"
    );
    assert!(
        token.claim_host().is_err(),
        "the Host holds exclusive initialization"
    );
    assert!(StdHost::from_selected_scanout_diagnostic(
        config(),
        &image,
        &catalog,
        &checked_source,
        parameters(&checked_source),
        token.clone(),
    )
    .err()
    .unwrap()
    .contains("AlreadyClaimed"));
    assert!(
        host.ready_selected_scanout().is_some(),
        "failed competing constructor preserves current Host"
    );
    let original_boot = host.advertisement.boot_id.clone();
    host.advertisement.boot_id = "boot/stale".into();
    assert!(host.ready_selected_scanout().is_none());
    host.advertisement.boot_id = original_boot;
    host.advertisement.offer_generation = OfferGeneration(2);
    assert!(host.ready_selected_scanout().is_none());
    host.advertisement.offer_generation = OfferGeneration(1);
    assert!(host.ready_selected_scanout().is_some());
    let old = host.advertisement.bases.clone();
    host.advertisement
        .bases
        .retain(|base| base.mechanism_family.as_str() != "display/scanout");
    assert!(
        host.ready_selected_scanout().is_none(),
        "descriptor drift revokes the borrow"
    );
    host.advertisement.bases = old;
    scanout.lose();
    assert!(
        host.ready_selected_scanout().is_none(),
        "actual loss is terminal"
    );
    assert!(host
        .kernel_resources
        .require_reference_reconfiguration()
        .is_err());
    assert!(host.kernel_resources.is_idle());
    drop(host);
    assert!(!token.is_live());
}
