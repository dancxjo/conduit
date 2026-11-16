extern crate std;
use super::*;
use crate::{remote_sign_storage_bytes, CordId, RemoteCordDirection, RemoteEndpointId};
use alloc::vec::Vec;

type Log = LiveSignLog<32, 4, 2>;
fn new_active() -> HostedSignLog {
    HostedSignLog::new_with_remote_storage(
        64,
        64 * size_of::<KernelEvent>() as u32,
        6,
        remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap()
}
fn limits<const E: usize, const R: usize, const S: usize>(active: &HostedSignLog) -> ArchiveLimits {
    ArchiveLimits {
        retained_events: 10_000,
        retained_bytes: 1_000_000,
        archive_backing_bytes: LiveSignLog::<E, R, S>::required_archive_backing_bytes(),
        transfer_overlap_bytes: LiveSignLog::<E, R, S>::required_transfer_overlap_bytes(active)
            .unwrap(),
    }
}
fn new() -> Log {
    let active = new_active();
    let limits = limits::<32, 4, 2>(&active);
    Log::prepare(active, limits).unwrap()
}
fn duplex<const E: usize, const R: usize, const S: usize, A: ArchiveBacking<E, R, S>>(
    log: &mut LiveSignLog<E, R, S, A>,
    sequence: u64,
) -> Vec<ArchivedSign> {
    duplex_at(log, sequence, NodeId(7))
}
fn duplex_at<const E: usize, const R: usize, const S: usize, A: ArchiveBacking<E, R, S>>(
    log: &mut LiveSignLog<E, R, S, A>,
    sequence: u64,
    node: NodeId,
) -> Vec<ArchivedSign> {
    [
        (
            KernelEventKind::RemoteInputAdmitted,
            RemoteCordDirection::Ingress,
        ),
        (
            KernelEventKind::RemoteValueOffered,
            RemoteCordDirection::Egress,
        ),
        (
            KernelEventKind::RemoteValueAccepted,
            RemoteCordDirection::Egress,
        ),
        (
            KernelEventKind::RemoteValueDelivered,
            RemoteCordDirection::Egress,
        ),
    ]
    .into_iter()
    .map(|(kind, direction)| {
        let stream = match direction {
            RemoteCordDirection::Ingress => 0,
            RemoteCordDirection::Egress => 1,
        };
        let remote = RemoteLifecycleIdentity {
            endpoint: RemoteEndpointId(stream),
            cord: CordId(stream),
            direction,
            sequence,
        };
        let event = log
            .record_remote(node, PortId(stream), kind, remote)
            .unwrap();
        ArchivedSign {
            event,
            remote: Some(remote),
        }
    })
    .collect()
}
fn snapshot<const E: usize, const R: usize, const S: usize>(
    log: &LiveSignLog<E, R, S>,
) -> (
    Vec<ArchivedSign>,
    u32,
    u64,
    u16,
    u32,
    u32,
    Option<SignRetentionGap>,
) {
    (
        log.events_with_remote().collect(),
        log.archived_events,
        log.archived_bytes,
        log.active.len,
        log.active.next_sequence,
        log.active.used_bytes,
        log.retention_gap(),
    )
}

#[test]
fn all_1264_duplex_reads_reconstruct_every_original_event_and_sidecar() {
    let mut log = new();
    let original_item_capacity = log.item_capacity();
    let original_byte_capacity = log.byte_capacity();
    let backing = log.owned_backing_bytes();
    let mut expected = Vec::new();
    for read in 0..1264 {
        expected.extend(duplex(&mut log, read));
        let next = log.active.next_sequence;
        let transferred = log.prepare_prefix_transfer().unwrap().commit();
        assert_eq!(transferred.events, 4);
        assert_eq!(transferred.next_sequence, next);
        assert_eq!(log.active.next_sequence, next);
        assert_eq!(log.len(), 0);
        assert_eq!(log.item_capacity(), original_item_capacity);
        assert_eq!(log.byte_capacity(), original_byte_capacity);
        assert_eq!(log.owned_backing_bytes(), backing);
        assert_eq!(
            log.events().last(),
            expected.last().map(|entry| entry.event)
        );
    }
    assert_eq!(log.archived_event_count(), 5056);
    assert_eq!(log.archive.len(), 1264);
    assert_eq!(log.events_with_remote().collect::<Vec<_>>(), expected);
    for entry in &expected {
        assert_eq!(log.remote_identity(entry.event.sequence), entry.remote);
    }
    let event = log
        .record(
            NodeId(3),
            None,
            Some(RequestId(19)),
            KernelEventKind::HostCallCompleted,
        )
        .unwrap();
    assert_eq!(event.sequence, 5056);
    assert_eq!(log.events().last(), Some(event));
    let mut iterator = log.events_with_remote();
    assert_eq!(iterator.next(), expected.first().copied());
    assert_eq!(iterator.last().unwrap().event, event);
}

#[test]
fn interleaved_sampling_variation_preserves_request_and_terminal_truth() {
    let mut log = new();
    let mut expected = Vec::new();
    for read in 0..12 {
        // Three finite trace runs: before, during, and after a sensor completion.
        if (4..8).contains(&read) {
            let event = log
                .record(
                    NodeId(3),
                    None,
                    Some(RequestId(read * 2)),
                    KernelEventKind::HostCallCompleted,
                )
                .unwrap();
            expected.push(ArchivedSign {
                event,
                remote: None,
            });
        }
        expected.extend(duplex(&mut log, u64::from(read)));
        log.prepare_prefix_transfer().unwrap().commit();
    }
    for kind in [
        KernelEventKind::BackCompleted,
        KernelEventKind::SemanticAbnormal,
        KernelEventKind::SemanticAbnormalRecovered,
        KernelEventKind::RunCancelled,
    ] {
        let event = log.record(NodeId(4), None, None, kind).unwrap();
        expected.push(ArchivedSign {
            event,
            remote: None,
        });
    }
    log.prepare_prefix_transfer().unwrap().commit();
    assert_eq!(log.events_with_remote().collect::<Vec<_>>(), expected);
    for kind in [
        KernelEventKind::BackCompleted,
        KernelEventKind::SemanticAbnormal,
        KernelEventKind::SemanticAbnormalRecovered,
        KernelEventKind::RunCancelled,
    ] {
        assert!(log.contains_kind(kind));
    }
    assert!(!log.contains_kind(KernelEventKind::BackFailed));
}

#[test]
fn dropped_preflight_is_not_transfer_or_new_active_capacity() {
    let mut log = new();
    duplex(&mut log, 0);
    let before = snapshot(&log);
    {
        let _token = log.prepare_prefix_transfer().unwrap();
    }
    assert_eq!(snapshot(&log), before);
    assert_eq!(log.archived_event_count(), 0);
    log.prepare_prefix_transfer().unwrap().commit();
    assert_eq!(log.len(), 0);
    assert_eq!(log.events_with_remote().collect::<Vec<_>>(), before.0);
    assert!(matches!(
        log.prepare_prefix_transfer(),
        Err(TransferRefusal::Empty)
    ));
}

#[test]
fn noncompressible_trace_and_stream_exhaustion_are_atomic() {
    let active = new_active();
    let limit = limits::<4, 1, 2>(&active);
    let mut log = LiveSignLog::<4, 1, 2>::prepare(active, limit).unwrap();
    duplex(&mut log, 0);
    log.prepare_prefix_transfer().unwrap().commit();
    // Different node forces a new exact template; no slot is available.
    duplex_at(&mut log, 1, NodeId(9));
    let before = snapshot(&log);
    assert!(matches!(
        log.prepare_prefix_transfer(),
        Err(TransferRefusal::ArchiveCapacity)
    ));
    assert_eq!(snapshot(&log), before);

    let active = new_active();
    let limit = limits::<8, 2, 1>(&active);
    let mut narrow = LiveSignLog::<8, 2, 1>::prepare(active, limit).unwrap();
    duplex(&mut narrow, 0);
    let before = snapshot(&narrow);
    assert!(matches!(
        narrow.prepare_prefix_transfer(),
        Err(TransferRefusal::ArchiveCapacity)
    ));
    assert_eq!(snapshot(&narrow), before);
}

#[test]
fn logical_byte_backing_and_overlap_limits_remain_distinct() {
    for underfund in 0..2 {
        let active = new_active();
        let mut limit = limits::<32, 4, 2>(&active);
        if underfund == 0 {
            limit.archive_backing_bytes -= 1;
        } else {
            limit.transfer_overlap_bytes -= 1;
        }
        let refused = Log::prepare(active, limit).err().unwrap();
        assert_eq!(refused.reason, TransferRefusal::Bounds);
        assert!(refused.into_active().is_empty());
    }
    for limit_bytes in [false, true] {
        let active = new_active();
        let mut limit = limits::<32, 4, 2>(&active);
        if limit_bytes {
            limit.retained_bytes = 1;
        } else {
            limit.retained_events = 3;
        }
        let mut log = Log::prepare(active, limit).unwrap();
        duplex(&mut log, 0);
        let before = snapshot(&log);
        assert!(matches!(
            log.prepare_prefix_transfer(),
            Err(TransferRefusal::Bounds)
        ));
        assert_eq!(snapshot(&log), before);
    }
}

#[test]
fn rejected_preparation_returns_the_exact_nonempty_original_owner() {
    let mut active = new_active();
    let event = active
        .record(
            NodeId(11),
            None,
            Some(RequestId(37)),
            KernelEventKind::BackFailed,
        )
        .unwrap();
    let mut limit = limits::<32, 4, 2>(&active);
    limit.transfer_overlap_bytes -= 1;
    let refused = Log::prepare(active, limit).err().unwrap();
    assert_eq!(refused.reason, TransferRefusal::Bounds);
    let active = refused.into_active();
    assert_eq!(active.events().collect::<Vec<_>>(), [event]);
    assert_eq!(active.next_sequence, event.sequence + 1);
    assert!(active.contains_kind(KernelEventKind::BackFailed));
}

#[test]
fn transient_gap_is_preserved_in_absolute_sequence_coordinates() {
    let active = HostedSignLog::new(4, 4 * size_of::<KernelEvent>() as u32).unwrap();
    let limit = limits::<12, 3, 0>(&active);
    let mut log = LiveSignLog::<12, 3, 0>::prepare(active, limit).unwrap();
    for _ in 0..10 {
        log.record(NodeId(0), None, None, KernelEventKind::StepYielded)
            .unwrap();
    }
    let mut expected = log.events_with_remote().collect::<Vec<_>>();
    let first_gap = log.retention_gap();
    log.prepare_prefix_transfer().unwrap().commit();
    assert_eq!(log.retention_gap(), first_gap);
    for _ in 0..6 {
        log.record(NodeId(0), None, None, KernelEventKind::ValueStored)
            .unwrap();
    }
    expected.extend(log.active.events().map(|event| ArchivedSign {
        event,
        remote: None,
    }));
    let before_gap = log.retention_gap();
    log.prepare_prefix_transfer().unwrap().commit();
    assert_eq!(log.events_with_remote().collect::<Vec<_>>(), expected);
    assert_eq!(log.retention_gap(), before_gap);
    assert_eq!(
        log.retention_gap().unwrap(),
        SignRetentionGap {
            first_sequence: 0,
            last_sequence: 11,
            entries: 8
        }
    );
}

#[test]
fn original_sequence_overflow_and_active_pressure_never_reset_or_auto_rotate() {
    let mut log = new();
    duplex(&mut log, 0);
    log.prepare_prefix_transfer().unwrap().commit();
    log.active.next_sequence = u32::MAX;
    let before = snapshot(&log);
    assert_eq!(
        log.record(NodeId(0), None, None, KernelEventKind::BackCompleted),
        Err(SignError::SequenceOverflow)
    );
    assert_eq!(snapshot(&log), before);

    let mut log = new();
    duplex(&mut log, 0);
    let before = snapshot(&log);
    assert_eq!(
        log.ensure_remote_capacity(3),
        Err(SignError::RemoteItemCapacityExceeded)
    );
    assert_eq!(snapshot(&log), before);
    assert_eq!(log.archived_event_count(), 0);
}

#[test]
fn actual_duplex_scheduler_transfers_while_value_is_unacknowledged_then_runs_1264_reads() {
    use crate::scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepBack,
        StepInputBytes, StepIo, StepOutcome,
    };
    use crate::{
        CordEndpoint, FixedRoutes, FixedValueStore, RouteRange, RouteTarget, ValueStorage,
    };
    struct Echo(u32);
    impl StepBack<1> for Echo {
        fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
            let Some(value) = io.input(PortId(0)) else {
                return StepOutcome::Await;
            };
            if !io.output_ready_for(PortId(0), value.byte_len) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).unwrap();
            io.send(PortId(0), value).unwrap();
            StepOutcome::Progress
        }
        fn step_committed(&mut self) {
            self.0 += 1;
        }
    }
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::Remote(RemoteEndpointId(1)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 8,
        pressure_policy: Default::default(),
    };
    let mut kernel = FixedScheduler::<_, _, _, 1, 2, 1, 2, 1, 1>::new(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 8,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                capacity(0),
            ),
            CordSpec::remote_egress(
                CordId(1),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(1),
                capacity(1),
            ),
        ],
        routes,
        [Echo(0)],
        FixedValueStore::<3, 8>::new(24).unwrap(),
        new(),
    )
    .unwrap();
    let capture = |signs: &Log| {
        signs
            .active
            .events()
            .map(|event| ArchivedSign {
                remote: signs.active.remote_identity(event.sequence),
                event,
            })
            .collect::<Vec<_>>()
    };
    let mut expected = Vec::new();
    for sequence in 0..1264_u64 {
        kernel
            .admit_remote_input(
                RemoteEndpointId(0),
                CordId(0),
                sequence,
                &sequence.to_le_bytes(),
            )
            .unwrap();
        assert_eq!(
            kernel.step().unwrap(),
            SchedulerStatus::Progress { node: NodeId(0) }
        );
        let offer = kernel
            .remote_egress_offer(RemoteEndpointId(1), CordId(1))
            .unwrap()
            .unwrap();
        assert_eq!(
            kernel.values().get(offer.value).unwrap(),
            sequence.to_le_bytes()
        );
        if sequence == 0 {
            let before = (
                kernel.decisions(),
                kernel.logical_invocations(),
                kernel.drivers()[0].0,
                kernel.values().used_items(),
                kernel.values().used_bytes(),
                kernel.has_ready_work(),
            );
            expected.extend(capture(kernel.signs()));
            kernel.prepare_sign_prefix_transfer().unwrap().commit();
            assert_eq!(
                (
                    kernel.decisions(),
                    kernel.logical_invocations(),
                    kernel.drivers()[0].0,
                    kernel.values().used_items(),
                    kernel.values().used_bytes(),
                    kernel.has_ready_work()
                ),
                before
            );
            let retained = kernel
                .remote_egress_offer(RemoteEndpointId(1), CordId(1))
                .unwrap()
                .unwrap();
            assert_eq!(retained.value, offer.value);
            assert_eq!(retained.sequence, offer.sequence);
            assert_eq!(
                kernel.values().get(retained.value).unwrap(),
                sequence.to_le_bytes()
            );
        }
        kernel
            .remote_egress_accept(RemoteEndpointId(1), CordId(1), offer.sequence)
            .unwrap();
        kernel
            .remote_egress_delivered(RemoteEndpointId(1), CordId(1), offer.sequence)
            .unwrap();
        assert_eq!(kernel.values().used_items(), 0);
        expected.extend(capture(kernel.signs()));
        kernel.prepare_sign_prefix_transfer().unwrap().commit();
    }
    assert_eq!(kernel.drivers()[0].0, 1264);
    assert_eq!(
        kernel.signs().events_with_remote().collect::<Vec<_>>(),
        expected
    );
    for entry in &expected {
        assert_eq!(
            kernel.signs().remote_identity(entry.event.sequence),
            entry.remote
        );
    }
    std::println!("LIVE_SIGN_ARCHIVE_ROW {{\"reads\":1264,\"retained_events\":{},\"canonical_retained_bytes\":{},\"archive_backing_bytes\":{},\"wrapper_and_active_backing_bytes\":{},\"transfer_overlap_backing_bytes\":{},\"scope\":\"CPU64 actual fixed scheduler; no Source, Root admission or hardware proof; excludes caller/stack/allocator overhead\"}}",
        kernel.signs().archived_events, kernel.signs().archived_bytes,
        Log::required_archive_backing_bytes(), kernel.signs().owned_backing_bytes().unwrap(),
        Log::required_transfer_overlap_bytes(&kernel.signs().active).unwrap());
}

#[test]
fn boxed_profile_preserves_inline_signs_remote_sidecars_and_transfer_truth() {
    let mut inline = new();
    let active = new_active();
    let limits = ArchiveLimits {
        retained_events: 10_000,
        retained_bytes: 1_000_000,
        archive_backing_bytes: BoxedLiveSignLog::<32, 4, 2>::required_archive_backing_bytes(),
        transfer_overlap_bytes: BoxedLiveSignLog::<32, 4, 2>::required_transfer_overlap_bytes(
            &active,
        )
        .unwrap(),
    };
    let mut boxed = BoxedLiveSignLog::<32, 4, 2>::try_prepare(active, limits).unwrap();
    for sequence in 0..12 {
        assert_eq!(duplex(&mut inline, sequence), duplex(&mut boxed, sequence));
        assert_eq!(
            inline.prepare_prefix_transfer().unwrap().commit(),
            boxed.prepare_prefix_transfer().unwrap().commit()
        );
        assert_eq!(
            inline.events_with_remote().collect::<Vec<_>>(),
            boxed.events_with_remote().collect::<Vec<_>>()
        );
        assert_eq!(inline.retention_gap(), boxed.retention_gap());
        assert_eq!(
            inline.archived_canonical_bytes(),
            boxed.archived_canonical_bytes()
        );
    }
}
