//! Exact cooperative network admission; no live service or USB proof.
use super::*;
#[path = "fixture.rs"]
mod fixture;

#[test]
fn unrelated_network_authority_cannot_bind_frame_queues() {
    let queues = EthernetFrameQueues::prepare(4, [2, 1, 2, 3, 4, 5]).unwrap();
    let (fragment, lowered, active, placement) = fixture::selected();
    let mut table = fixture::table(7);
    let mut issue = fixture::request();
    issue.scope.authority_contract_id = "conduit.authority/network-config@1".into();
    issue.authority.grant.contract_id = issue.scope.authority_contract_id.clone();
    issue.scope.plan_id = fragment.plan_id.clone();
    issue.scope.active_play_id = active.active_play_id.clone();
    let handle = table.issue(issue).unwrap();
    let mut claim = fixture::claim();
    claim.plan_id = fragment.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    let result = unsafe {
        EthernetFrameCallOwner::bind_admitted(
            &queues,
            table,
            handle,
            claim,
            &EthernetFrameContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement,
            },
        )
    };
    assert!(matches!(result, Err(FrameCallRefusal::Possession)));
}

#[test]
fn exact_selected_owner_refuses_replay_and_revocation_before_queue_effects() {
    let queues = EthernetFrameQueues::prepare(4, [2, 1, 2, 3, 4, 5]).unwrap();
    let (fragment, lowered, active, placement) = fixture::selected();
    let mut table = fixture::table(7);
    let mut issue = fixture::request();
    issue.scope.plan_id = fragment.plan_id.clone();
    issue.scope.active_play_id = active.active_play_id.clone();
    let handle = table.issue(issue).unwrap();
    let mut claim = fixture::claim();
    claim.plan_id = fragment.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    let contract = EthernetFrameContract::prepare().unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.request_type().shape() else {
        panic!("variant")
    };
    let case = cases
        .iter()
        .find(|case| case.tag() == "take-transmit")
        .unwrap();
    let input = StructuredInfoValue::variant(
        contract.request_type().clone(),
        "take-transmit",
        StructuredInfoValue::leaf(case.payload_type().clone(), alloc::vec![]).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut owner = unsafe {
        EthernetFrameCallOwner::bind_admitted(
            &queues,
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
    }
    .unwrap();
    let node = owner.node;
    let Err(FrameCallRefusal::Canonical(error)) =
        owner.invoke(node, HostCallId(0), RequestId(0), &[])
    else {
        panic!("malformed packet accepted")
    };
    assert!(!alloc::format!("{error:?}").is_empty());
    assert_eq!(owner.next, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    let mut frame = [0; FRAME_BYTES];
    frame[..6].fill(0xff);
    frame[6..12].copy_from_slice(&queues.mac());
    frame[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    let frame_allocations = crate::test_allocations::allocations(|| {
        queues.enqueue_transmit(4, &frame).unwrap();
        let output = owner
            .invoke(node, HostCallId(0), RequestId(0), &input)
            .unwrap();
        let output = validate_canonical_structured_value(output).unwrap();
        assert_eq!(output.variant_tag().unwrap(), "transmit");
        let payload = output.variant_payload("transmit").unwrap().unwrap();
        assert_eq!(
            payload
                .record_field("wire")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bytes")
                .unwrap(),
            frame
        );
        queues
            .acknowledge_transmit(4, 0, FRAME_BYTES as u64)
            .unwrap();
    });
    assert_eq!(frame_allocations, 0);
    let allocations = crate::test_allocations::allocations(|| {
        for request in 1..1025 {
            let output = owner
                .invoke(node, HostCallId(0), RequestId(request), &input)
                .unwrap();
            assert_eq!(
                validate_canonical_structured_value(output)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "pressure"
            );
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1025
    );
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(1024), &input),
        Err(FrameCallRefusal::StaleRequest)
    ));
    owner.cancel().unwrap();
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(1025), &input),
        Err(FrameCallRefusal::Capability(BaseCapabilityRefusal::Revoked))
    ));
}
