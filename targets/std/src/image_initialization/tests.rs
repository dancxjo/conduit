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
fn selected_timer_boot_does_not_grant_unaccounted_play_storage() {
    let image = image(&SOURCE.replace("slots: 8", "slots: 1"));
    let mut first = host(&image, "boot/a").unwrap();
    let plan = plan(&first);
    let error = first
        .kernel_resources
        .prepare_and_reserve_plans(&first.advertisement, &[(&plan, false)])
        .unwrap_err();
    assert!(
        error.contains("complete ordinary managed-memory accounting"),
        "{error}"
    );
    assert!(first.kernel_resources.is_idle());
    let mut fresh = host(&image, "boot/b").unwrap();
    let stale = fresh
        .kernel_resources
        .prepare_and_reserve_plans(&fresh.advertisement, &[(&plan, false)])
        .unwrap_err();
    assert!(stale.contains("stale Host/Boot/offer"), "{stale}");
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

#[test]
fn selected_image_reconfiguration_refuses_without_losing_current_providers() {
    let image = image(SOURCE);
    let mut current = host(&image, "boot/original").unwrap();
    let advertisement = current.advertisement.clone();
    let bases = current.base_registry.entries().to_vec();
    let refused = current
        .set_declared_name(HostDeclaredName::new("other-host").unwrap())
        .unwrap_err();
    assert!(refused.contains("fresh checked construction"));
    assert_eq!(current.advertisement, advertisement);
    assert_eq!(current.base_registry.entries(), bases);
    assert_eq!(
        current.image_identity().unwrap().image_id,
        image.manifest.image_id
    );
    assert!(current.kernel_resources.is_idle());
    let next = host(&image, "boot/replacement").unwrap();
    assert_ne!(next.advertisement.boot_id, advertisement.boot_id);
    assert_eq!(
        next.image_identity().unwrap().image_id,
        image.manifest.image_id
    );
}

#[test]
fn actual_profile_boot_preserves_zero_unused_line_capacity() {
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
    let initialized = host(&image, "boot/no-lines").unwrap();
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
