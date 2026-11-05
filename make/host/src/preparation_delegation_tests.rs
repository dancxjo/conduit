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
fn source() -> String {
    SOURCE.replacen(" schema = 1", &format!(" {POLICY} schema = 1"), 1)
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
