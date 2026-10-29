//! CPU ownership proof: refusals preserve custody; successful calls use one owner.
use super::*;
use conduit_plan_lowering::{
    local_tables::DirectLocalKernelTables,
    selected_operation::{SelectedDirectOperationPlan, SelectedVerifiedOperationPlan},
};
use conduit_pulse_capture::installation::*;

#[test]
fn verified_capture_preserves_slot_on_refusal_then_transfers_and_dispatches() {
    let state = Rc::new(State::default());
    let scope = fixture::scope();
    let identity = PulseNativeIdentity {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_id: "base/pulse".into(),
        provider_instance_id: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        artifact_id: "artifact/pulse/fixture".into(),
        implementation_id: scope.implementation_id.clone(),
        base_implementation_id: "base/pulse/fixture@1".into(),
    };
    // SAFETY: inert CPU provider follows the exercised finite/cancellation laws.
    let mut slot = Some(
        unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 7,
                    resource_bytes: 4096,
                    limits: limits(),
                },
                Provider(state.clone()),
            )
        }
        .unwrap(),
    );
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = PULSE_CAPABILITY.into();
    gear.execution_profile_id = PULSE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = PULSE_AUTHORITY.into();
    gear.authority[0].capability_id = PULSE_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = PULSE_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = PULSE_ATTACHMENT_CLASS.into();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    );
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let fragment_id = fragment.fragment_id.clone();
    let mut wrong_lowered = lowered.clone();
    wrong_lowered.nodes[0].maximum_step_fuel += 1;
    let mut stale = active.clone();
    stale.play_sequence += 1;
    let issue = || {
        let mut scope = fixture::scope();
        scope.capability_id = PULSE_CAPABILITY.into();
        scope.authority_contract_id = PULSE_AUTHORITY.into();
        fixture::authorization(scope, fragment, &active)
    };
    let metadata =
        verify_plan_metadata_with_configurations(plan.clone(), |_| Ok::<_, ()>(vec![])).unwrap();
    let contract = PulseReadContract::prepare().unwrap();
    for fault in 0..4 {
        let (table, handle, mut claim) = issue();
        if fault == 2 {
            claim.resource_generation_id = ResourceGenerationId("stale".into());
        }
        if fault == 3 {
            claim.base_provider_generation += 1;
        }
        assert!(ReadyPulseBase::bind_verified_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            SelectedVerifiedOperationPlan {
                plan: &metadata,
                fragment_id: &fragment_id,
                lowered: if fault == 0 { &wrong_lowered } else { &lowered },
                active: if fault == 1 { &stale } else { &active },
                placement_id: &placement,
            },
        )
        .is_err());
        assert!(slot.is_some());
        assert_eq!(state.effects.get(), 0);
    }
    let (table, handle, claim) = issue();
    let selected = || SelectedVerifiedOperationPlan {
        plan: &metadata,
        fragment_id: &fragment_id,
        lowered: &lowered,
        active: &active,
        placement_id: &placement,
    };
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .check_verified(&table, &claim, &contract, selected())
            .unwrap(),
        lowered.nodes[0].node
    );
    assert_eq!(state.effects.get(), 0);
    let mut owner = ReadyPulseBase::bind_verified_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(),
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(state.effects.get(), 0);
    let request = request(5);
    assert!(
        ready(owner.invoke(lowered.nodes[0].node, HostCallId(0), RequestId(0), &request)).is_ok()
    );
    assert_eq!(state.effects.get(), 1);
}

#[test]
fn direct_capture_preserves_slot_on_refusal_then_transfers_and_dispatches() {
    let state = Rc::new(State::default());
    let scope = fixture::scope();
    let identity = PulseNativeIdentity {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_id: "base/pulse".into(),
        provider_instance_id: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        artifact_id: "artifact/pulse/fixture".into(),
        implementation_id: scope.implementation_id.clone(),
        base_implementation_id: "base/pulse/fixture@1".into(),
    };
    // SAFETY: inert CPU provider follows the exercised finite/cancellation laws.
    let mut slot = Some(
        unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 7,
                    resource_bytes: 4096,
                    limits: limits(),
                },
                Provider(state.clone()),
            )
        }
        .unwrap(),
    );
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = PULSE_CAPABILITY.into();
    gear.execution_profile_id = PULSE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = PULSE_AUTHORITY.into();
    gear.authority[0].capability_id = PULSE_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = PULSE_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = PULSE_ATTACHMENT_CLASS.into();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    );
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let fragment_id = fragment.fragment_id.clone();
    let mut stale = active.clone();
    stale.play_sequence += 1;
    let issue = || {
        let mut scope = fixture::scope();
        scope.capability_id = PULSE_CAPABILITY.into();
        scope.authority_contract_id = PULSE_AUTHORITY.into();
        fixture::authorization(scope, fragment, &active)
    };
    let metadata =
        verify_plan_metadata_with_configurations(plan.clone(), |_| Ok::<_, ()>(vec![])).unwrap();
    let tables =
        DirectLocalKernelTables::<1, 0, 1, 1, 0, 1>::prepare(&metadata, &fragment_id).unwrap();
    let missing_placement: PlacementId = "foreign/placement".into();
    let contract = PulseReadContract::prepare().unwrap();
    for fault in 0..4 {
        let (table, handle, mut claim) = issue();
        if fault == 2 {
            claim.resource_generation_id = ResourceGenerationId("stale".into());
        }
        if fault == 3 {
            claim.base_provider_generation += 1;
        }
        assert!(ReadyPulseBase::bind_direct_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            SelectedDirectOperationPlan {
                tables: &tables,
                active: if fault == 1 { &stale } else { &active },
                placement_id: if fault == 0 {
                    &missing_placement
                } else {
                    &placement
                },
            },
        )
        .is_err());
        assert!(slot.is_some());
        assert_eq!(state.effects.get(), 0);
    }
    let (table, handle, claim) = issue();
    let selected = || SelectedDirectOperationPlan {
        tables: &tables,
        active: &active,
        placement_id: &placement,
    };
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .check_direct(&table, &claim, &contract, selected())
            .unwrap(),
        lowered.nodes[0].node
    );
    assert_eq!(state.effects.get(), 0);
    let mut owner = ReadyPulseBase::bind_direct_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(),
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(state.effects.get(), 0);
    let request = request(5);
    assert!(
        ready(owner.invoke(lowered.nodes[0].node, HostCallId(0), RequestId(0), &request)).is_ok()
    );
    assert_eq!(state.effects.get(), 1);
}
