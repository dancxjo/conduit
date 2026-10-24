//! Ordinary selected preflight must retain the same initialized pin owner on refusal.
use super::*;
use conduit_tm1637_wire::installation::*;
fn selected() -> (
    PlanFragment,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = WRITE_CAPABILITY.into();
    gear.execution_profile_id = WRITE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = WRITE_AUTHORITY.into();
    gear.authority[0].capability_id = WRITE_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = WRITE_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = WRITE_ATTACHMENT_CLASS.into();
    reseal(fragment, placement)
}
fn reseal(
    fragment: PlanFragment,
    placement: PlacementId,
) -> (
    PlanFragment,
    conduit_plan_lowering::lowering::LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let fragment = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
    .fragments
    .remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    (fragment, lowered, active, placement)
}
#[test]
fn ordinary_selected_refusal_retains_provider_then_exact_transfer_and_retirement() {
    let state = Rc::new(State::default());
    let mut slot = Some(installation::ready(state.clone()));
    let identity = slot.as_ref().unwrap().native_identity().clone();
    let address = slot.as_ref().unwrap() as *const _;
    let contract = WriteContract::prepare().unwrap();
    // Every independently resealed alternative must still match Root's native facts.
    for mutation in 0..6 {
        let (mut fragment, _, mut active, placement) = selected();
        match mutation {
            2 => fragment.placements[0].artifact_id = "artifact/foreign".into(),
            3 => fragment.placements[0].execution_profile_id = "profile/foreign".into(),
            4 => fragment.placements[0].inputs[0].port_id = port_id("foreign"),
            _ => {}
        }
        if mutation >= 2 {
            let (f, _, a, _) = reseal(fragment, placement.clone());
            fragment = f;
            active = a;
        }
        let mut lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
        let mut scope = fixture::scope();
        scope.capability_id = WRITE_CAPABILITY.into();
        scope.authority_contract_id = WRITE_AUTHORITY.into();
        let (table, handle, mut claim) = fixture::authorization(scope, &fragment, &active);
        match mutation {
            0 => active.boot_id = "boot/foreign".into(),
            1 => {
                claim.resource_generation_id =
                    ResourceGenerationId("foreign/resource-generation".into())
            }
            5 => lowered.host_calls[0].binding.maximum_output_bytes = 1,
            _ => {}
        }
        assert!(
            matches!(
                ReadyWriteBase::bind_selected_retained(
                    &mut slot,
                    table,
                    handle,
                    claim,
                    &contract,
                    SelectedOperationPlan {
                        fragment: &fragment,
                        lowered: &lowered,
                        active: &active,
                        placement_id: &placement
                    }
                ),
                Err(WriteCallRefusal::WrongBinding)
            ),
            "mutation {mutation}"
        );
        assert_eq!(slot.as_ref().unwrap() as *const _, address);
        assert_eq!(slot.as_ref().unwrap().native_identity(), &identity);
        assert_eq!(state.edges.get(), 0);
    }
    let (fragment, lowered, active, placement) = selected();
    let mut scope = fixture::scope();
    scope.capability_id = WRITE_CAPABILITY.into();
    scope.authority_contract_id = WRITE_AUTHORITY.into();
    let (table, handle, claim) = fixture::authorization(scope, &fragment, &active);
    let mut owner = ReadyWriteBase::bind_selected_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(state.edges.get(), 0);
    let bytes = encoded();
    let (_, memory) = allocation_probe::observe(|| {
        owner
            .invoke(
                lowered.nodes[0].node,
                HostCallId(0),
                RequestId(0),
                &bytes,
                || false,
            )
            .unwrap();
        let mut retained = Some(owner);
        assert!(matches!(
            WriteHostCall::retire_retained(&mut retained),
            Err(RetirementRefusal::NotRevoked)
        ));
        retained
            .as_mut()
            .unwrap()
            .revoke(lowered.nodes[0].node, HostCallId(0))
            .unwrap();
        let (provider, attachment) = WriteHostCall::retire_retained(&mut retained).unwrap();
        assert!(Rc::ptr_eq(&provider.0, &state));
        assert_eq!(attachment.generation, 7);
        assert_eq!(attachment.resource_bytes, 4096);
        assert!(retained.is_none());
        assert!(matches!(
            WriteHostCall::<Provider>::retire_retained(&mut retained),
            Err(RetirementRefusal::Unavailable)
        ));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
