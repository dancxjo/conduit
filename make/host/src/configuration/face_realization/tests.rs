use crate::{
    check_host_configuration, parse_host_configuration_conduit, ConfigurationDiagnostic,
    MakeCatalog, MakeContribution, MakePackageSet,
};
const TARGET: &str = "conduitos/aarch64/virt";
const NATIVE: &str = "presenter/native-graphical@1";
const SOURCE: &str = r#"host anchored-display {
  schema = 1
  target = {architecture: "aarch64", machine: "virt"}
  base = {kind: "serial/text", implementation: "conduitos/pl011@1"}
  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1"}
  need = {id: "surface/main", class: "face/surface", slots: 4, bytes: 8388608}
  limits = {static_memory_bytes: 8388608, heap_arena_bytes: 268435456, queue_items: 64, buffered_bytes: 16384, active_instances: 16, operation_slots: 64, timer_slots: 4, line_sessions: 1, evidence_items: 64}
}"#;
fn packages_and_catalog() -> (MakePackageSet, MakeCatalog) {
    let mut contributions = crate::test_packages::test_package_set()
        .contributions()
        .to_vec();
    for contribution in &mut contributions {
        if let MakeContribution::Extension(extension) = contribution {
            if extension.package_id == "linear-framebuffer-fixture@1" {
                extension.compatible_target_patterns.push(TARGET.into());
                extension.offers[0].target_patterns.push(TARGET.into());
            }
        }
    }
    let packages = MakePackageSet::from_contributions(contributions).unwrap();
    let mut catalog = crate::test_packages::test_catalog().with_packages(&packages);
    catalog
        .presenters
        .get_mut(NATIVE)
        .unwrap()
        .targets
        .push(TARGET.into());
    (packages, catalog)
}
#[test]
fn explicitly_selected_surface_retains_anchor_and_adds_one_complete_realization() {
    let (packages, catalog) = packages_and_catalog();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let profile = checked.profile();
    assert_eq!(profile.presenters.len(), 2);
    assert_eq!(
        profile.presenters[0].implementation,
        "presenter/linear-serial@1"
    );
    assert!(!profile.presenters[0].interactive);
    assert_eq!(profile.presenters[1].implementation, NATIVE);
    assert_ne!(profile.presenters[0].id, profile.presenters[1].id);
    assert_eq!(profile.host_calls, ["conduit.host/present@1"]);
    assert_eq!(profile.facilities, ["compositor/native@1"]);
}
#[test]
fn missing_surface_or_scanout_never_admits_the_optional_presenter() {
    let (packages, catalog) = packages_and_catalog();
    for absent in ["face/surface", "display/scanout"] {
        let source = SOURCE
            .lines()
            .filter(|line| !line.contains(absent))
            .collect::<Vec<_>>()
            .join("\n");
        let checked = check_host_configuration(
            parse_host_configuration_conduit(&source).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        assert_eq!(checked.profile().presenters.len(), 1);
        assert_eq!(
            checked.profile().presenters[0].implementation,
            "presenter/linear-serial@1"
        );
        assert!(checked.profile().facilities.is_empty());
    }
}
#[test]
fn competing_optional_realizations_still_refuse_instead_of_picking_one() {
    let (packages, mut catalog) = packages_and_catalog();
    catalog.presenters.insert(
        "presenter/other-graphical@1".into(),
        catalog.presenters[NATIVE].clone(),
    );
    let errors = check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| matches!(error,
        ConfigurationDiagnostic::AmbiguousFaceRealization {implementations}
            if implementations == &[NATIVE,"presenter/other-graphical@1"])));
}
