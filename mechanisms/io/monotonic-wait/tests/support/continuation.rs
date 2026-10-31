//! Public Ready-origin continuation keeps actual timer custody and finite grants distinct.
use super::*;
fn identity(generation: u64) -> WaitNativeIdentity {
    let mut value = installation::identity();
    value.provider_generation = generation;
    value
}
fn initialized(state: Rc<State>, generation: u64) -> ReadyMonotonicWaitBase<Provider> {
    // SAFETY: initial inert CPU timer owns its counted state and follows drop/quiescence.
    unsafe {
        ReadyMonotonicWaitBase::new(
            identity(generation),
            WaitAttachment {
                generation: 7,
                resource_bytes: 4096,
                maximum_duration_ms: 1000,
            },
            Provider(state),
        )
    }
    .unwrap()
}
fn bind(
    slot: &mut Option<ReadyMonotonicWaitBase<Provider>>,
    sequence: u64,
) -> (MonotonicWaitHostCall<Provider>, NodeId, ActivePlayId) {
    let generation = slot.as_ref().unwrap().native_identity().provider_generation;
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = WAIT_CAPABILITY.into();
    gear.execution_profile_id = WAIT_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = WAIT_AUTHORITY.into();
    gear.authority[0].capability_id = WAIT_CAPABILITY.into();
    let base = gear.base.as_mut().unwrap();
    base.mechanism_family = WAIT_ATTACHMENT_CLASS.into();
    base.provider_generation = generation;
    gear.resources[0].class_id = WAIT_ATTACHMENT_CLASS.into();
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
    let active = bind_active_play(
        &fragment.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        sequence,
    );
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let mut scope = fixture::scope();
    scope.base_provider_generation = generation;
    scope.capability_id = WAIT_CAPABILITY.into();
    scope.authority_contract_id = WAIT_AUTHORITY.into();
    let (table, handle, claim) = fixture::authorization(scope, &fragment, &active);
    let owner = ReadyMonotonicWaitBase::bind_selected_retained(
        slot,
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
    (owner, lowered.nodes[0].node, active.active_play_id)
}
fn retired(state: Rc<State>, generation: u64) -> RetiredWaitBase<Provider> {
    let (mut owner, node, _) = bind(&mut Some(initialized(state, generation)), 1);
    owner.revoke(node, HostCallId(0)).unwrap();
    MonotonicWaitHostCall::retire_base_retained(&mut Some(owner)).unwrap()
}
#[test]
#[allow(clippy::result_large_err)] // Measure affine all-owner refusal without boxing.
fn full_timer_native_identity_is_sealed_and_every_refusal_keeps_the_owner() {
    let state = Rc::new(State::default());
    let mut token = retired(state.clone(), 4);
    for field in 0..11 {
        let mut candidate = identity(5);
        match field {
            0 => candidate.host_id = "wrong".into(),
            1 => candidate.boot_id = "wrong".into(),
            2 => candidate.base_id = "wrong".into(),
            3 => candidate.provider_instance_id = "wrong".into(),
            4 => candidate.resource_pool_id = "wrong".into(),
            5 => candidate.resource_generation_id = ResourceGenerationId("wrong".into()),
            6 => candidate.envelope_id = "wrong".into(),
            7 => candidate.artifact_id = "wrong".into(),
            8 => candidate.implementation_id = "wrong".into(),
            9 => candidate.base_implementation_id = "wrong".into(),
            10 => candidate.provider_generation = 4,
            _ => unreachable!(),
        }
        let (result, memory) = allocation_probe::observe(|| token.reattach(candidate));
        let (returned, _, error) = match result {
            Err(v) => v,
            Ok(_) => panic!("forged timer identity"),
        };
        token = returned;
        assert_eq!(
            error,
            if field == 10 {
                ReattachRefusal::GenerationMismatch
            } else {
                ReattachRefusal::IdentityMismatch
            }
        );
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(token.provider_generation(), 4);
        assert_eq!(token.attachment().generation, 7);
        assert_eq!(state.calls.get(), 0);
    }
    let candidate = identity(5);
    let (result, memory) = allocation_probe::observe(|| token.reattach(candidate));
    let ready = match result {
        Ok(v) => v,
        Err(_) => panic!("valid timer custody"),
    };
    assert_eq!(ready.native_identity(), &identity(5));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn second_timer_play_uses_the_same_provider_only_after_fresh_binding() {
    let state = Rc::new(State::default());
    let (mut first, node, old) = bind(&mut Some(initialized(state.clone(), 4)), 1);
    immediate(first.invoke(
        node,
        HostCallId(0),
        RequestId(0),
        &encode_monotonic_duration(250),
    ))
    .unwrap();
    first.revoke(node, HostCallId(0)).unwrap();
    assert!(matches!(
        immediate(first.invoke(
            node,
            HostCallId(0),
            RequestId(1),
            &encode_monotonic_duration(250)
        )),
        Err(WaitCallRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
    let token = MonotonicWaitHostCall::retire_base_retained(&mut Some(first)).unwrap();
    let ready = match token.reattach(identity(5)) {
        Ok(v) => v,
        Err(_) => panic!("fresh timer readiness"),
    };
    let (mut second, node, new) = bind(&mut Some(ready), 2);
    assert_ne!(old, new);
    immediate(second.invoke(
        node,
        HostCallId(0),
        RequestId(0),
        &encode_monotonic_duration(250),
    ))
    .unwrap();
    assert_eq!(state.calls.get(), 2);
}
#[test]
fn timer_generation_overflow_and_untracked_unsafe_origins_refuse_without_loss() {
    let state = Rc::new(State::default());
    let token = retired(state.clone(), u64::MAX);
    let (token, _, error) = match token.reattach(identity(u64::MAX)) {
        Err(v) => v,
        Ok(_) => panic!("timer generation wrapped"),
    };
    assert_eq!(error, ReattachRefusal::GenerationExhausted);
    assert_eq!(token.provider_generation(), u64::MAX);
    let (mut raw, node) = fixture::owner(Provider(state.clone()));
    raw.revoke(node, HostCallId(0)).unwrap();
    let raw = MonotonicWaitHostCall::retire_base_retained(&mut Some(raw)).unwrap();
    let (raw, _, error) = match raw.reattach(identity(5)) {
        Err(v) => v,
        Ok(_) => panic!("raw origin forged"),
    };
    assert_eq!(error, ReattachRefusal::NoReadyOrigin);
    let (provider, attachment) = raw.into_parts();
    assert!(Rc::ptr_eq(&provider.0, &state));
    assert_eq!(attachment.generation, 7);
}
