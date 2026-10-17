//! Sign inspection work is bounded by occupied history, independently of the
//! admitted storage ceiling. Retention and remote correlation share that prefix.
use conduit_kernel::{
    remote_sign_storage_bytes, CordId, FixedSignLog, KernelEvent, KernelEventKind, NodeId, PortId,
    RemoteCordDirection, RemoteEndpointId, RemoteLifecycleIdentity, SignQuery, SignSink,
};
use std::mem::size_of;

fn exercise<L: SignSink + SignQuery>(
    mut log: L,
    view: impl Fn(&L) -> (Option<usize>, Vec<KernelEvent>),
) {
    let capacity = usize::from(log.item_capacity());
    assert_eq!(view(&log), (Some(0), vec![]));
    let identity = RemoteLifecycleIdentity {
        endpoint: RemoteEndpointId(1),
        cord: CordId(0),
        direction: RemoteCordDirection::Egress,
        sequence: 7,
    };
    let mut expected = Vec::new();
    let mut remote = None;
    for index in 0..=capacity {
        let event = if index == 1 {
            let event = log
                .record_remote(
                    NodeId(0),
                    PortId(0),
                    KernelEventKind::RemoteValueOffered,
                    identity,
                )
                .unwrap();
            remote = Some(event.sequence);
            event
        } else {
            log.record(NodeId(0), None, None, KernelEventKind::ValueConsumed)
                .unwrap()
        };
        if expected.len() == capacity {
            expected.remove(0);
        }
        expected.push(event);
        assert_eq!(view(&log), (Some(expected.len()), expected.clone()));
        assert!(!log.contains_kind(KernelEventKind::BackFailed));
        assert_eq!(log.remote_identity(u32::MAX), None);
        if let Some(sequence) = remote {
            assert_eq!(log.remote_identity(sequence), Some(identity));
        }
    }
    let gap = log.retention_gap().unwrap();
    assert_eq!(
        (gap.first_sequence, gap.last_sequence, gap.entries),
        (0, 0, 1)
    );
}

#[test]
fn fixed_queries_use_occupied_prefix_through_eviction() {
    let log = FixedSignLog::<128>::new_with_remote_storage(
        128 * size_of::<KernelEvent>() as u32,
        2,
        remote_sign_storage_bytes(2).unwrap(),
    )
    .unwrap();
    exercise(log, |log| {
        (log.events().size_hint().1, log.events().collect())
    });
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_queries_use_occupied_prefix_through_eviction() {
    let log = conduit_kernel::HostedSignLog::new_with_remote_storage(
        128,
        128 * size_of::<KernelEvent>() as u32,
        2,
        remote_sign_storage_bytes(2).unwrap(),
    )
    .unwrap();
    exercise(log, |log| {
        (log.events().size_hint().1, log.events().collect())
    });
}
