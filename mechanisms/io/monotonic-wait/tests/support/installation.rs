//! Installed ownership is fixture-only; no physical timer is asserted here.
use super::*;
use conduit_plan_lowering::lowering::lower_plan_fragment;
pub(super) fn identity() -> WaitNativeIdentity {
    let scope = fixture::scope();
    WaitNativeIdentity {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_id: "base/wait".into(),
        provider_instance_id: scope.base_instance_id,
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        artifact_id: "artifact/wait/fixture".into(),
        implementation_id: scope.implementation_id,
        base_implementation_id: "base/wait/fixture@1".into(),
    }
}
fn attachment() -> WaitAttachment {
    WaitAttachment {
        generation: 7,
        resource_bytes: 4096,
        maximum_duration_ms: 1000,
    }
}
pub(super) fn ready(state: Rc<State>) -> ReadyMonotonicWaitBase<Provider> {
    // SAFETY: inert CPU timer fixture is retained and follows the tested drop law.
    unsafe { ReadyMonotonicWaitBase::new(identity(), attachment(), Provider(state)) }.unwrap()
}
fn advertisement() -> HostAdvertisement {
    let identity = identity();
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id,
        boot_id: identity.boot_id,
        offer_generation: OfferGeneration(1),
        profile: "fixture/wait".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn only_retained_native_slot_projects_its_exact_resource_and_zero_result_offer() {
    let state = Rc::new(State::default());
    let ready = ready(state.clone());
    let mut host = advertisement();
    ready.append_to_advertisement(&mut host).unwrap();
    assert_eq!(host.bases.len(), 1);
    assert_eq!(host.resources.len(), 1);
    assert_eq!(host.capabilities.len(), 1);
    let offer = &host.capabilities[0];
    assert_eq!(offer.kind_id.as_str(), conduit_time::TICK_KIND);
    assert_eq!(
        offer.host_calls[0].contract_id.as_str(),
        WAIT_HOST_CALL_CONTRACT
    );
    assert_eq!(offer.host_calls[0].maximum_input_bytes, 8);
    assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        WAIT_AUTHORITY
    );
    assert_eq!(
        host.bases[0].provider_instance_id,
        identity().provider_instance_id
    );
    let retained = host.clone();
    assert!(ready.append_to_advertisement(&mut host).is_err());
    assert_eq!(host, retained);
    host.boot_id = "fixture/stale".into();
    let retained = host.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut host),
        Err(WaitInstallationRefusal::WrongBoot)
    );
    assert_eq!(host, retained);
    assert_eq!(state.calls.get(), 0);
}
#[test]
fn retained_slot_survives_binding_refusal_and_transfers_only_after_complete_preflight() {
    retained_transfer(0);
}
#[test]
fn verified_slot_survives_binding_refusal_and_transfers_only_after_complete_preflight() {
    retained_transfer(1);
}
#[test]
fn direct_slot_survives_binding_refusal_and_transfers_only_after_complete_preflight() {
    retained_transfer(2);
}
fn retained_transfer(verified: u8) {
    let state = Rc::new(State::default());
    let mut slot = Some(ready(state.clone()));
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = WAIT_CAPABILITY.into();
    gear.execution_profile_id = WAIT_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = WAIT_AUTHORITY.into();
    gear.authority[0].capability_id = WAIT_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = WAIT_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = WAIT_ATTACHMENT_CLASS.into();
    let plot_identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(plot_identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let issue = || {
        let mut scope = fixture::scope();
        scope.capability_id = WAIT_CAPABILITY.into();
        scope.authority_contract_id = WAIT_AUTHORITY.into();
        fixture::authorization(scope, &fragment, &active)
    };
    let mut foreign = active.clone();
    foreign.boot_id = "fixture/stale".into();
    let (table, handle, claim) = issue();
    assert!(bind_variant(
        verified,
        &mut slot,
        table,
        handle,
        claim,
        &fixture::configuration(),
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &foreign,
            placement_id: &placement
        }
    )
    .is_err());
    assert!(slot.is_some());
    assert_eq!(state.calls.get(), 0);
    let mut changed_configuration = fixture::configuration();
    changed_configuration[1].value = ConfigurationValue::U64(251);
    let (table, handle, claim) = issue();
    assert!(matches!(
        bind_variant(
            verified,
            &mut slot,
            table,
            handle,
            claim,
            &changed_configuration,
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
        ),
        Err(WaitCallRefusal::WrongBinding)
    ));
    assert!(slot.is_some());
    assert_eq!(state.calls.get(), 0);
    let mut small_scope = fixture::scope();
    small_scope.capability_id = WAIT_CAPABILITY.into();
    small_scope.authority_contract_id = WAIT_AUTHORITY.into();
    small_scope.maximum_operations = 2; // independently checked Source needs three
    let (table, handle, claim) = fixture::authorization(small_scope, &fragment, &active);
    assert!(matches!(
        bind_variant(
            verified,
            &mut slot,
            table,
            handle,
            claim,
            &fixture::configuration(),
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            }
        ),
        Err(WaitCallRefusal::Possession)
    ));
    assert!(slot.is_some());
    let mut too_short = Some(
        unsafe {
            ReadyMonotonicWaitBase::new(
                identity(),
                WaitAttachment {
                    maximum_duration_ms: 100,
                    ..attachment()
                },
                Provider(state.clone()),
            )
        }
        .unwrap(),
    );
    let (table, handle, claim) = issue();
    assert!(matches!(
        bind_variant(
            verified,
            &mut too_short,
            table,
            handle,
            claim,
            &fixture::configuration(),
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            }
        ),
        Err(WaitCallRefusal::InvalidDuration)
    ));
    assert!(too_short.is_some());
    assert_eq!(state.calls.get(), 0);
    let (table, handle, claim) = issue();
    let mut owner = bind_variant(
        verified,
        &mut slot,
        table,
        handle,
        claim,
        &fixture::configuration(),
        SelectedOperationPlan {
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &placement,
        },
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(
        immediate(owner.invoke(
            lowered.nodes[0].node,
            HostCallId(0),
            RequestId(0),
            &encode_monotonic_duration(250)
        )),
        Ok(())
    );
    assert_eq!(state.calls.get(), 1);
}

fn bind_variant(
    verified: u8,
    slot: &mut Option<ReadyMonotonicWaitBase<Provider>>,
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    configuration: &[ConfigurationEntry],
    selection: SelectedOperationPlan<'_>,
) -> Result<MonotonicWaitHostCall<Provider>, WaitCallRefusal> {
    if verified == 0 {
        return ReadyMonotonicWaitBase::bind_selected_retained(
            slot,
            table,
            handle,
            claim,
            configuration,
            selection,
        );
    }
    let fragment = selection.fragment;
    let mut plan = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment.clone()],
    );
    for gear in &mut plan.fragments[0].placements {
        gear.configuration.clear();
    }
    let plan = verify_plan_metadata_with_configurations(plan, |gear| {
        Ok::<_, ()>(
            fragment
                .placements
                .iter()
                .find(|original| original.placement_id == gear.placement_id)
                .unwrap()
                .configuration
                .clone(),
        )
    })
    .unwrap();
    if verified == 2 {
        let tables=conduit_plan_lowering::local_tables::DirectLocalKernelTables::<1,0,1,1,0,1>::prepare(&plan,&fragment.fragment_id).unwrap();
        return ReadyMonotonicWaitBase::bind_direct_retained(
            slot,
            table,
            handle,
            claim,
            configuration,
            conduit_plan_lowering::selected_operation::SelectedDirectOperationPlan {
                tables: &tables,
                active: selection.active,
                placement_id: selection.placement_id,
            },
        );
    }
    ReadyMonotonicWaitBase::bind_verified_retained(
        slot,
        table,
        handle,
        claim,
        configuration,
        conduit_plan_lowering::selected_operation::SelectedVerifiedOperationPlan {
            plan: &plan,
            fragment_id: &fragment.fragment_id,
            lowered: selection.lowered,
            active: selection.active,
            placement_id: selection.placement_id,
        },
    )
}
