use super::*;

#[test]
fn foreign_handle_and_every_changed_operation_scope_refuse_before_submission() {
    let input = fixture::input();
    let mut owner = fixture::owner(7);
    let foreign = fixture::owner(8);
    owner.handle = foreign.handle;
    assert_eq!(
        owner
            .begin(
                owner.node,
                HostCallId(0),
                RequestId(owner.next_request),
                &input
            )
            .err(),
        Some(EndpointReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::UnknownCapability
        ))
    );
    assert!(owner.pending.is_none());
    type Change = fn(&mut BaseOperationClaim);
    let changes: [Change; 13] = [
        |claim| claim.host_id = "host/foreign".into(),
        |claim| claim.boot_id = "boot/foreign".into(),
        |claim| claim.base_instance_id = "base/foreign".into(),
        |claim| claim.base_provider_generation += 1,
        |claim| claim.plan_id = "plan/foreign".into(),
        |claim| claim.active_play_id = "play/foreign".into(),
        |claim| claim.implementation_id = "implementation/foreign".into(),
        |claim| claim.operation_contract_id = "call/foreign".into(),
        |claim| claim.subject_kind = "kind/foreign".into(),
        |claim| claim.resource_pool_id = "resource/foreign".into(),
        |claim| {
            claim.resource_generation_id =
                conduit_core::ResourceGenerationId("generation/foreign".into())
        },
        |claim| claim.envelope_id = "envelope/foreign".into(),
        |claim| {
            claim.plan_id = "plan/replacement".into();
            claim.active_play_id = "play/replacement".into();
        },
    ];
    for change in changes {
        let mut owner = fixture::owner(7);
        change(&mut owner.claim);
        assert_eq!(
            owner
                .begin(
                    owner.node,
                    HostCallId(0),
                    RequestId(owner.next_request),
                    &input
                )
                .err(),
            Some(EndpointReadOwnerRefusal::Capability(
                BaseCapabilityRefusal::WrongScope
            ))
        );
        assert!(owner.pending.is_none());
        assert_eq!(
            owner
                .table
                .inspections()
                .next()
                .unwrap()
                .in_flight_operations,
            0
        );
    }
}

#[test]
fn identical_descriptive_ids_cannot_substitute_another_issuers_transfer() {
    let mut owner = fixture::owner(7);
    let mut foreign = fixture::owner(8);
    let input = fixture::input();
    let node = owner.node;
    let genuine = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    let forged = foreign
        .begin(
            foreign.node,
            HostCallId(0),
            RequestId(foreign.next_request),
            &input,
        )
        .unwrap();
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &forged,
                NativeEndpointReadObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    assert!(
        unsafe {
            owner.finish_quiesced(
                &genuine,
                NativeEndpointReadObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        }
        .is_ok()
    );
}
