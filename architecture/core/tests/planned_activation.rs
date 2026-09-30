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
    let PlannedActivationEntry::Unary(activation) = &mut stale.activations[0] else {
        panic!()
    };
    activation.selected_plan.fragments[0]
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
        let PlannedActivationEntry::Unary(activation) = &mut altered.activations[0] else {
            panic!()
        };
        change(activation);
        assert!(!verify_plan(&altered));
    }
}

#[test]
fn activation_owner_is_unique_even_when_ids_and_variants_differ() {
    let child = selected_plan("duplicate-owner");
    let outer = common::fragment();
    let mut first = activation(child.clone());
    first.activation_id = "first".into();
    let mut second = activation(child);
    second.activation_id = "second".into();
    let plan = seal_plan_with_activation_entries(
        FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![
            PlannedActivationEntry::Unary(first),
            PlannedActivationEntry::Unary(second),
        ],
        vec![outer],
    );
    assert!(!verify_plan(&plan));
}

#[test]
fn activation_preparation_binding_refuses_missing_extra_stale_and_nonlocal_truth() {
    let plan = outer_plan(selected_plan("preparation"));
    assert!(verify_plan(&plan));
    assert_eq!(plan.activation_preparations.len(), 1);

    let mut missing = plan.clone();
    missing.activation_preparations.clear();
    assert!(!verify_plan(&missing));

    let mut extra = plan.clone();
    extra
        .activation_preparations
        .push(extra.activation_preparations[0].clone());
    assert!(!verify_plan(&extra));

    let mut stale = plan.clone();
    stale.activation_preparations[0].child_fragments[0].obligation_digest[0] ^= 1;
    assert!(!verify_plan(&stale));

    let mut child = selected_plan("foreign-host");
    child.fragments[0].host_id = HostId::from("foreign");
    child.fragments[0].placements[0].host_id = HostId::from("foreign");
    child = seal_plan_with_completion(
        FormIdentity {
            source_document_id: child.source_document_id,
            checked_form_id: child.checked_form_id,
            expanded_form_id: child.expanded_form_id,
        },
        child.completion_policy,
        child.fragments,
    );
    assert!(!verify_plan(&outer_plan(child)));
}

#[test]
fn fold_activation_seals_two_inputs_initial_storage_and_child_plan() {
    let mut child = selected_plan("fold");
    let fragment = &mut child.fragments[0];
    fragment.fore_ports[0].front_port_id = port_id("accumulator");
    let mut item = fragment.fore_ports[0].clone();
    item.front_port_id = port_id("item");
    fragment.fore_ports.push(item);
    fragment.fore_ports[1].front_port_id = port_id("combined");
    child = seal_plan_with_completion(
        FormIdentity {
            source_document_id: child.source_document_id,
            checked_form_id: child.checked_form_id,
            expanded_form_id: child.expanded_form_id,
        },
        child.completion_policy,
        child.fragments,
    );
    let outer = common::fragment();
    let fold = PlannedFoldActivation {
        activation_id: "flow-fold/combine".into(),
        owner_placement_id: PlacementId::from("placement"),
        selected_plan_id: child.plan_id.clone(),
        selected_plan: Box::new(child),
        accumulator_input: PlannedActivationFront {
            front_port_id: port_id("accumulator"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        item_input: PlannedActivationFront {
            front_port_id: port_id("item"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        output: PlannedActivationFront {
            front_port_id: port_id("combined"),
            value_kind: kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        initial_accumulator: vec![0],
        retained_accumulator_bytes: 1,
        retained_item_bytes: 1,
        limits: PlannedActivationLimits {
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 4,
            maximum_items: 2,
        },
        terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
        abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
        cancellation_policy: PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
        effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
    };
    let plan = seal_plan_with_activation_entries(
        FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![PlannedActivationEntry::Fold(fold)],
        vec![outer],
    );
    assert!(verify_plan(&plan));
    let fold_entry = plan.activations[0].clone();
    let duplicate = seal_plan_with_activation_entries(
        FormIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_form_id: plan.checked_form_id.clone(),
            expanded_form_id: plan.expanded_form_id.clone(),
        },
        plan.completion_policy,
        vec![],
        vec![
            fold_entry,
            PlannedActivationEntry::Unary(activation(selected_plan("cross-variant"))),
        ],
        plan.fragments.clone(),
    );
    assert!(!verify_plan(&duplicate));
    let mut stale = plan.clone();
    let PlannedActivationEntry::Fold(fold) = &mut stale.activations[0] else {
        panic!()
    };
    fold.initial_accumulator.push(1);
    assert!(!verify_plan(&stale));
}
