use super::*;
use crate::{check_host_configuration, parse_host_configuration_conduit, MakeCatalog};
const SOURCE: &str = r#"host linux {
 schema = 1
 target = {architecture: "x86_64", machine: "computer", os: "linux"}
 base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}
 limits = {static_memory_bytes: 1048576, heap_arena_bytes: 1048576, queue_items: 64, buffered_bytes: 65536, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}"#;
fn configuration() -> HostConfiguration {
    parse_host_configuration_conduit(SOURCE).unwrap()
}
#[test]
fn raw_source_and_checked_profile_keep_the_original_identity() {
    let raw = configuration();
    let packages = crate::test_packages::test_package_set();
    let checked = check_host_configuration(
        raw.clone(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap();
    let public = crate::host_configuration_identity(&raw).unwrap();
    assert_eq!(public, checked.configuration_id());
    assert_eq!(
        checked.profile().source_configuration_id.as_deref(),
        Some(public.as_str())
    );
    // Independent pre-extraction hash call verifies accepted ID encoding.
    let old = stable_host_configuration_identity_source(&raw).unwrap();
    assert_eq!(
        public,
        format!("sha256:{:x}", Sha256::digest(old.as_bytes()))
    );
}
#[test]
fn outer_declaration_order_is_unchanged_but_initialization_is_pinned() {
    let mut a = configuration();
    let mut second = a.bases[0].clone();
    second.kind = "clock/monotonic".into();
    second.implementation = Some("hosted/monotonic-clock@1".into());
    a.bases.push(second);
    let mut reordered = a.clone();
    reordered.bases.reverse();
    assert_eq!(
        host_configuration_identity(&a).unwrap(),
        host_configuration_identity(&reordered).unwrap()
    );
    // Hashing an unchecked explicit initialization still creates no checked Host.
    let initialized = crate::BaseInitialization {
        values: [("maximum_duration_ms".into(), serde_json::json!(100))]
            .into_iter()
            .collect(),
    };
    a.bases[1].initialization = Some(initialized);
    let before = host_configuration_identity(&a).unwrap();
    a.bases[1]
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("maximum_duration_ms".into(), serde_json::json!(101));
    assert_ne!(before, host_configuration_identity(&a).unwrap());
}
#[test]
fn each_finite_delegation_or_host_bound_changes_identity() {
    let raw = configuration();
    let original = host_configuration_identity(&raw).unwrap();
    let mut selected = raw.clone();
    selected.preparation = Some(conduit_body::HostPreparationDelegation {
        authorizer: conduit_body::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 4096,
        maximum_runtime_state_bytes: 8192,
    });
    let prepared = host_configuration_identity(&selected).unwrap();
    assert_ne!(original, prepared);
    selected
        .preparation
        .as_mut()
        .unwrap()
        .maximum_assignment_bytes += 1;
    assert_ne!(prepared, host_configuration_identity(&selected).unwrap());
    let changed_assignment = host_configuration_identity(&selected).unwrap();
    selected
        .preparation
        .as_mut()
        .unwrap()
        .maximum_runtime_state_bytes += 1;
    let no_execution = host_configuration_identity(&selected).unwrap();
    assert_ne!(changed_assignment, no_execution);
    selected.execution = Some(conduit_body::HostExecutionDelegation {
        authorizer: conduit_body::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    });
    let executed = host_configuration_identity(&selected).unwrap();
    assert_ne!(no_execution, executed);
    selected.execution.as_mut().unwrap().maximum_active_worksets = 2;
    assert_ne!(executed, host_configuration_identity(&selected).unwrap());
    let mut narrower = raw;
    narrower.limits.buffered_bytes -= 1;
    assert_ne!(original, host_configuration_identity(&narrower).unwrap());
}
