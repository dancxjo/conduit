use super::*;
mod fixture;

#[test]
fn zero_length_submission_retains_authority_until_actual_acknowledgement() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &fixture::input_wire(&[]))
        .unwrap();
    assert_eq!(submission.length(), 0);
    assert!(submission.wire().is_empty());
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
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &fixture::input())
            .err(),
        Some(EndpointWriteOwnerRefusal::Pressure)
    );
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            NativeEndpointWriteObservation::Completed { actual: 0 },
        )
    }
    .unwrap();
    let value = conduit_core::validate_canonical_structured_value(result).unwrap();
    assert_eq!(value.variant_tag().unwrap(), "completed");
    let completion = value.variant_payload("completed").unwrap().unwrap();
    assert_eq!(
        completion
            .record_field("actual")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        &0u64.to_le_bytes()
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
}

#[test]
fn exact_wire_is_owned_and_replay_cannot_repeat_an_out_effect() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let mut bytes = fixture::input_wire(&[1, 2, 3, 4]);
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &bytes)
        .unwrap();
    bytes.fill(0);
    assert_eq!(submission.wire(), &[1, 2, 3, 4]);
    unsafe {
        owner.finish_quiesced(
            &submission,
            NativeEndpointWriteObservation::Completed { actual: 4 },
        )
    }
    .unwrap();
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(0), &fixture::input())
            .err(),
        Some(EndpointWriteOwnerRefusal::StaleTransfer)
    );
}

#[test]
fn software_revocation_preserves_physical_pending_until_exact_stop() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &fixture::input())
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &fixture::input())
            .err(),
        Some(EndpointWriteOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    );
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) },
        Err(EndpointWriteOwnerRefusal::Capability(
            BaseCapabilityRefusal::StaleCompletion
        ))
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) },
        Err(EndpointWriteOwnerRefusal::StaleTransfer)
    );
}

#[test]
fn foreign_attachment_and_generation_cannot_release_real_pending_dma() {
    let changes: &[fn(&mut EndpointWriteAttachment)] = &[
        |a| a.slot += 1,
        |a| a.generation += 1,
        |a| a.endpoint_dci += 2,
        |a| a.endpoint_generation += 1,
    ];
    for change in changes {
        let mut owner = fixture::owner(7);
        let node = owner.node;
        let mut submission = owner
            .begin(node, HostCallId(0), RequestId(0), &fixture::input())
            .unwrap();
        let original = submission.attachment;
        change(&mut submission.attachment);
        assert_eq!(
            unsafe { owner.discard_quiesced(&submission) },
            Err(EndpointWriteOwnerRefusal::StaleTransfer)
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
        submission.attachment = original;
        unsafe { owner.discard_quiesced(&submission) }.unwrap();
    }
}

#[test]
fn matching_ids_cannot_substitute_the_lowered_call_route() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(EndpointWriteOwnerRefusal::WrongBinding)
    );
}

#[test]
fn selected_native_identity_and_resource_generation_are_exact() {
    let changes: &[fn(&mut conduit_core::BaseOperationClaim)] = &[
        |claim| claim.host_id = "host/foreign".into(),
        |claim| claim.boot_id = "boot/stale".into(),
        |claim| claim.base_instance_id = "provider/foreign".into(),
        |claim| claim.base_provider_generation += 1,
        |claim| claim.resource_pool_id = "pool/foreign".into(),
        |claim| claim.resource_generation_id.0.push_str("/stale"),
        |claim| claim.envelope_id = "envelope/foreign".into(),
    ];
    for change in changes {
        let (fragment, lowered, active, placement) = fixture::selected();
        let mut table = fixture::table(7);
        let mut request = fixture::request();
        request.scope.plan_id = fragment.plan_id.clone();
        request.scope.active_play_id = active.active_play_id.clone();
        let handle = table.issue(request).unwrap();
        let mut claim = fixture::claim();
        claim.plan_id = fragment.plan_id.clone();
        claim.active_play_id = active.active_play_id.clone();
        change(&mut claim);
        let owner = unsafe {
            EndpointWriteCallOwner::bind_admitted(
                table,
                handle,
                claim,
                EndpointWriteAttachment {
                    slot: 1,
                    generation: 7,
                    endpoint_generation: 9,
                    endpoint_dci: 2,
                    maximum_data_bytes: 512,
                    resource_bytes: 4096,
                },
                &EndpointWriteContract::prepare().unwrap(),
                SelectedOperationPlan {
                    fragment: &fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &placement,
                },
            )
        };
        assert_eq!(owner.err(), Some(EndpointWriteOwnerRefusal::WrongBinding));
    }
}

mod native_fault;
