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

fn named_fragment(host: &str, placement: &str) -> PlanFragment {
    let mut fragment = common::fragment();
    fragment.host_id = HostId::from(host);
    fragment.boot_id = BootId::from(format!("{host}-boot"));
    fragment.placements[0].host_id = fragment.host_id.clone();
    fragment.placements[0].boot_id = fragment.boot_id.clone();
    fragment.placements[0].placement_id = PlacementId::from(placement);
    fragment.placements[0].gear_id = GearId::from(placement);
    fragment.states[0].gear_id = GearId::from(placement);
    fragment.states[0].state_id = StateId::from(format!("{placement}-state"));
    fragment.startup_order[0] = PlacementId::from(placement);
    fragment
}

fn unary_for(owner: &str, id: &str) -> PlannedActivationEntry {
    let child = selected_plan();
    let mut value = match plan().activations[0].clone() {
        PlannedActivationEntry::Unary(value) => value,
        _ => unreachable!(),
    };
    value.activation_id = id.into();
    value.owner_placement_id = PlacementId::from(owner);
    value.selected_plan_id = child.plan_id.clone();
    value.selected_plan = Box::new(child);
    PlannedActivationEntry::Unary(value)
}

fn progression_for(owner: &str, id: &str, scan: bool) -> PlannedActivationEntry {
    let mut child = selected_plan();
    child.fragments[0].fore_ports[0].front_port_id = port_id("accumulator");
    let mut item = child.fragments[0].fore_ports[0].clone();
    item.front_port_id = port_id("item");
    child.fragments[0].fore_ports.push(item);
    child.fragments[0].fore_ports[1].front_port_id = port_id("combined");
    child = seal_plan_with_completion(
        FormIdentity {
            source_document_id: child.source_document_id,
            checked_form_id: child.checked_form_id,
            expanded_form_id: child.expanded_form_id,
        },
        child.completion_policy,
        child.fragments,
    );
    let front = |name| PlannedActivationFront {
        front_port_id: port_id(name),
        value_kind: kind_id("fixture/byte@1"),
        abnormal_kind: None,
    };
    let limits = PlannedActivationLimits {
        maximum_active: 1,
        maximum_queue_items: 1,
        maximum_queue_bytes: 4,
        maximum_items: 3,
    };
    if scan {
        PlannedActivationEntry::Scan(PlannedScanActivation {
            activation_id: id.into(),
            owner_placement_id: PlacementId::from(owner),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: front("accumulator"),
            item_input: front("item"),
            output: front("combined"),
            initial_accumulator: vec![9],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits,
            terminal_policy: PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission,
            abnormal_policy: PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy: PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: SignStorageBudget {
                item_capacity: 2,
                byte_capacity: 64,
            },
        })
    } else {
        PlannedActivationEntry::Fold(PlannedFoldActivation {
            activation_id: id.into(),
            owner_placement_id: PlacementId::from(owner),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            accumulator_input: front("accumulator"),
            item_input: front("item"),
            output: front("combined"),
            initial_accumulator: vec![7],
            retained_accumulator_bytes: 1,
            retained_item_bytes: 1,
            limits,
            terminal_policy: PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce,
            abnormal_policy: PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact,
            cancellation_policy: PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission,
            effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: SignStorageBudget {
                item_capacity: 2,
                byte_capacity: 64,
            },
        })
    }
}

#[test]
fn verified_two_fragment_projection_filters_in_plan_order_and_keeps_progression_state() {
    let mut left = named_fragment("left", "left-unary");
    let mut second = left.placements[0].clone();
    second.placement_id = PlacementId::from("left-scan");
    second.gear_id = GearId::from("left-scan");
    left.placements.push(second);
    left.startup_order.push(PlacementId::from("left-scan"));
    let right = named_fragment("right", "right-fold");
    let plan = seal_plan_with_activation_entries(
        FormIdentity {
            source_document_id: left.source_document_id.clone(),
            checked_form_id: left.checked_form_id.clone(),
            expanded_form_id: left.expanded_form_id.clone(),
        },
        PlanCompletionPolicy::Live,
        vec![],
        vec![
            unary_for("left-unary", "first"),
            progression_for("right-fold", "second", false),
            progression_for("left-scan", "third", true),
        ],
        vec![left, right],
    );
    assert!(verify_plan(&plan));
    let left = lower_fragment_activations(&plan, &plan.fragments[0].fragment_id).unwrap();
    let right = lower_fragment_activations(&plan, &plan.fragments[1].fragment_id).unwrap();
    assert_eq!(left.entries.len(), 2);
    assert_eq!(right.entries.len(), 1);
    let PlannedActivationEntry::Unary(first) = &left.entries[0] else {
        panic!()
    };
    assert_eq!(first.activation_id, "first");
    let PlannedActivationEntry::Scan(scan) = &left.entries[1] else {
        panic!()
    };
    assert_eq!(scan.activation_id, "third");
    assert_eq!(scan.selected_plan_id, scan.selected_plan.plan_id);
    assert_eq!(scan.initial_accumulator, vec![9]);
    assert_eq!(
        (scan.retained_accumulator_bytes, scan.retained_item_bytes),
        (1, 1)
    );
    assert_eq!(
        scan.terminal_policy,
        PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission
    );
    assert_eq!(
        scan.abnormal_policy,
        PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact
    );
    assert_eq!(
        scan.cancellation_policy,
        PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission
    );
    assert_eq!(
        scan.effect_multiplicity,
        PlannedActivationEffectMultiplicity::OncePerAcceptedInput
    );
    assert_eq!(
        scan.per_activation_sign_budget,
        SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64
        }
    );
    let PlannedActivationEntry::Fold(fold) = &right.entries[0] else {
        panic!()
    };
    assert_eq!(fold.activation_id, "second");
    assert_eq!(fold.selected_plan_id, fold.selected_plan.plan_id);
    assert_eq!(fold.initial_accumulator, vec![7]);
    assert_eq!(
        (fold.retained_accumulator_bytes, fold.retained_item_bytes),
        (1, 1)
    );
    assert_eq!(
        fold.terminal_policy,
        PlannedFoldTerminalPolicy::DrainThenEmitAccumulatorExactlyOnce
    );
    assert_eq!(
        fold.abnormal_policy,
        PlannedFoldAbnormalPolicy::DiscardAccumulatorAndPropagateExact
    );
    assert_eq!(
        fold.cancellation_policy,
        PlannedFoldCancellationPolicy::DiscardAccumulatorWithoutEmission
    );
    assert_eq!(
        fold.effect_multiplicity,
        PlannedActivationEffectMultiplicity::OncePerAcceptedInput
    );
    assert_eq!(
        fold.per_activation_sign_budget,
        SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64
        }
    );
}
