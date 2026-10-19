#![cfg(feature = "alloc")]
//! Native transport preserves ordinary logical Steps while avoiding byte stores.
use conduit_kernel::{scheduler::*, *};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "native_region/fixture.rs"]
mod fixture;
use fixture::*;
#[test]
fn pressure_outputs_all_signs_and_zero_allocations_match_without_canonical_intermediates() {
    for count in [0, 1, 4] {
        for delay in [0, 100] {
            let mut reference = prepare(false, count, delay, None);
            let mut native = prepare(true, count, delay, None);
            let (_, allocation) = allocation_probe::observe(|| {
                finish(&mut reference);
                finish(&mut native);
            });
            assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
            assert_eq!(
                reference.drivers()[N - 1].result(),
                native.drivers()[N - 1].result()
            );
            assert_eq!(
                reference.signs().events().collect::<Vec<_>>(),
                native.signs().events().collect::<Vec<_>>()
            );
            assert_eq!(
                reference.logical_invocations(),
                native.logical_invocations()
            );
            assert_eq!(reference.values().canonical().stores, count as u64 * 33);
            assert_eq!(native.values().canonical().stores, count as u64 * 2);
            assert_eq!(native.values().native_used_items(), 0);
            assert_eq!(native.values().used_items(), 0);
        }
    }
}
#[test]
fn every_failure_retains_exact_source_frontier_and_cancel_releases_native_ownership() {
    for node in 1..=32 {
        let mut reference = prepare(false, 1, 0, Some(node));
        let mut native = prepare(true, 1, 0, Some(node));
        for play in [&mut reference, &mut native] {
            let mut failed = false;
            for _ in 0..10000 {
                match play.step() {
                    Err(SchedulerError::BackFailed(f)) => {
                        assert_eq!(f.detail, 77);
                        failed = true;
                        break;
                    }
                    Ok(_) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(failed);
        }
        assert_eq!(
            reference.signs().events().collect::<Vec<_>>(),
            native.signs().events().collect::<Vec<_>>()
        );
        native.cancel().unwrap();
        assert_eq!(native.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(native.values().used_items(), 0);
    }
}
#[test]
fn native_binding_refuses_insufficient_reservation_before_changing_dispatch() {
    let mut play = prepare_unbound(1, 0, None, 8, evidence());
    let chain: Vec<_> = (1..=32).map(NodeId).collect();
    assert_eq!(
        play.bind_native_u64_chain(&chain),
        Err(SchedulerError::InvalidPlan)
    );
    play.bind_execution_chain(&chain).unwrap();
    finish(&mut play);
    assert_eq!(play.drivers()[N - 1].result().0[0], 32);
}
#[test]
fn debugger_boundaries_refuse_native_binding_or_require_an_unfused_reprepare() {
    use conduit_kernel::debug_observation::*;
    let execution = DebugExecutionIdentity {
        body: [1; 32],
        plan: [2; 32],
        play: [3; 32],
    };
    for attached in [false, true] {
        let mut signs = ObservedSignSink::<_, N, 1, 8>::detached(
            evidence(),
            execution,
            [DebugNodeBinding { host: 1, plot: 1 }; N],
            [[Some(1)]; N],
        );
        let history = || {
            DebugObservationBuffer::<8>::new(
                execution,
                8,
                (8 * core::mem::size_of::<DebugObservationRecord>()) as u32,
                0,
            )
            .unwrap()
        };
        if attached {
            signs.attach(history()).unwrap();
        }
        let mut play = prepare_unbound(1, 0, None, 256, signs);
        let chain: Vec<_> = (1..=32).map(NodeId).collect();
        if attached {
            assert_eq!(
                play.bind_native_u64_chain(&chain),
                Err(SchedulerError::InvalidPlan)
            );
            play.bind_execution_chain(&chain).unwrap();
        } else {
            play.bind_native_u64_chain(&chain).unwrap();
            assert_eq!(
                play.attach_debug_observer(history()),
                Err(DebugObservationRefusal::UnsupportedPhysicalProfile)
            );
            assert_eq!(
                play.request_debug_breakpoint(DebugBreakpoint {
                    schema_version: DEBUG_CONTROL_SCHEMA_VERSION,
                    execution,
                    subject: DebugSubject::Gear(NodeId(1)),
                    kind: DebugBreakpointKind::BeforeGearStart
                }),
                Err(DebugControlRefusal::UnsupportedPhysicalProfile)
            );
        }
        finish(&mut play);
        assert_eq!(play.drivers()[N - 1].result().0[0], 32);
    }
}

#[test]
fn exhausted_sign_capacity_releases_staged_native_results_without_partial_commit() {
    for capacity in 3..=32 {
        let log = || {
            HostedSignLog::new(
                capacity,
                u32::from(capacity) * core::mem::size_of::<KernelEvent>() as u32,
            )
            .unwrap()
        };
        let count = if capacity.is_multiple_of(2) { 4 } else { 1 };
        let delay = if count == 4 { 100 } else { 0 };
        let mut reference = prepare_unbound(count, delay, None, 256, log());
        let mut native = prepare_unbound(count, delay, None, 256, log());
        let chain: Vec<_> = (1..=32).map(NodeId).collect();
        reference.bind_execution_chain(&chain).unwrap();
        native.bind_native_u64_chain(&chain).unwrap();
        for play in [&mut reference, &mut native] {
            let mut refused = false;
            for _ in 0..1000 {
                match play.step() {
                    Err(SchedulerError::Sign(_)) => {
                        refused = true;
                        break;
                    }
                    Ok(_) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(refused);
            // Transient evidence can be evicted, so refusal may occur after a
            // payload drained. Compare exact live ownership and accepted output
            // to the reference instead of assuming a particular early frontier.
            assert!(usize::from(play.values().used_items()) <= count);
            assert!(play.values().native_used_items() <= play.values().used_items());
        }
        assert_eq!(
            reference.values().used_items(),
            native.values().used_items()
        );
        assert_eq!(
            reference.drivers()[N - 1].result(),
            native.drivers()[N - 1].result()
        );
        assert_eq!(
            reference.signs().retention_gap(),
            native.signs().retention_gap()
        );
        assert_eq!(
            reference.signs().events().collect::<Vec<_>>(),
            native.signs().events().collect::<Vec<_>>()
        );
    }
}
#[test]
fn invalid_overlapping_and_late_native_bindings_refuse_without_changing_the_chain() {
    let mut play = prepare_unbound(1, 0, None, 256, evidence());
    for chain in [
        vec![NodeId(1)],
        vec![NodeId(1), NodeId(1)],
        vec![NodeId(1), NodeId(3)],
        vec![NodeId(2), NodeId(1)],
        vec![NodeId(1), NodeId(99)],
        vec![NodeId(0), NodeId(1)],
    ] {
        assert_eq!(
            play.bind_native_u64_chain(&chain),
            Err(SchedulerError::InvalidPlan)
        );
    }
    play.bind_native_u64_chain(&[NodeId(1), NodeId(2)]).unwrap();
    assert_eq!(
        play.bind_native_u64_chain(&[NodeId(2), NodeId(3)]),
        Err(SchedulerError::InvalidPlan)
    );
    play.step().unwrap();
    assert_eq!(
        play.bind_native_u64_chain(&[NodeId(3), NodeId(4)]),
        Err(SchedulerError::InvalidPlan)
    );
    finish(&mut play);
}
#[test]
fn native_regions_keep_every_logical_cancellation_checkpoint() {
    for node in 1..=32 {
        let mut play = prepare(true, 1, 0, None);
        let mut stopped = false;
        for _ in 0..100 {
            play.step_with_checkpoint(|candidate| {
                assert!(!stopped);
                stopped = candidate == NodeId(node);
                !stopped
            })
            .unwrap();
            if stopped {
                break;
            }
        }
        assert!(stopped);
        assert!(!play
            .signs()
            .events()
            .any(|e| e.node == NodeId(node) && e.kind == KernelEventKind::StepFuelGranted));
        play.cancel().unwrap();
        assert_eq!(play.values().used_items(), 0);
    }
}
