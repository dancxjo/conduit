use conduit_core::*;
use conduit_plan_lowering::activation_fragment::{
    lower_fragment_activations, verify_lowered_fragment_activations, ActivationLoweringError,
};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn selected_plan() -> Plan {
    let mut fragment = common::fragment();
    fragment.fore_ports = vec![
        PlannedForePort {
            front_port_id: port_id("in"),
            direction: PortDirection::Input,
            placement_id: PlacementId::from("placement"),
            gear_port_id: port_id("next"),
            value_kind: kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
        PlannedForePort {
            front_port_id: port_id("out"),
            direction: PortDirection::Output,
            placement_id: PlacementId::from("placement"),
            gear_port_id: port_id("current"),
            value_kind: kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
    ];
    common::seal(fragment)
}

fn plan() -> Plan {
    let child = selected_plan();
    let outer = common::fragment();
    let activation = PlannedActivation {
        activation_id: "each/item".into(),
        owner_placement_id: PlacementId::from("placement"),
        selected_plan_id: child.plan_id.clone(),
        selected_plan: Box::new(child),
        input: PlannedActivationFront {
            front_port_id: port_id("in"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        output: PlannedActivationFront {
            front_port_id: port_id("out"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        limits: PlannedActivationLimits {
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 1,
            maximum_items: 2,
        },
        terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
    };
    seal_plan_with_activations(
        FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![activation],
        vec![outer],
    )
}

#[test]
fn selects_only_the_exact_fragment_owner_and_retains_recursive_truth() {
    let plan = plan();
    let lowered = lower_fragment_activations(&plan, &plan.fragments[0].fragment_id).unwrap();
    assert_eq!(lowered.entries, plan.activations);
    let PlannedActivationEntry::Unary(entry) = &lowered.entries[0] else {
        panic!()
    };
    assert_eq!(entry.selected_plan_id, entry.selected_plan.plan_id);
    assert_eq!(entry.input.front_port_id.as_str(), "in");
    assert_eq!(entry.limits.maximum_items, 2);
    assert!(verify_lowered_fragment_activations(&lowered, &plan));
    assert!(matches!(
        lower_fragment_activations(&plan, &FragmentId::from("foreign")),
        Err(ActivationLoweringError::UnknownFragment(_))
    ));
}

#[test]
fn refuses_stale_plan_and_lowered_mutation() {
    let plan = plan();
    let mut lowered = lower_fragment_activations(&plan, &plan.fragments[0].fragment_id).unwrap();
    let PlannedActivationEntry::Unary(entry) = &mut lowered.entries[0] else {
        panic!()
    };
    entry.limits.maximum_items += 1;
    assert!(!verify_lowered_fragment_activations(&lowered, &plan));
    let mut stale = plan.clone();
    stale.fragments[0].fragment_id = FragmentId::from("stale");
    assert_eq!(
        lower_fragment_activations(&stale, &FragmentId::from("stale")),
        Err(ActivationLoweringError::InvalidPlan)
    );
}

#[test]
fn activation_free_plan_projects_an_exact_empty_set() {
    let plan = common::seal(common::fragment());
    let lowered = lower_fragment_activations(&plan, &plan.fragments[0].fragment_id).unwrap();
    assert!(lowered.entries.is_empty());
    assert!(verify_lowered_fragment_activations(&lowered, &plan));
}

#[test]
fn duplicate_fragment_identity_is_refused_as_invalid_plan() {
    let mut plan = plan();
    plan.fragments.push(plan.fragments[0].clone());
    let fragment = plan.fragments[0].fragment_id.clone();
    assert_eq!(
        lower_fragment_activations(&plan, &fragment),
        Err(ActivationLoweringError::InvalidPlan)
    );
}

#[test]
fn fingerprint_is_the_exact_recursive_plan_and_fragment_commitment() {
    let plan = plan();
    let lowered = lower_fragment_activations(&plan, &plan.fragments[0].fragment_id).unwrap();
    let mut changed = plan.clone();
    let PlannedActivationEntry::Unary(entry) = &mut changed.activations[0] else {
        panic!()
    };
    entry.input.abnormal_kind = Some(kind_id("failure/changed"));
    assert!(!verify_lowered_fragment_activations(&lowered, &changed));
}
