use conduit_core::StateId;
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweringError};

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

#[test]
fn current_profiles_refuse_state_instead_of_ignoring_its_sealed_contract() {
    let plan = common::seal(common::fragment());
    assert!(matches!(lower_plan_fragment(&plan.fragments[0]),
        Err(LoweringError::UnsupportedState(state)) if state == StateId::from("retained")));
}

#[test]
fn altered_state_refuses_as_invalid_before_profile_admission() {
    let mut plan = common::seal(common::fragment());
    plan.fragments[0].states[0].initial_value = Some(vec![8]);
    assert!(matches!(
        lower_plan_fragment(&plan.fragments[0]),
        Err(LoweringError::InvalidFragment)
    ));
}

#[test]
fn explicit_state_storage_lowers_every_exact_contract_and_numeric_owner() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let plan = common::seal(common::fragment());
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    let lowered = lower_plan_fragment_for_profile(&plan.fragments[0], profile).unwrap();
    assert_eq!(lowered.states.len(), 1);
    let state = &lowered.states[0];
    assert_eq!(state.contract, plan.fragments[0].states[0]);
    assert_eq!(state.slot, 0);
    assert_eq!(state.node, lowered.nodes[0].node);
    assert_eq!(state.next, lowered.nodes[0].inputs[0].port);
    assert_eq!(state.current, lowered.nodes[0].outputs[0].port);
    assert_eq!(
        lowered.sign_items,
        plan.fragments[0].sign_storage_budget.item_capacity
    );
}

#[test]
fn smaller_selected_state_storage_refuses_before_installation() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let mut fragment = common::fragment();
    fragment.states[0].maximum_value_bytes = 2;
    let plan = common::seal(fragment);
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    assert_eq!(
        lower_plan_fragment_for_profile(&plan.fragments[0], profile),
        Err(LoweringError::StateStorageExceeded)
    );
    assert!(FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(0, 1)
        .is_err());
    assert!(FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 0)
        .is_err());
}

#[test]
fn fresh_state_profile_refuses_retained_state_instead_of_resetting_it() {
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };
    let plan = common::seal(common::retained_fragment());
    assert!(conduit_core::verify_plan(&plan));
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    assert!(matches!(
        lower_plan_fragment_for_profile(&plan.fragments[0], profile),
        Err(LoweringError::UnsupportedState(_))
    ));
}

#[test]
fn plan_sealed_fore_ports_lower_to_exact_bounded_remote_cords() {
    use conduit_core::{
        ConnectionTrack, DeliveryPressurePolicy, PlannedForePort, PortDirection, PortTemporal,
    };
    use conduit_kernel::CordEndpoint;
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
    };

    let mut fragment = common::fragment();
    fragment.fore_ports = vec![
        PlannedForePort {
            front_port_id: conduit_core::port_id("presentation"),
            direction: PortDirection::Input,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("next"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
        PlannedForePort {
            front_port_id: conduit_core::port_id("show"),
            direction: PortDirection::Output,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("current"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
    ];
    let plan = common::seal(fragment);
    assert!(conduit_core::verify_plan(&plan));
    let lowered = lower_plan_fragment_for_profile(
        &plan.fragments[0],
        FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, 1)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(lowered.fore_ports.len(), 2);
    let input = &lowered.fore_ports[0];
    let output = &lowered.fore_ports[1];
    assert_eq!(input.front_port_id, conduit_core::port_id("presentation"));
    assert_eq!(output.front_port_id, conduit_core::port_id("show"));
    assert!(
        matches!(lowered.cords[usize::from(input.cord.0)].spec.source, CordEndpoint::Remote(id) if id == input.endpoint)
    );
    assert!(
        matches!(lowered.cords[usize::from(output.cord.0)].spec.sink, CordEndpoint::Remote(id) if id == output.endpoint)
    );
    assert_eq!(lowered.cord_value_slots, 2);
    assert_eq!(lowered.cord_value_bytes, 2);
}

#[test]
fn abnormal_fore_projection_retains_its_exact_contract_through_lowering() {
    use conduit_core::{
        CheckedValueContract, ConnectionTrack, DeliveryPressurePolicy, FrontValueContract,
        FrontValueLocation, KindSemanticLaw, PlannedForePort, PortDirection, PortTemporal,
    };
    use conduit_plan_lowering::lowering::{
        lower_plan_fragment_for_profile, ForeValueRefusal, FIXED_KERNEL_STORAGE_PROFILE,
    };

    let mut fragment = common::fragment();
    let placement = &mut fragment.placements[0];
    placement.outputs[0].abnormal_kind = Some(conduit_core::kind_id("value/text"));
    placement.limits.max_queue_bytes = 8;
    let contract =
        CheckedValueContract::new(conduit_core::kind_id("value/text"), 3, vec![]).unwrap();
    placement
        .semantic_contract
        .laws
        .push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::OutputAbnormal(conduit_core::port_id("current")),
            contract: contract.clone(),
        }]));
    fragment.fore_ports = vec![PlannedForePort {
        front_port_id: conduit_core::port_id("failure"),
        direction: PortDirection::Output,
        placement_id: conduit_core::PlacementId::from("placement"),
        gear_port_id: conduit_core::port_id("current"),
        value_kind: conduit_core::kind_id("value/text"),
        value_contract: Some(contract),
        abnormal_kind: None,
        track: ConnectionTrack::AbnormalTerminal,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 8,
    }];
    let plan = common::seal(fragment);
    assert!(conduit_core::verify_plan(&plan));

    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    let lowered = lower_plan_fragment_for_profile(&plan.fragments[0], profile).unwrap();
    let projected = &lowered.fore_ports[0];
    assert_eq!(projected.track, ConnectionTrack::AbnormalTerminal);
    assert_eq!(projected.value_kind, conduit_core::kind_id("value/text"));
    assert_eq!(projected.value_contract.as_ref().unwrap().maximum_bytes, 3);
    assert_eq!(projected.validate_value(b"bad"), Ok(()));
    assert_eq!(
        projected.validate_value(b"long"),
        Err(ForeValueRefusal::Constraint(
            conduit_core::ValueConstraintRefusal::Oversize {
                actual: 4,
                maximum: 3,
            }
        ))
    );
    let cord = &lowered.cords[usize::from(projected.cord.0)].spec;
    assert_eq!(cord.maximum_value_bytes, 3);
}
