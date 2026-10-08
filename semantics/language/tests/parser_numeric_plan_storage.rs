extern crate alloc;
#[path = "../src/parser_session_numeric_plan_storage.rs"]
mod storage;
use conduit_core::*;
use storage::{numeric_plan_retained_bytes, NumericPlanStorageRefusal};
fn fragment() -> PlanFragment {
    PlanFragment {
        plan_id: "plan".into(),
        fragment_id: "fragment".into(),
        source_document_id: "source".into(),
        checked_plot_id: "checked".into(),
        expanded_plot_id: "expanded".into(),
        completion_policy: PlanCompletionPolicy::Live,
        realization_backs: vec![],
        host_id: "host".into(),
        boot_id: "boot".into(),
        offer_generation: OfferGeneration(1),
        placements: vec![],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 0,
            byte_capacity: 0,
        },
        plan_fragments: vec![],
    }
}
#[test]
fn original_plan_spare_slots_and_identity_capacities_are_retained() {
    let mut plan = Plan {
        plan_id: "plan".into(),
        source_document_id: "source".into(),
        checked_plot_id: "checked".into(),
        expanded_plot_id: "expanded".into(),
        completion_policy: PlanCompletionPolicy::Live,
        realization_backs: vec![],
        activations: vec![],
        activation_preparations: vec![],
        fragments: vec![fragment()],
    };
    let baseline = numeric_plan_retained_bytes(&plan).unwrap();
    plan.fragments[0].placements = Vec::with_capacity(7);
    plan.fragments[0].connections = Vec::with_capacity(11);
    plan.fragments[0].expected_sign = Vec::with_capacity(13);
    plan.activations = Vec::with_capacity(3);
    let slots = plan.fragments[0].placements.capacity() * core::mem::size_of::<PlannedGear>()
        + plan.fragments[0].connections.capacity() * core::mem::size_of::<PlannedConnection>()
        + plan.fragments[0].expected_sign.capacity() * core::mem::size_of::<ExpectedSign>()
        + plan.activations.capacity() * core::mem::size_of::<PlannedActivationEntry>();
    assert_eq!(
        numeric_plan_retained_bytes(&plan).unwrap(),
        baseline + slots
    );
    let mut identity = String::with_capacity(8192);
    identity.push_str(plan.plan_id.as_str());
    let extra = identity.capacity() - plan.plan_id.0.capacity();
    plan.plan_id.0 = identity;
    assert_eq!(
        numeric_plan_retained_bytes(&plan).unwrap(),
        baseline + slots + extra
    );
    plan.fragments[0].fore_ports.push(PlannedForePort {
        front_port_id: "input".into(),
        direction: PortDirection::Input,
        placement_id: "placement".into(),
        gear_port_id: "input".into(),
        value_kind: kind_id("value/u64"),
        value_contract: Some(CheckedValueContract {
            value_kind: kind_id("value/u64"),
            maximum_bytes: 8,
            constraints: vec![ValueConstraint::UnsignedRange {
                minimum: Some(0),
                maximum: Some(9),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Inclusive,
            }],
        }),
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: Default::default(),
        item_capacity: 1,
        byte_capacity: 8,
        selected_line: None,
    });
    assert_eq!(
        numeric_plan_retained_bytes(&plan),
        Err(NumericPlanStorageRefusal::Unsupported)
    );
}
