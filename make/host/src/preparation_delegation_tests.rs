//! Checked policy intent must survive canonical Source and IMAGE correspondence.
use crate::test_packages::{test_build_host_image, test_package_set};
use crate::*;
const SOURCE: &str = r#"
host delegated {
 schema = 1
 target = {architecture: "x86_64", machine: "computer", os: "linux"}
 base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}
 limits = {static_memory_bytes: 1048576, heap_arena_bytes: 1048576, queue_items: 64, buffered_bytes: 65536, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}
"#;
const POLICY: &str = "preparation = {authorizer: \"seed-owner\", maximum_assignment_bytes: 8192, maximum_runtime_state_bytes: 8192}\n";
const EXECUTION: &str = "execution = {authorizer: \"seed-owner\", maximum_active_worksets: 1}\n";
fn source() -> String {
    SOURCE.replacen(" schema = 1", &format!(" {POLICY} schema = 1"), 1)
}

#[test]
fn execution_is_independent_checked_identity_bound_and_parser_free() {
    let preparation_only = check(&source()).unwrap();
    assert!(preparation_only.configuration().execution.is_none());
    assert_eq!(rust_execution_delegation(&preparation_only), "None");
    let selected_source = source().replacen(" schema = 1", &format!(" {EXECUTION} schema = 1"), 1);
    let selected = check(&selected_source).unwrap();
    assert_ne!(
        selected.configuration_id(),
        preparation_only.configuration_id()
    );
    let canonical = canonical_host_configuration_conduit(selected.configuration()).unwrap();
    assert!(canonical.contains(EXECUTION.trim()));
    let reopened = check(&canonical).unwrap();
    assert_eq!(selected.configuration_id(), reopened.configuration_id());
    assert_eq!(selected.configuration(), reopened.configuration());
    let packages = test_package_set();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let (image, _) = test_build_host_image(
        selected.profile().clone(),
        &catalog,
        &BuildInputs {
            source_identity: "fixture/execution".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert_eq!(
        image.manifest.source_configuration_id.as_deref(),
        Some(selected.configuration_id())
    );
    assert_eq!(
        image.payload.source_configuration_id,
        image.manifest.source_configuration_id
    );
    assert_ne!(
        image.manifest.source_configuration_id.as_deref(),
        Some(preparation_only.configuration_id())
    );
    for (checked, enabled) in [(&preparation_only, false), (&selected, true)] {
        compiled_rust::verify_with_libraries(
            &format!("const EXECUTION:Option<conduit_body::HostExecutionDelegation>={};", rust_execution_delegation(checked)),
            &format!("fn main(){{assert_eq!(EXECUTION.is_some(),{enabled});if let Some(policy)=EXECUTION{{assert_eq!(policy.authorizer.as_str(),\"seed-owner\");assert_eq!(policy.maximum_active_worksets,1);}}}}"),
            "checked-execution-policy", &["conduit_body"],
        );
    }
}

#[test]
fn execution_without_preparation_or_finite_capacity_refuses() {
    let selected = source().replacen(" schema = 1", &format!(" {EXECUTION} schema = 1"), 1);
    for invalid in [
        selected.replace(POLICY, ""),
        selected.replace("maximum_active_worksets: 1", "maximum_active_worksets: 0"),
        selected.replace("maximum_active_worksets: 1", "maximum_active_worksets: 9"),
        selected.replace(
            "maximum_runtime_state_bytes: 8192",
            "maximum_runtime_state_bytes: 0",
        ),
    ] {
        assert!(check(&invalid).is_err(), "accepted {invalid}");
    }
    for invalid in [
        selected.replace(EXECUTION, &format!("{EXECUTION}{EXECUTION}")),
        selected.replace(
            EXECUTION,
            "execution = {authorizer: \"any-member\", maximum_active_worksets: 1}\n",
        ),
        selected.replace(EXECUTION, "execution = {maximum_active_worksets: 1}\n"),
        selected.replace(
            EXECUTION,
            "execution = {authorizer: \"seed-owner\", maximum_active_worksets: 1, ambient: true}\n",
        ),
    ] {
        assert!(parse_host_configuration_conduit(&invalid).is_err());
    }
}
fn check(source: &str) -> Result<CheckedHostConfiguration, Vec<ConfigurationDiagnostic>> {
    let packages = test_package_set();
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
}

#[test]
fn absent_delegation_is_exact_and_selected_policy_changes_artifact_identity() {
    let absent = check(SOURCE).unwrap();
    assert!(absent.configuration().preparation.is_none());
    let selected = check(&source()).unwrap();
    let policy = selected.configuration().preparation.unwrap();
    assert_eq!(policy.authorizer, HostPreparationAuthorizer::SeedOwner);
    assert_eq!(policy.maximum_assignment_bytes, 8192);
    assert_ne!(selected.configuration_id(), absent.configuration_id());
    let canonical = canonical_host_configuration_conduit(selected.configuration()).unwrap();
    assert!(canonical.contains(POLICY.trim()));
    let reopened = check(&canonical).unwrap();
    assert_eq!(selected.configuration(), reopened.configuration());
    assert_eq!(selected.configuration_id(), reopened.configuration_id());
    let mut changed = selected.configuration().clone();
    changed
        .preparation
        .as_mut()
        .unwrap()
        .maximum_runtime_state_bytes -= 1;
    let packages = test_package_set();
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let changed = check_host_configuration(changed, &catalog, &packages).unwrap();
    assert_ne!(selected.configuration_id(), changed.configuration_id());
    let (image, _) = test_build_host_image(
        selected.profile().clone(),
        &catalog,
        &BuildInputs {
            source_identity: "fixture/delegation".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    verify_configuration_image(&selected, &image, &catalog).unwrap();
    assert!(verify_configuration_image(&absent, &image, &catalog).is_err());
    assert!(verify_configuration_image(&changed, &image, &catalog).is_err());
    assert_eq!(
        image.manifest.source_configuration_id.as_deref(),
        Some(selected.configuration_id())
    );
    assert_eq!(
        image.payload.source_configuration_id,
        image.manifest.source_configuration_id
    );
    for other in [&absent, &changed] {
        assert_ne!(
            Some(other.configuration_id()),
            image.manifest.source_configuration_id.as_deref()
        );
        let (other_image, _) = test_build_host_image(
            other.profile().clone(),
            &catalog,
            &BuildInputs {
                source_identity: "fixture/delegation".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        assert_ne!(other_image.manifest.build_id, image.manifest.build_id);
        assert_eq!(
            other_image.manifest.source_configuration_id.as_deref(),
            Some(other.configuration_id())
        );
    }
}

#[test]
fn duplicate_unknown_missing_and_unbounded_delegations_refuse() {
    for bad in [
        source().replace("authorizer: \"seed-owner\"", "authorizer: \"any-member\""),
        source().replace("authorizer: \"seed-owner\", ", ""),
        source().replace(
            "maximum_assignment_bytes: 8192",
            "maximum_assignment_bytes: -1",
        ),
        source().replace(
            "maximum_assignment_bytes: 8192",
            "maximum_assignment_bytes: 4294967296",
        ),
        source().replace(POLICY, &format!("{POLICY}{POLICY}")),
        source().replace(
            "authorizer: \"seed-owner\"",
            "authorizer: \"seed-owner\", ambient: true",
        ),
    ] {
        assert!(parse_host_configuration_conduit(&bad).is_err());
    }
    for bad in [
        source().replace(
            "maximum_assignment_bytes: 8192",
            "maximum_assignment_bytes: 0",
        ),
        source().replace(
            "maximum_runtime_state_bytes: 8192",
            "maximum_runtime_state_bytes: 0",
        ),
        source().replace(
            "maximum_assignment_bytes: 8192",
            "maximum_assignment_bytes: 65537",
        ),
        source().replace(
            "maximum_runtime_state_bytes: 8192",
            "maximum_runtime_state_bytes: 1048577",
        ),
    ] {
        assert!(check(&bad).is_err());
    }
}

#[path = "../../../architecture/plot/tests/common/compiled_rust.rs"]
mod compiled_rust;
#[test]
fn checked_policy_emission_compiles_without_a_runtime_parser_or_default_permission() {
    for (source, selected) in [(SOURCE.to_owned(), false), (source(), true)] {
        let checked = check(&source).unwrap();
        let generated = format!(
            "pub const POLICY:Option<conduit_body::HostPreparationDelegation>={};",
            rust_preparation_delegation(&checked)
        );
        let exercise = format!("fn main(){{assert_eq!(POLICY.is_some(),{selected});if let Some(policy)=POLICY{{assert_eq!(policy.authorizer.as_str(),\"seed-owner\");assert_eq!(policy.maximum_assignment_bytes,8192);assert_eq!(policy.maximum_runtime_state_bytes,8192);}}}}");
        compiled_rust::verify_with_libraries(
            &generated,
            &exercise,
            "checked-preparation-policy",
            &["conduit_body"],
        );
    }
}

#[test]
fn continuation_is_explicit_finite_and_survives_source_and_rust_fabrication() {
    let selected = source().replacen(" schema = 1", &format!(" {EXECUTION} schema = 1"), 1);
    assert!(check(&selected)
        .unwrap()
        .configuration()
        .execution
        .unwrap()
        .continuation
        .is_none());
    let continued = selected.replace("maximum_active_worksets: 1}", "maximum_active_worksets: 1, continuation: {assignment_bytes: 8192, runtime_bytes: 16384, durable_bytes: 65536, operation_slots: 8, active_worksets: 1}}");
    let checked = check(&continued).unwrap();
    let canonical = canonical_host_configuration_conduit(checked.configuration()).unwrap();
    let reopened = check(&canonical).unwrap();
    assert_eq!(checked.configuration_id(), reopened.configuration_id());
    assert!(rust_execution_delegation(&checked)
        .contains("continuation:Some(conduit_body::ContinuationCeilings"));
    compiled_rust::verify_with_libraries(
        &format!("const EXECUTION:Option<conduit_body::HostExecutionDelegation>={};", rust_execution_delegation(&checked)),
        "fn main(){let c=EXECUTION.unwrap().continuation.unwrap();assert_eq!(c.durable_bytes,65536);assert!(c.has_finite_bounds());}",
        "checked-continuation-policy", &["conduit_body"],
    );
    for invalid in [
        continued.replace(
            "continuation: {assignment_bytes: 8192",
            "continuation: {assignment_bytes: 8193",
        ),
        continued.replace("runtime_bytes: 16384", "runtime_bytes: 1048577"),
        continued.replace("durable_bytes: 65536", "durable_bytes: 0"),
        continued.replace(
            "operation_slots: 8, active_worksets",
            "operation_slots: 9, active_worksets",
        ),
        continued.replace("active_worksets: 1}}", "active_worksets: 2}}"),
    ] {
        assert!(check(&invalid).is_err(), "accepted {invalid}");
    }
}
