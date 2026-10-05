use super::*;

#[test]
fn stale_physical_attachment_cannot_release_a_genuine_pending_transfer() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let mut submission = owner
        .begin(
            owner.node,
            HostCallId(0),
            RequestId(owner.next_request),
            &input,
        )
        .unwrap();
    submission.attachment.generation += 1;
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Disposition(EndpointReadDisposition::ProviderLost),
            )
        },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    submission.attachment.generation -= 1;
    submission.attachment.slot += 1;
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Disposition(EndpointReadDisposition::ProviderLost),
            )
        },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    submission.attachment.slot -= 1;
    assert!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Disposition(EndpointReadDisposition::ProviderLost),
            )
        }
        .is_ok()
    );
}

#[test]
fn endpoint_and_device_generation_changes_never_release_a_pending_receive() {
    type Change = fn(&mut EndpointReadAttachment);
    let changes: [Change; 4] = [
        |attachment| attachment.slot += 1,
        |attachment| attachment.generation += 1,
        |attachment| attachment.endpoint_dci += 2,
        |attachment| attachment.endpoint_generation += 1,
    ];
    for change in changes {
        let mut owner = fixture::owner(7);
        let mut submission = owner
            .begin(owner.node, HostCallId(0), RequestId(0), &fixture::input())
            .unwrap();
        let original = submission.attachment;
        change(&mut submission.attachment);
        assert_eq!(
            unsafe {
                owner.finish_quiesced(
                    &submission,
                    NativeEndpointReadObservation::Disposition(
                        EndpointReadDisposition::ProviderLost,
                    ),
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
        submission.attachment = original;
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        }
        .unwrap();
    }
}
