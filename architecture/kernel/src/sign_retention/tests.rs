use super::*;
use crate::{FixedSignLog, NodeId, PortId, SignSink};

fn check(log: &impl SignSink, events: impl Iterator<Item = KernelEvent>) -> u16 {
    // Independent history oracle; only the tests walk the actual log.
    let retained = events.filter(|event| !transient_sign(event.kind)).count() as u16;
    let charge = size_of::<KernelEvent>() as u32;
    for additional in 0..=log.item_capacity() + 1 {
        let items = u32::from(retained) + u32::from(additional);
        let expected = if items > u32::from(log.item_capacity()) {
            Err(SignError::ItemCapacityExceeded)
        } else if items * charge > log.byte_capacity() {
            Err(SignError::ByteCapacityExceeded)
        } else {
            Ok(())
        };
        assert_eq!(log.ensure_capacity(additional), expected);
    }
    retained
}

fn fixed_patterns<const N: usize>() {
    for byte_items in 1..=N {
        for pattern in 0..(1u16 << N) {
            let mut log =
                FixedSignLog::<N>::new((byte_items * size_of::<KernelEvent>()) as u32).unwrap();
            for index in 0..N * 3 {
                assert_eq!(log.retained_items, check(&log, log.events()));
                let kind = if pattern & (1 << (index % N)) == 0 {
                    KernelEventKind::ValueConsumed
                } else {
                    KernelEventKind::BackCompleted
                };
                let _ = log.record(NodeId(0), None, None, kind);
            }
            assert_eq!(log.retained_items, check(&log, log.events()));
        }
    }
}

#[test]
fn fixed_retained_count_matches_history_through_eviction_and_refused_records() {
    fixed_patterns::<1>();
    fixed_patterns::<2>();
    fixed_patterns::<4>();
    fixed_patterns::<8>();
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_retained_count_matches_history_through_eviction_and_refused_records() {
    for capacity in 1..=8 {
        for byte_items in 1..=capacity {
            for pattern in 0..(1u16 << capacity) {
                let mut log = crate::HostedSignLog::new(
                    capacity,
                    u32::from(byte_items) * size_of::<KernelEvent>() as u32,
                )
                .unwrap();
                for index in 0..capacity * 3 {
                    assert_eq!(log.retained_items, check(&log, log.events()));
                    let kind = if pattern & (1 << (index % capacity)) == 0 {
                        KernelEventKind::ValueConsumed
                    } else {
                        KernelEventKind::BackFailed
                    };
                    let _ = log.record(NodeId(0), None, None, kind);
                }
                assert_eq!(log.retained_items, check(&log, log.events()));
            }
        }
    }
}

#[test]
fn maximum_reservation_uses_retained_count_without_narrowing_overflow() {
    assert_eq!(
        ensure_retained_sign_capacity(0, u16::MAX, u32::MAX, u16::MAX),
        Ok(())
    );
    assert_eq!(
        ensure_retained_sign_capacity(1, u16::MAX, u32::MAX, u16::MAX),
        Err(SignError::ItemCapacityExceeded)
    );
}

macro_rules! remote_and_sequence_refusals {
    ($log:expr) => {{
        let mut log = $log;
        let remote = crate::RemoteLifecycleIdentity {
            endpoint: crate::RemoteEndpointId(0),
            cord: crate::CordId(0),
            direction: crate::RemoteCordDirection::Egress,
            sequence: 0,
        };
        log.record(NodeId(0), None, None, KernelEventKind::ValueConsumed)
            .unwrap();
        log.record_remote(
            NodeId(0),
            PortId(0),
            KernelEventKind::RemoteValueOffered,
            remote,
        )
        .unwrap();
        assert_eq!(log.retained_items, check(&log, log.events()));
        assert_eq!(
            log.record_remote(
                NodeId(0),
                PortId(0),
                KernelEventKind::RemoteValueOffered,
                remote
            ),
            Err(SignError::RemoteItemCapacityExceeded)
        );
        assert_eq!(log.retained_items, check(&log, log.events()));
        log.next_sequence = u32::MAX;
        assert_eq!(
            log.record(NodeId(0), None, None, KernelEventKind::BackCompleted),
            Err(SignError::SequenceOverflow)
        );
        assert_eq!(log.retained_items, check(&log, log.events()));
    }};
}

#[test]
fn fixed_remote_and_sequence_failures_do_not_publish_retained_entries() {
    remote_and_sequence_refusals!(FixedSignLog::<2>::new_with_remote_storage(
        2 * size_of::<KernelEvent>() as u32,
        1,
        crate::remote_sign_storage_bytes(1).unwrap()
    )
    .unwrap());
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_remote_and_sequence_failures_do_not_publish_retained_entries() {
    remote_and_sequence_refusals!(crate::HostedSignLog::new_with_remote_storage(
        2,
        2 * size_of::<KernelEvent>() as u32,
        1,
        crate::remote_sign_storage_bytes(1).unwrap()
    )
    .unwrap());
}
