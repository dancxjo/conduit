//! Private computation receives fuel only from exact sealed resource bindings.
use conduit_core::*;
use conduit_plan_lowering::lowering::{lower_plan_fragment_from_plan, LoweringError};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn binding(class: &str, units: u32) -> ResourceBinding {
    ResourceBinding {
        pool_id: "host/private-work".into(),
        class_id: class.into(),
        units,
        protected: None,
        compute: None,
        content: None,
    }
}
fn plan(resources: Vec<ResourceBinding>) -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.placements[0].resources = resources;
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    common::seal(fragment)
}
fn lower(
    plan: &Plan,
) -> Result<conduit_plan_lowering::lowering::LoweredPlanFragment, LoweringError> {
    lower_plan_fragment_from_plan(plan, &plan.fragments[0].fragment_id).map(|x| x.0)
}
#[test]
fn absent_or_unrelated_reservations_never_enlarge_the_io_grant() {
    for resources in [vec![], vec![binding("fixture/memory", 65535)]] {
        let lowered = lower(&plan(resources)).unwrap();
        assert_eq!(lowered.nodes[0].maximum_step_fuel, 3);
        assert_eq!(lowered.node_specs[0].maximum_step_fuel, 3);
    }
}
#[test]
fn exact_work_bindings_fund_private_steps_and_remain_kernel_resources() {
    let mut second = binding(COOPERATIVE_STEP_WORK_CLASS_ID, 4);
    second.pool_id = "host/second-work-pool".into();
    let plan = plan(vec![binding(COOPERATIVE_STEP_WORK_CLASS_ID, 3), second]);
    let lowered = lower(&plan).unwrap();
    assert_eq!(lowered.identity.plan_id, plan.plan_id);
    assert_eq!(lowered.nodes[0].maximum_step_fuel, 10);
    assert_eq!(lowered.node_specs[0].maximum_step_fuel, 10);
    assert_eq!(lowered.resources.len(), 2);
    assert_eq!(lowered.resources[0].binding.units, 3);
    assert_eq!(lowered.resources[1].binding.units, 4);
}
#[test]
fn unrepresentable_or_malformed_work_grants_refuse_before_kernel_construction() {
    let overflow = plan(vec![binding(COOPERATIVE_STEP_WORK_CLASS_ID, u32::MAX)]);
    assert_eq!(lower(&overflow), Err(LoweringError::CapacityOverflow));
    assert!(lower(&plan(vec![binding(COOPERATIVE_STEP_WORK_CLASS_ID, 0)])).is_err());
    let mut malformed = binding(COOPERATIVE_STEP_WORK_CLASS_ID, 1);
    malformed.compute = Some(ComputeReservation {
        selected_lanes: 1,
        service_guarantee: ComputeServiceGuarantee::Shared,
        architecture_base_id: "host/base".into(),
        architecture_base_kind: ArchitectureBaseKind::BareMetal,
        topology_group_id: None,
        performance_class: None,
        nominal_clock_hz: None,
    });
    assert!(lower(&plan(vec![malformed])).is_err());
}
#[test]
fn changing_selected_work_changes_plan_identity_and_unsealed_edits_refuse() {
    let first = plan(vec![binding(COOPERATIVE_STEP_WORK_CLASS_ID, 3)]);
    let second = plan(vec![binding(COOPERATIVE_STEP_WORK_CLASS_ID, 4)]);
    assert_ne!(first.plan_id, second.plan_id);
    let mut forged = first;
    forged.fragments[0].placements[0].resources[0].units = 4;
    assert_eq!(lower(&forged), Err(LoweringError::InvalidFragment));
}
