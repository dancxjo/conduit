use super::*;
use conduit_core::{ConfigurationEntry, OfferGeneration};
fn placement(updates: u64) -> PlannedGear {
    let o = crate::ambient_home_offer::offer(crate::ambient_home_offer::workspace_bytes().unwrap())
        .unwrap();
    conduit_core::planned_gear_from_parts! {
        placement_id:"home/test".into(), gear_id:"home".into(),
        kind_id:o.kind_id, kind_contract_revision:o.kind_contract_revision,
        execution_profile_id:o.implementation.execution_profile_id,
        configuration:vec![
            ConfigurationEntry {key:"label".into(),value:ConfigurationValue::Text("Home".into())},
            ConfigurationEntry { key:"maximum-updates".into(), value:ConfigurationValue::U64(updates) }],
        host_id:"host/home".into(),boot_id:"boot/home".into(),offer_generation:OfferGeneration(1),
        capability_id:o.capability_id,implementation_id:o.implementation.implementation_id,
        artifact_id:o.implementation.artifact_id,base:None,realization_characteristics:vec![],
        limits:o.limits,inputs:o.inputs,outputs:o.outputs,terminal_transductions:vec![],
        semantic_contract:o.semantic_contract,host_calls:o.host_calls,
        resources:vec![],authority:vec![],pool_references:vec![]
    }
}
#[test]
fn factory_budgets_original_borrowed_value_and_two_operation_identities() {
    let p = placement(2);
    let b = budget(&p).unwrap();
    assert_eq!(
        (b.value_items, b.value_bytes, b.maximum_value_bytes),
        (0, 0, 8192)
    );
    assert_eq!(b.host_requests, 2);
    assert_eq!(b.sign_items, 20);
    let mut values = conduit_kernel::HostedValueStore::new(1, 8192, 8192).unwrap();
    assert!(matches!(
        prepare(&p, &mut values).unwrap(),
        InstalledBack::AmbientHome(_)
    ));
    assert_eq!(budget(&placement(3)).unwrap().host_requests, 2);
}
#[test]
fn factory_refuses_wrong_offer_or_unrepresentable_scope_before_installing() {
    for count in [0, u64::MAX, u32::MAX as u64, 8192] {
        assert!(budget(&placement(count)).is_err());
    }
    let mut p = placement(2);
    p.host_calls.swap(0, 1);
    assert!(configuration(&p).is_err());
    let mut p = placement(2);
    p.artifact_id = "sha256/wrong".into();
    assert!(configuration(&p).is_err());
    let mut p = placement(2);
    p.configuration.push(p.configuration[0].clone());
    assert!(configuration(&p).is_err());
}
