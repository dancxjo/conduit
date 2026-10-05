//! Acknowledged stop releases retained possession without publishing a result.
use super::*;

#[test]
fn software_revocation_retains_the_transfer_until_exact_stop_acknowledgement() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &fixture::input())
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    // This fixture supplies the native stop acknowledgement explicitly.
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) },
        Err(EndpointReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::StaleCompletion
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
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
}

#[test]
fn another_attachment_cannot_supply_a_stop_acknowledgement() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let mut submission = owner
        .begin(node, HostCallId(0), RequestId(0), &fixture::input())
        .unwrap();
    submission.attachment.generation += 1;
    assert_eq!(
        unsafe { owner.discard_quiesced(&submission) },
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
}

#[test]
fn revocation_keeps_physical_pending_and_rejects_late_software_completion() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner.table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Revoked
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(EndpointReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    );
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        },
        Err(EndpointReadOwnerRefusal::Capability(
            BaseCapabilityRefusal::StaleCompletion
        ))
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        0
    );
}
