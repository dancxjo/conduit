use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod common;

fn selected_plan(suffix: &str) -> Plan {
    let mut fragment = common::fragment();
    fragment.source_document_id = SourceDocumentId::from(format!("selected-source-{suffix}"));
    fragment.checked_form_id = CheckedFormId::from(format!("selected-checked-{suffix}"));
    fragment.expanded_form_id = ExpandedFormId::from(format!("selected-expanded-{suffix}"));
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

fn activation(selected_plan: Plan) -> PlannedActivation {
    PlannedActivation {
        activation_id: "flow-each/element".into(),
        owner_placement_id: PlacementId::from("placement"),
        selected_plan_id: selected_plan.plan_id.clone(),
        selected_plan: Box::new(selected_plan),
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
        },
        terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
    }
}

fn outer_plan(selected_plan: Plan) -> Plan {
    let fragment = common::fragment();
    seal_plan_with_activations(
        FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![activation(selected_plan)],
        vec![fragment],
    )
}

#[test]
fn selected_plan_is_recursive_plan_truth_and_changes_outer_identity() {
    let first = outer_plan(selected_plan("first"));
    let second = outer_plan(selected_plan("second"));

    assert!(verify_plan(&first));
    assert!(verify_plan(&second));
    assert_ne!(first.plan_id, second.plan_id);

    let mut stale = first.clone();
    stale.activations[0].selected_plan.fragments[0]
        .sign_storage_budget
        .byte_capacity += 1;
    assert!(!verify_plan(&stale));
}

#[test]
fn activation_refuses_inexact_ownership_fronts_limits_and_sign_accounting() {
    let plan = outer_plan(selected_plan("exact"));
    let changes: [fn(&mut PlannedActivation); 5] = [
        |value| value.owner_placement_id = PlacementId::from("missing"),
        |value| value.input.front_port_id = port_id("missing"),
        |value| value.limits.maximum_active = 2,
        |value| value.limits.maximum_queue_bytes = 0,
        |value| value.per_activation_sign_budget.byte_capacity += 1,
    ];

    for change in changes {
        let mut altered = plan.clone();
        change(&mut altered.activations[0]);
        assert!(!verify_plan(&altered));
    }
}
