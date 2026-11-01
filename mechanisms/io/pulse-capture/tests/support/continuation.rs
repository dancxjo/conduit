//! Actual public Ready -> bound -> revoked -> opaque return -> fresh Ready path.
use super::*;
use conduit_pulse_capture::installation::*;
fn identity(generation: u64) -> PulseNativeIdentity {
    let scope = fixture::scope();
    PulseNativeIdentity {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_id: "base/pulse".into(),
        provider_instance_id: scope.base_instance_id,
        provider_generation: generation,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        artifact_id: "artifact/pulse/fixture".into(),
        implementation_id: scope.implementation_id,
        base_implementation_id: "base/pulse/fixture@1".into(),
    }
}
fn initialized(state: Rc<State>, generation: u64) -> ReadyPulseBase<Provider> {
    // SAFETY: initial fixture owns exactly the inert finite/cancellable provider.
    unsafe {
        ReadyPulseBase::new(
            identity(generation),
            PulseAttachment {
                generation: 7,
                resource_bytes: 4096,
                limits: limits(),
            },
            Provider(state),
        )
    }
    .unwrap()
}
fn bind(
    slot: &mut Option<ReadyPulseBase<Provider>>,
    sequence: u64,
) -> (PulseHostCall<Provider>, NodeId, ActivePlayId) {
    let generation = slot.as_ref().unwrap().native_identity().provider_generation;
    let (mut fragment, _, _, placement) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = PULSE_CAPABILITY.into();
    gear.execution_profile_id = PULSE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = PULSE_AUTHORITY.into();
    gear.authority[0].capability_id = PULSE_CAPABILITY.into();
    let base = gear.base.as_mut().unwrap();
    base.mechanism_family = PULSE_ATTACHMENT_CLASS.into();
    base.provider_generation = generation;
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
    let active = bind_active_play(
        &plan.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        sequence,
    );
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let mut scope = fixture::scope();
    scope.base_provider_generation = generation;
    scope.capability_id = PULSE_CAPABILITY.into();
    scope.authority_contract_id = PULSE_AUTHORITY.into();
    let (table, handle, claim) = fixture::authorization(scope, fragment, &active);
    let owner = slot
        .take()
        .unwrap()
        .bind_selected(
            table,
            handle,
            claim,
            &PulseReadContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement,
            },
        )
        .unwrap();
    (owner, lowered.nodes[0].node, active.active_play_id)
}
fn retired(state: Rc<State>, generation: u64) -> RetiredPulseBase<Provider> {
    let (mut owner, node, _) = bind(&mut Some(initialized(state, generation)), 1);
    owner.revoke(node, HostCallId(0)).unwrap();
    PulseHostCall::retire_base_retained(&mut Some(owner)).unwrap()
}
#[test]
#[allow(clippy::result_large_err)] // Measure affine all-owner refusal without boxing.
fn every_native_identity_fact_is_sealed_and_refusal_preserves_actual_provider() {
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
            Ok(_) => panic!("forged native identity"),
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
        assert_eq!(state.effects.get(), 0);
    }
    let next = identity(5);
    let (result, memory) = allocation_probe::observe(|| token.reattach(next));
    let owner = match result {
        Ok(v) => v,
        Err(_) => panic!("valid custody lost"),
    };
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(owner.native_identity(), &identity(5));
}
#[test]
fn returned_provider_runs_only_after_a_distinct_independently_bound_play() {
    let state = Rc::new(State::default());
    let mut slot = Some(initialized(state.clone(), 4));
    let (mut first, node, old_play) = bind(&mut slot, 1);
    ready(first.invoke(node, HostCallId(0), RequestId(0), &request(5))).unwrap();
    first.revoke(node, HostCallId(0)).unwrap();
    assert!(matches!(
        ready(first.invoke(node, HostCallId(0), RequestId(1), &request(5))),
        Err(PulseCallRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
    let returned = PulseHostCall::retire_base_retained(&mut Some(first)).unwrap();
    let fresh = match returned.reattach(identity(5)) {
        Ok(v) => v,
        Err(_) => panic!("fresh readiness"),
    };
    let (mut second, node, new_play) = bind(&mut Some(fresh), 2);
    assert_ne!(old_play, new_play);
    ready(second.invoke(node, HostCallId(0), RequestId(0), &request(5))).unwrap();
    assert_eq!(state.effects.get(), 2);
}
#[test]
fn overflow_and_raw_unsafe_origins_cannot_create_safe_ready_continuation() {
    let state = Rc::new(State::default());
    let token = retired(state.clone(), u64::MAX);
    let (token, _, error) = match token.reattach(identity(u64::MAX)) {
        Err(v) => v,
        Ok(_) => panic!("generation wrapped"),
    };
    assert_eq!(error, ReattachRefusal::GenerationExhausted);
    assert_eq!(token.provider_generation(), u64::MAX);
    let (mut raw, node) = fixture::owner(Provider(state.clone()));
    raw.revoke(node, HostCallId(0)).unwrap();
    let raw = PulseHostCall::retire_base_retained(&mut Some(raw)).unwrap();
    let (raw, _, error) = match raw.reattach(identity(5)) {
        Err(v) => v,
        Ok(_) => panic!("raw origin forged"),
    };
    assert_eq!(error, ReattachRefusal::NoReadyOrigin);
    let (provider, attachment) = raw.into_parts();
    assert!(Rc::ptr_eq(&provider.0, &state));
    assert_eq!(attachment.generation, 7);
}
