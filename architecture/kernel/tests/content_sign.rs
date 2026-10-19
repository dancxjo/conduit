//! Content custody at the real kernel routing seam; fixture digest is not crypto.
use conduit_kernel::debug_observation::*;
use conduit_kernel::{content_sign::*, *};
use std::{cell::Cell, mem::size_of};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Default)]
struct FixtureDigest(Cell<usize>);
fn expected(bytes: &[u8]) -> [u8; 32] {
    let mut state = 2_166_136_261u32;
    for b in bytes {
        state = (state ^ u32::from(*b)).wrapping_mul(16_777_619);
    }
    let mut out = [0; 32];
    out[..4].copy_from_slice(&state.to_le_bytes());
    out
}
impl ContentDigestProvider for FixtureDigest {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        self.0.set(self.0.get() + 1);
        expected(bytes)
    }
}
fn base<const N: usize>() -> FixedSignLog<N> {
    FixedSignLog::new((N * size_of::<KernelEvent>()) as u32).unwrap()
}
fn log<const N: usize>() -> ContentSignLog<FixedSignLog<N>, FixtureDigest, N> {
    ContentSignLog::new(
        base(),
        FixtureDigest::default(),
        content_sign_storage_bytes(N as u16).unwrap(),
    )
    .unwrap()
}
#[test]
fn binding_storage_is_admitted_before_recording_and_legacy_events_stay_identical() {
    let bytes = content_sign_storage_bytes(4).unwrap();
    assert!(
        ContentSignLog::<_, _, 4>::new(base::<4>(), FixtureDigest::default(), bytes - 1).is_err()
    );
    assert!(ContentSignLog::<_, _, 3>::new(
        base::<4>(),
        FixtureDigest::default(),
        content_sign_storage_bytes(3).unwrap()
    )
    .is_err());
    let mut plain = base::<4>();
    let mut old = base::<4>();
    assert_eq!(
        plain
            .record_value(NodeId(1), PortId(2), CordId(3), b"value")
            .unwrap(),
        old.record(
            NodeId(1),
            Some(PortId(2)),
            None,
            KernelEventKind::ValueRouted
        )
        .unwrap()
    );
    assert_eq!(plain.used_bytes(), old.used_bytes());
    let log = log::<4>();
    assert_eq!(log.content_capacity_bytes(), bytes);
    assert!(log.container_bytes() >= size_of::<FixedSignLog<4>>() + bytes as usize);
}
#[test]
fn retained_bindings_follow_actual_transient_eviction_and_keep_pinned_signs() {
    let mut log = log::<4>();
    let pinned = log
        .record(NodeId(0), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    let (_, alloc) = allocation_probe::observe(|| {
        for i in 0..128u32 {
            let bytes = i.to_le_bytes();
            let event = log
                .record_value(NodeId(1), PortId(2), CordId(3), &bytes)
                .unwrap();
            let binding = log.binding(event.sequence).unwrap();
            assert_eq!(binding.cord, CordId(3));
            assert_eq!(binding.byte_length, 4);
            assert_eq!(binding.digest, expected(&bytes));
            if i % 3 == 0 {
                log.record(NodeId(1), None, None, KernelEventKind::StepYielded)
                    .unwrap();
            }
            assert!(log.inner().events().any(|e| e == pinned));
            for binding in log.bindings() {
                assert!(log
                    .inner()
                    .events()
                    .any(|e| e.sequence == binding.event_sequence
                        && e.kind == KernelEventKind::ValueRouted));
            }
            assert_eq!(
                log.bindings().count(),
                log.inner()
                    .events()
                    .filter(|e| e.kind == KernelEventKind::ValueRouted)
                    .count()
            );
        }
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    assert_eq!(log.provider().0.get(), 128);
    assert!(log.retention_gap().is_some());
    assert!(log.binding(1).is_none());
}
#[test]
fn full_retained_log_refuses_before_hash_or_event_mutation() {
    let mut log = log::<2>();
    for _ in 0..2 {
        log.record(NodeId(0), None, None, KernelEventKind::BackCompleted)
            .unwrap();
    }
    let before = log.inner().events().collect::<Vec<_>>();
    assert_eq!(
        log.record_value(NodeId(0), PortId(0), CordId(0), b"never committed"),
        Err(SignError::ItemCapacityExceeded)
    );
    assert_eq!(log.provider().0.get(), 0);
    assert_eq!(log.bindings().count(), 0);
    assert!(log.inner().events().eq(before));
}
#[test]
fn optional_debug_pressure_and_detachment_preserve_mandatory_content_bindings() {
    let execution = DebugExecutionIdentity {
        body: [1; 32],
        plan: [2; 32],
        play: [3; 32],
    };
    let mut observed = ObservedSignSink::<_, 1, 1, 1>::detached(
        log::<4>(),
        execution,
        [DebugNodeBinding { plot: 0, host: 0 }],
        [[Some(0)]],
    );
    observed
        .attach(
            DebugObservationBuffer::<1>::new(
                execution,
                1,
                size_of::<DebugObservationRecord>() as u32,
                0,
            )
            .unwrap(),
        )
        .unwrap();
    let mut plain = log::<4>();
    let (_, alloc) = allocation_probe::observe(|| {
        for i in 0..32u32 {
            let bytes = i.to_le_bytes();
            assert_eq!(observed.ensure_capacity(1), plain.ensure_capacity(1));
            let event = observed
                .record_value(NodeId(0), PortId(0), CordId(0), &bytes)
                .unwrap();
            assert_eq!(
                event,
                plain
                    .record_value(NodeId(0), PortId(0), CordId(0), &bytes)
                    .unwrap()
            );
            observed.observe_debug(DebugRuntimeEvent {
                node: NodeId(0),
                port: Some(PortId(0)),
                cord: Some(CordId(0)),
                kind: DebugEventKind::ValueSent,
                type_identity: Some(0),
                value: Some(&bytes),
                fault_code: None,
            });
            assert_eq!(
                observed.inner().binding(event.sequence),
                plain.binding(event.sequence)
            );
        }
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    let history = observed.detach().unwrap();
    assert!(history.gap().is_some());
    let event = observed
        .record_value(NodeId(0), PortId(0), CordId(0), b"detached")
        .unwrap();
    assert_eq!(
        observed.inner().binding(event.sequence).unwrap().digest,
        expected(b"detached")
    );
}

use conduit_kernel::scheduler::*;
#[derive(Clone, Copy)]
enum Driver<'a> {
    Source {
        value: ValueRef,
        remaining: u8,
        staged: bool,
    },
    Sink {
        allow: Option<&'a Cell<bool>>,
    },
}
impl StepBack<1> for Driver<'_> {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source {
                value,
                remaining,
                staged,
            } if *remaining > 0 => {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Source { .. } => StepOutcome::Complete,
            Self::Sink { allow } => {
                if allow.is_some_and(|gate| !gate.get()) {
                    io.consume_fuel(io.remaining_fuel()).unwrap();
                    return StepOutcome::Yield;
                }
                if io.input(PortId(0)).is_some() {
                    io.consume(PortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn step_committed(&mut self) {
        if let Self::Source {
            remaining, staged, ..
        } = self
        {
            if *staged {
                *remaining -= 1;
                *staged = false;
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Source { staged, .. } = self {
            *staged = false;
        }
    }
}
type Play<'a> = FixedScheduler<
    Driver<'a>,
    FixedValueStore<2, 8>,
    ContentSignLog<FixedSignLog<32>, FixtureDigest, 32>,
    2,
    1,
    1,
    2,
    2,
    1,
>;
fn play<'a>(bytes: &[u8], repeats: u8, gate: Option<&'a Cell<bool>>) -> Play<'a> {
    let mut values = FixedValueStore::<2, 8>::new(16).unwrap();
    let value = values.store(bytes).unwrap();
    // Fixture source owns one preloaded reference per future emission.
    for _ in 1..repeats {
        values.retain(value).unwrap();
    }
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    FixedScheduler::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 2,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 8,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [
            Driver::Source {
                value,
                remaining: repeats,
                staged: false,
            },
            Driver::Sink { allow: gate },
        ],
        values,
        log::<32>(),
    )
    .unwrap()
}
#[test]
fn actual_scheduler_binds_committed_output_bytes_after_value_storage_is_released() {
    for bytes in [b"42".as_slice(), b"43".as_slice()] {
        let mut scheduler = play(bytes, 1, None);
        let (_, alloc) = allocation_probe::observe(|| {
            scheduler.run(16).unwrap();
        });
        assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
        assert_eq!(scheduler.values().used_items(), 0);
        let binding = scheduler.signs().bindings().next().unwrap();
        assert_eq!(binding.digest, expected(bytes));
        assert_eq!(binding.byte_length, bytes.len() as u32);
        assert_eq!(binding.cord, CordId(0));
        assert!(scheduler
            .signs()
            .inner()
            .events()
            .any(|e| e.sequence == binding.event_sequence
                && e.node == NodeId(0)
                && e.port == Some(PortId(0))
                && e.kind == KernelEventKind::ValueRouted));
    }
}
#[test]
fn pressure_and_cancellation_never_bind_an_uncommitted_second_output() {
    for cancel in [false, true] {
        let gate = Cell::new(false);
        let mut scheduler = play(b"42", 2, Some(&gate));
        let (_, alloc) = allocation_probe::observe(|| {
            for _ in 0..3 {
                scheduler.step().unwrap();
            }
            assert_eq!(scheduler.signs().bindings().count(), 1);
            assert_eq!(scheduler.signs().provider().0.get(), 1);
            if cancel {
                scheduler.cancel().unwrap();
                assert_eq!(scheduler.signs().bindings().count(), 1);
                assert_eq!(scheduler.signs().provider().0.get(), 1);
            } else {
                gate.set(true);
                scheduler.run(32).unwrap();
                assert_eq!(scheduler.signs().bindings().count(), 2);
                assert_eq!(scheduler.signs().provider().0.get(), 2);
            }
        });
        assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
        assert_eq!(scheduler.values().used_items(), 0);
    }
}
#[cfg(feature = "alloc")]
#[test]
fn hosted_storage_matches_fixed_content_and_event_history_without_growth() {
    let inner = HostedSignLog::new(4, (4 * size_of::<KernelEvent>()) as u32).unwrap();
    let capacity = inner.allocation_capacity();
    let mut hosted = ContentSignLog::<_, _, 4>::new(
        inner,
        FixtureDigest::default(),
        content_sign_storage_bytes(4).unwrap(),
    )
    .unwrap();
    let mut fixed = log::<4>();
    let (_, alloc) = allocation_probe::observe(|| {
        for i in 0..64u32 {
            let bytes = i.to_le_bytes();
            assert_eq!(
                hosted.record_value(NodeId(0), PortId(0), CordId(0), &bytes),
                fixed.record_value(NodeId(0), PortId(0), CordId(0), &bytes)
            );
            assert!(hosted.bindings().eq(fixed.bindings()));
            assert!(hosted.inner().events().eq(fixed.inner().events()));
        }
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    assert_eq!(hosted.inner().allocation_capacity(), capacity);
}

#[test]
fn content_retention_preserves_remote_identity_inside_transient_gap_ranges() {
    let inner = FixedSignLog::<4>::new_with_remote_storage(
        (4 * size_of::<KernelEvent>()) as u32,
        1,
        remote_sign_storage_bytes(1).unwrap(),
    )
    .unwrap();
    let mut log = ContentSignLog::<_, _, 4>::new(
        inner,
        FixtureDigest::default(),
        content_sign_storage_bytes(4).unwrap(),
    )
    .unwrap();
    let remote = RemoteLifecycleIdentity {
        endpoint: RemoteEndpointId(0),
        cord: CordId(1),
        direction: RemoteCordDirection::Egress,
        sequence: 9,
    };
    let (_, alloc) = allocation_probe::observe(|| {
        let content = log
            .record_value(NodeId(0), PortId(0), CordId(0), b"content")
            .unwrap();
        let pinned = log
            .record_remote(
                NodeId(0),
                PortId(0),
                KernelEventKind::RemoteValueOffered,
                remote,
            )
            .unwrap();
        for _ in 0..16 {
            log.record(NodeId(0), None, None, KernelEventKind::StepYielded)
                .unwrap();
        }
        assert_eq!(log.remote_identity(pinned.sequence), Some(remote));
        assert!(log.binding(content.sequence).is_none());
        assert!(log.inner().events().any(|e| e == pinned));
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
}
