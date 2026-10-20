#![cfg(feature = "alloc")]
use conduit_kernel::{scheduler::*, *};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "compact_f32_dag/fixture.rs"]
mod fixture;
use fixture::*;
#[test]
fn diamond_multi_input_and_atomic_fanout_preserve_every_logical_sign() {
    for delay in [0, 100] {
        let mut canonical = prepare(true, delay, None, None);
        let mut compact = prepare(true, delay, None, None);
        let mut dispatch_only = prepare(true, delay, None, None);
        dispatch_only.bind_execution_dag(&nodes()).unwrap();
        assert_eq!(compact.compact_f32_binding_bytes(), 0);
        compact.bind_compact_f32_dag(&nodes()).unwrap();
        assert!(compact.compact_f32_binding_bytes() > 0);
        assert_eq!(dispatch_only.compact_f32_binding_bytes(), 0);
        let (_, alloc) = allocation_probe::observe(|| {
            finish(&mut canonical);
            finish(&mut dispatch_only);
            finish(&mut compact);
        });
        assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
        assert_eq!(canonical.drivers()[5].result, Some([9., 6.]));
        assert_eq!(compact.drivers()[5].result, canonical.drivers()[5].result);
        assert_eq!(
            canonical.signs().events().collect::<Vec<_>>(),
            compact.signs().events().collect::<Vec<_>>()
        );
        assert_eq!(
            canonical.logical_invocations(),
            compact.logical_invocations()
        );
        assert_eq!(
            dispatch_only.signs().events().collect::<Vec<_>>(),
            compact.signs().events().collect::<Vec<_>>()
        );
        assert_eq!(
            dispatch_only.drivers()[5].result,
            compact.drivers()[5].result
        );
        assert_eq!(dispatch_only.decisions(), compact.decisions());
        assert!(compact.decisions() < canonical.decisions());
        assert_eq!(dispatch_only.values().canonical, 5);
        assert_eq!(dispatch_only.values().compact, 0);
        assert_eq!(canonical.values().canonical, 5);
        assert_eq!(compact.values().canonical, 2);
        assert_eq!(compact.values().compact, 3);
        assert_eq!(compact.values().physical, 24);
        for node in 1..=3 {
            assert!(compact.drivers()[node].compact_staging_witness());
        }
        assert!(!compact.drivers()[4].compact_staging_witness());
        assert_eq!(compact.values().used_items(), 0);
        assert_eq!(compact.values().used_bytes(), 0);
    }
}
#[test]
fn failed_preflight_is_atomic_and_never_admits_unsupported_or_unprepared_storage() {
    let mut wrong_order = prepare(true, 0, None, None);
    assert_eq!(
        wrong_order.bind_compact_f32_dag(&[NodeId(4), NodeId(1), NodeId(2), NodeId(3)]),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(wrong_order.compact_f32_binding_bytes(), 0);
    wrong_order.bind_compact_f32_dag(&nodes()).unwrap();
    finish(&mut wrong_order);
    assert_eq!(wrong_order.values().compact, 3);
    for (prepared, unsupported) in [(false, None), (true, Some(2))] {
        let mut play = prepare(prepared, 0, None, unsupported);
        assert_eq!(
            play.bind_compact_f32_dag(&nodes()),
            Err(SchedulerError::InvalidPlan)
        );
        finish(&mut play);
        assert_eq!(play.values().compact, 0);
        assert_eq!(play.drivers()[5].result, Some([9., 6.]));
    }
}
#[test]
fn failure_frontier_and_cancellation_release_match_the_canonical_graph() {
    for failed in 1..=4 {
        let mut canonical = prepare(true, 0, Some(failed), None);
        let mut compact = prepare(true, 0, Some(failed), None);
        compact.bind_compact_f32_dag(&nodes()).unwrap();
        for play in [&mut canonical, &mut compact] {
            let mut seen = false;
            for _ in 0..100 {
                match play.step() {
                    Err(SchedulerError::BackFailed(f)) => {
                        assert_eq!(f.detail, 77);
                        seen = true;
                        break;
                    }
                    Ok(_) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(seen);
        }
        assert_eq!(
            canonical.signs().events().collect::<Vec<_>>(),
            compact.signs().events().collect::<Vec<_>>()
        );
        compact.cancel().unwrap();
        assert_eq!(compact.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(compact.values().used_items(), 0);
    }
}

#[test]
fn canonical_escape_boundaries_and_overlapping_regions_cannot_be_erased() {
    let mut partial = prepare(true, 0, None, None);
    assert_eq!(
        partial.bind_compact_f32_dag(&[NodeId(1), NodeId(2)]),
        Err(SchedulerError::InvalidPlan)
    );
    partial
        .bind_compact_f32_dag(&[NodeId(1), NodeId(2), NodeId(3)])
        .unwrap();
    assert_eq!(
        partial.bind_compact_f32_dag(&nodes()),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(
        partial.bind_execution_dag(&nodes()),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(
        partial.bind_execution_chain(&[NodeId(1), NodeId(2)]),
        Err(SchedulerError::InvalidPlan)
    );
    finish(&mut partial);
    assert_eq!(partial.values().compact, 1);
    assert_eq!(partial.values().canonical, 4);
    assert_eq!(partial.drivers()[5].result, Some([9., 6.]));
}

#[test]
fn mismatched_physical_width_or_logical_quota_refuses_before_any_binding() {
    for layout in [
        CompactF32Layout {
            lanes: 1,
            logical_bytes: 16,
        },
        CompactF32Layout {
            lanes: 2,
            logical_bytes: 8,
        },
        CompactF32Layout {
            lanes: 0,
            logical_bytes: 16,
        },
    ] {
        let mut play = prepare_with_layout(true, 0, None, None, Some(layout));
        assert_eq!(
            play.bind_compact_f32_dag(&nodes()),
            Err(SchedulerError::InvalidPlan)
        );
        finish(&mut play);
        assert_eq!(play.values().compact, 0);
        assert_eq!(play.drivers()[5].result, Some([9., 6.]));
    }
}

#[test]
fn cancellation_can_stop_between_independent_members_of_the_physical_order() {
    let mut canonical = prepare(true, 0, None, None);
    let mut compact = prepare(true, 0, None, None);
    canonical.bind_execution_dag(&nodes()).unwrap();
    compact.bind_compact_f32_dag(&nodes()).unwrap();
    for play in [&mut canonical, &mut compact] {
        assert_eq!(
            play.step_with_checkpoint(|node| node != NodeId(3)).unwrap(),
            SchedulerStatus::Progress { node: NodeId(0) }
        );
        assert_eq!(
            play.step_with_checkpoint(|node| node != NodeId(3)).unwrap(),
            SchedulerStatus::Progress { node: NodeId(2) }
        );
        assert_eq!(play.logical_invocations(), 3);
        assert!(!play.signs().events().any(|event| event.node == NodeId(3)));
        play.cancel().unwrap();
        assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
        assert_eq!(play.values().used_items(), 0);
    }
    assert_eq!(
        canonical.signs().events().collect::<Vec<_>>(),
        compact.signs().events().collect::<Vec<_>>()
    );
}

#[test]
fn an_escaping_dispatch_boundary_retains_region_membership() {
    let mut play = prepare(true, 0, None, None);
    play.bind_execution_dag(&[NodeId(1), NodeId(2)]).unwrap();
    assert_eq!(play.compact_f32_binding_bytes(), 0);
    assert_eq!(
        play.bind_execution_dag(&nodes()),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(
        play.bind_compact_f32_dag(&nodes()),
        Err(SchedulerError::InvalidPlan)
    );
    finish(&mut play);
    assert_eq!(play.drivers()[5].result, Some([9., 6.]));
    assert_eq!(play.values().compact, 0);
}

#[test]
fn owner_checkpoint_stops_each_logical_frontier_inside_a_compact_region() {
    for frontier in 1..=4 {
        let mut canonical = prepare(true, 0, None, None);
        let mut compact = prepare(true, 0, None, None);
        compact.bind_compact_f32_dag(&nodes()).unwrap();
        for play in [&mut canonical, &mut compact] {
            let mut stopped = false;
            for _ in 0..100 {
                play.step_with_checkpoint(|node| {
                    if node == NodeId(frontier) {
                        stopped = true;
                        false
                    } else {
                        true
                    }
                })
                .unwrap();
                if stopped {
                    break;
                }
            }
            assert!(stopped, "frontier {frontier} was not reached");
            // A denied node must not even stage arithmetic or publish bytes.
            for node in usize::from(frontier)..=4 {
                assert!(!play.drivers()[node].execution_witness());
            }
            assert_eq!(play.drivers()[5].result, None);
        }
        assert_eq!(
            canonical.logical_invocations(),
            compact.logical_invocations()
        );
        assert_eq!(
            canonical.signs().events().collect::<Vec<_>>(),
            compact.signs().events().collect::<Vec<_>>()
        );
        // Denial is a checkpoint, not a fabricated failure or cancellation.
        // Resuming must still consume the retained inputs exactly once.
        finish(&mut canonical);
        finish(&mut compact);
        assert_eq!(compact.drivers()[5].result, Some([9., 6.]));
        assert_eq!(canonical.drivers()[5].result, compact.drivers()[5].result);
        assert_eq!(
            canonical.signs().events().collect::<Vec<_>>(),
            compact.signs().events().collect::<Vec<_>>()
        );
    }
}

#[test]
fn explicit_external_canonical_input_preserves_exact_transactions_and_zero_play_allocations() {
    let mut canonical = prepare_canonical_input(1, PortId(0));
    let mut compact = prepare_canonical_input(1, PortId(0));
    assert_eq!(
        compact.bind_compact_f32_dag(&nodes()),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(compact.compact_f32_binding_bytes(), 0);
    compact
        .bind_compact_f32_dag_with_canonical_inputs(&nodes(), &[(NodeId(1), PortId(0))])
        .unwrap();
    let (_, alloc) = allocation_probe::observe(|| {
        finish(&mut canonical);
        finish(&mut compact);
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    assert_eq!(compact.drivers()[5].result, Some([9., 6.]));
    assert_eq!(compact.drivers()[5].result, canonical.drivers()[5].result);
    assert_eq!(
        compact.signs().events().collect::<Vec<_>>(),
        canonical.signs().events().collect::<Vec<_>>()
    );
    assert_eq!(
        compact.logical_invocations(),
        canonical.logical_invocations()
    );
    assert!(compact.decisions() < canonical.decisions());
    assert_eq!(
        (
            compact.values().canonical,
            compact.values().compact,
            compact.values().physical
        ),
        (2, 3, 24)
    );
    assert_eq!(compact.values().used_items(), 0);
}

#[test]
fn canonical_input_declarations_refuse_conflicts_internal_edges_and_invalid_ports_atomically() {
    let requests = [
        vec![(NodeId(1), PortId(0)), (NodeId(1), PortId(0))],
        vec![(NodeId(1), PortId(1))],
        vec![(NodeId(1), PortId(2))],
        vec![(NodeId(5), PortId(0))],
        vec![(NodeId(6), PortId(0))],
    ];
    for request in requests {
        let mut play = prepare_canonical_input(1, PortId(0));
        assert_eq!(
            play.bind_compact_f32_dag_with_canonical_inputs(&nodes(), &request),
            Err(SchedulerError::InvalidPlan)
        );
        assert_eq!(play.compact_f32_binding_bytes(), 0);
        finish(&mut play);
        assert_eq!(play.values().compact, 0);
        assert_eq!(play.drivers()[5].result, Some([9., 6.]));
    }
    for (mut play, declaration) in [
        (prepare(true, 0, None, None), (NodeId(1), PortId(0))),
        (
            prepare_canonical_input(2, PortId(0)),
            (NodeId(2), PortId(0)),
        ),
    ] {
        assert_eq!(
            play.bind_compact_f32_dag_with_canonical_inputs(&nodes(), &[declaration]),
            Err(SchedulerError::InvalidPlan)
        );
        assert_eq!(play.compact_f32_binding_bytes(), 0);
        finish(&mut play);
        assert_eq!(play.values().compact, 0);
        assert_eq!(play.drivers()[5].result, Some([9., 6.]));
    }
}

#[path = "compact_f32_dag/owned.rs"]
mod owned;
