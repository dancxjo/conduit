//! Checked inclusion only: no provider initialization, Ready offer or Pi boot claim.
use conduit_host_make::*;
const SOURCE: &str =
    include_str!("../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit");
fn check(source: &str) -> Result<CheckedHostConfiguration, Vec<ConfigurationDiagnostic>> {
    let packages = conduit_workspace_make::package_set();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &conduit_workspace_make::catalog(),
        &packages,
    )
}
#[test]
fn pi3_explicit_tick_enters_image_without_expanding_selected_storage_or_serial() {
    let checked = check(SOURCE).unwrap();
    assert_eq!(checked.resolved_bases().len(), 3);
    assert_eq!(checked.profile().capabilities.len(), 1);
    assert_eq!(
        checked.profile().capabilities[0].implementation,
        "std/kernel-time-tick@2"
    );
    let source = canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        check(&source).unwrap().configuration_id(),
        checked.configuration_id()
    );
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/selected-pi3".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert_eq!(
        image.manifest.implementations,
        vec!["std/kernel-time-tick@2"]
    );
    assert_eq!(image.manifest.resource_budgets.len(), 1);
    assert!(!image
        .manifest
        .implementations
        .iter()
        .any(|id| id.contains("file")));
}
#[test]
fn missing_budget_wrong_base_unknown_duplicate_and_cycle_refuse() {
    for source in [
        SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("need ="))
            .collect::<Vec<_>>()
            .join("\n"),
        SOURCE.replace("kind: \"timer/monotonic\"", "kind: \"clock/monotonic\""),
        SOURCE.replace("std/kernel-time-tick@2", "unknown/implementation"),
        SOURCE.replace("  need =", "  capability = {kind: \"time/tick\"}\n  need ="),
        SOURCE.replace("kind: \"time/tick\"", "kind: \"file/copy\""),
    ] {
        assert!(check(&source).is_err(), "accepted {source}");
    }
    let packages = conduit_workspace_make::package_set();
    let mut catalog = conduit_workspace_make::catalog();
    catalog.dependencies.insert(
        PrerequisiteNode::Base("timer/monotonic".into()),
        vec![PrerequisiteNode::Resource(
            "conduit.resource/timer-slot@1".into(),
        )],
    );
    assert!(check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages
    )
    .is_err());
}
#[test]
fn malformed_capability_fields_refuse_and_order_preserves_identity() {
    for operation in [
        "{kind: \"time/tick\", extra: 1}",
        "{kind: \"time/tick\", kind: \"time/tick\"}",
    ] {
        let source = SOURCE.replace(
            "{kind: \"time/tick\", pinned_implementation: \"std/kernel-time-tick@2\"}",
            operation,
        );
        assert!(parse_host_configuration_conduit(&source).is_err());
    }
    let parsed = parse_host_configuration_conduit(SOURCE).unwrap();
    assert_eq!(parsed.capabilities.len(), 1);
}

#[test]
fn protected_file_declaration_cannot_manufacture_executable_file_workset() {
    let source = SOURCE.replace(
        "{kind: \"time/tick\", pinned_implementation: \"std/kernel-time-tick@2\"}",
        "{kind: \"file/copy\", pinned_implementation: \"std/kernel-file-copy@1\"}",
    );
    let errors = check(&source).unwrap_err();
    assert!(errors.iter().any(|error| matches!(error, ConfigurationDiagnostic::Capability { detail } if detail.contains("explicitly selected Base/driver/resource budgets"))));
}

#[test]
fn ambiguous_operation_requires_pin_and_wrong_target_refuses() {
    let packages = conduit_workspace_make::package_set();
    let mut catalog = conduit_workspace_make::catalog();
    let metadata = catalog.implementations["std/kernel-time-tick@2"].clone();
    catalog
        .implementations
        .insert("fixture/another-tick".into(), metadata);
    let unpinned = SOURCE.replace(", pinned_implementation: \"std/kernel-time-tick@2\"", "");
    let errors = check_host_configuration(
        parse_host_configuration_conduit(&unpinned).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| matches!(error, ConfigurationDiagnostic::Capability { detail } if detail.contains("ambiguous"))));
    assert!(check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages
    )
    .is_ok());
    catalog
        .implementations
        .get_mut("std/kernel-time-tick@2")
        .unwrap()
        .targets = vec!["browser/*/*".into()];
    assert!(check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages
    )
    .is_err());
}
