use super::*;

#[test]
fn kernel_request_replay_cannot_repeat_a_completed_native_effect() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(1), &input).err(),
        Some(EndpointReadOwnerRefusal::StaleTransfer)
    );
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(0), &input).err(),
        Some(EndpointReadOwnerRefusal::Pressure)
    );
    assert!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    ordinal: 0,
                    actual: 8,
                    input: &[0; 8],
                },
            )
        }
        .is_ok()
    );
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(0), &input).err(),
        Some(EndpointReadOwnerRefusal::StaleTransfer)
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
    assert!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &input)
            .is_ok()
    );
}

#[test]
fn pending_transfer_exerts_pressure_until_acknowledged_quiescence() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    assert_eq!(submission.length(), 8);
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(EndpointReadOwnerRefusal::Pressure)
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            NativeEndpointReadObservation::Completed {
                ordinal: 0,
                actual: 3,
                input: &[1, 2, 3],
            },
        )
    }
    .unwrap();
    assert!(conduit_core::validate_canonical_structured_value(result).is_ok());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    ordinal: 0,
                    actual: 3,
                    input: &[1, 2, 3],
                },
            )
        },
        Err(EndpointReadOwnerRefusal::StaleTransfer)
    );
}

#[test]
fn reusable_possession_and_malformed_physical_results_do_not_hide_work_or_retry() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    let allocations = crate::test_allocations::allocations(|| {
        for _ in 0..10_000 {
            let submission = owner
                .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
                .unwrap();
            assert!(
                unsafe {
                    owner.finish_quiesced(
                        &submission,
                        NativeEndpointReadObservation::Disposition(
                            EndpointReadDisposition::Stalled,
                        ),
                    )
                }
                .is_ok()
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
        10_000
    );
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointReadObservation::Completed {
                    ordinal: 0,
                    actual: 9,
                    input: &[0; 9],
                },
            )
        },
        Err(EndpointReadOwnerRefusal::Result(
            EndpointReadResultRefusal::ActualLength
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
        10_001
    );
}

#[test]
fn zero_or_oversized_read_is_refused_before_possession_is_consumed() {
    let mut owner = fixture::owner(7);
    for length in [0, 513, 2049, u64::MAX] {
        assert_eq!(
            owner
                .begin(
                    owner.node,
                    HostCallId(0),
                    RequestId(0),
                    &fixture::input_length(length)
                )
                .err(),
            Some(EndpointReadOwnerRefusal::Decode(
                EndpointReadRequestRefusal::Length
            ))
        );
        assert!(owner.pending.is_none());
        assert_eq!(owner.next_request, 0);
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
    let submission = owner
        .begin(owner.node, HostCallId(0), RequestId(0), &fixture::input())
        .unwrap();
    assert_eq!(submission.length(), 8);
    assert_eq!(submission.attachment().endpoint_dci, 3);
}
