use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

const SOURCE: &str = include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit");
fn image(source: &str) -> HostImage {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/selected-timer".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}
fn host(image: &HostImage, boot: &str) -> Result<StdHost, String> {
    StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id: "fixture/pi3".into(),
            boot_id: boot.into(),
            offer_generation: OfferGeneration(1),
        },
        image,
        &conduit_workspace_make::catalog(),
    )
}
fn plan(host: &StdHost) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "plot clock-demo {\nclock: time/tick(count = 1, period-ms = 0)\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let expanded = expand_canonical_plot(&checked, "clock-demo", &profile).unwrap();
    host.plan_expanded_local(&expanded).unwrap()
}
#[test]
fn selected_timer_preserves_actual_registry_and_only_authored_capacity() {
    let image = image(SOURCE);
    let host = host(&image, "boot/a").unwrap();
    assert_eq!(host.advertisement.resources.len(), 1);
    assert_eq!(host.advertisement.resources[0].capacity_units, 8);
    assert_eq!(host.advertisement.capabilities.len(), 1);
    assert_eq!(host.base_registry.entries().len(), 1);
    assert_eq!(
        host.base_registry.entries()[0].provider_instance_id,
        host.advertisement.bases[0].provider_instance_id
    );
    assert_eq!(
        host.image_identity().unwrap().image_id,
        image.manifest.image_id
    );
    assert!(!host.advertisement.bases.iter().any(|base| base
        .mechanism_family
        .as_str()
        .contains("file")
        || base.mechanism_family.as_str().contains("serial")));
    assert!(host.kernel_resources.is_idle());
}
#[test]
fn selected_single_timer_reservation_refuses_excess_and_old_boot() {
    let image = image(&SOURCE.replace("slots: 8", "slots: 1"));
    let mut first = host(&image, "boot/a").unwrap();
    let plan = plan(&first);
    let reservation = first
        .kernel_resources
        .prepare_and_reserve_plans(&first.advertisement, &[(&plan, false)])
        .unwrap()
        .pop()
        .unwrap();
    assert!(first
        .kernel_resources
        .prepare_and_reserve_plans(&first.advertisement, &[(&plan, false)])
        .is_err());
    first.kernel_resources.release(reservation).unwrap();
    let mut fresh = host(&image, "boot/b").unwrap();
    assert!(fresh
        .kernel_resources
        .prepare_and_reserve_plans(&fresh.advertisement, &[(&plan, false)])
        .is_err());
    assert!(fresh.kernel_resources.is_idle());
}
#[test]
fn duplicate_pools_aggregate_capacity_and_unsupported_operations_refuse() {
    // Each pool individually fits 128; the two together exceed the selected ceiling.
    let source = SOURCE.replace("slots: 8", "slots: 100").replace("  limits =", "  need = {id: \"timer/second\", class: \"conduit.resource/timer-slot@1\", slots: 100, bytes: 512}\n  limits =");
    assert!(host(&image(&source), "boot/a")
        .err()
        .unwrap()
        .contains("aggregate"));
    let mut duplicate = image(SOURCE);
    duplicate
        .manifest
        .resource_budgets
        .push(duplicate.manifest.resource_budgets[0].clone());
    duplicate.payload.resource_budgets = duplicate.manifest.resource_budgets.clone();
    assert!(host(&duplicate, "boot/a").is_err());
    let mut unsupported = image(SOURCE);
    unsupported
        .manifest
        .implementations
        .push("std/kernel-file-copy@1".into());
    unsupported.payload.implementations = unsupported.manifest.implementations.clone();
    assert!(host(&unsupported, "boot/a")
        .err()
        .unwrap()
        .contains("workset"));
    let mut mismatch = image(SOURCE);
    mismatch.manifest.bounds.timer_slots += 1;
    assert!(host(&mismatch, "boot/a")
        .err()
        .unwrap()
        .contains("manifest/payload"));
}
