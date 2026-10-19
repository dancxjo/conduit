//! Log/export contract; native bytes are fixtures, not a simulated physical run.
use super::*;
use conduit_core::{bind_active_play, PortDirection};
use conduit_kernel::{HostedSignLog, KernelEvent, SignSink};
use conduit_plan_lowering::lowering::{lower_plan_fragment_from_plan, KernelExecutionIdentityMap};

#[test]
fn native_eviction_exports_a_distinct_gap_even_without_any_retained_route() {
    let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|f| f.host_id == exact.source_advertisement.host_id)
        .unwrap();
    let (lowered, _) = lower_plan_fragment_from_plan(&exact.plan, &fragment.fragment_id).unwrap();
    // Exercise optional policy admission independently of the sealed fixture Plan.
    let mut policy = fragment.clone();
    policy.placements[0]
        .realization_properties
        .push(conduit_std_offers::routed_content_sign_requirement().unwrap());
    let base = HostedSignLog::new(4, (4 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    let mut log = StdSignLog::prepare(base, &[&policy], 32).unwrap();
    let connection = fragment
        .connections
        .iter()
        .find(|c| {
            lowered
                .identity
                .node_for_placement(&c.source_placement_id)
                .is_some()
        })
        .unwrap();
    let node = lowered
        .identity
        .node_for_placement(&connection.source_placement_id)
        .unwrap();
    let port = lowered
        .identity
        .port_for_identity(node, PortDirection::Output, &connection.source_port_id)
        .unwrap();
    let cord = lowered
        .identity
        .cord_for_connection(&connection.connection_id)
        .unwrap();
    let guard = crate::allocation_probe::begin();
    for _ in 0..128 {
        log.record(node, None, None, KernelEventKind::StepFuelGranted)
            .unwrap();
    }
    let routed = log.record_value(node, port, cord, b"kept").unwrap();
    assert_eq!(guard.finish(), 0);
    let gap = log.retention_gap().unwrap();
    assert!(gap.entries > 0 && gap.last_sequence < routed.sequence);
    assert_eq!(identity_capacity(&log).unwrap(), 5);
    let play = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 9);
    let mut identities =
        KernelExecutionIdentityMap::new(&lowered.identity, &play, 0, 0, 5).unwrap();
    let mut sequence = 1000;
    let mut context = ObservationContext {
        advertisement: &exact.source_advertisement,
        fragment,
        active_play: &play,
        lowered: &lowered.identity,
        execution_identity: &mut identities,
        next_sign_sequence: &mut sequence,
    };
    let observations = project(&log, &mut context).unwrap();
    assert_eq!(observations.len(), 2);
    assert!(
        matches!(observations[0].kind, ObservationKind::ExecutionEventGap {
        first_sequence, last_sequence, entries
    } if first_sequence == gap.first_sequence && last_sequence == gap.last_sequence && entries == gap.entries)
    );
    assert!(observations[0].placement_id.is_none() && observations[0].connection_id.is_none());
    assert_eq!(
        observations[1].connection_id.as_ref(),
        Some(&connection.connection_id)
    );
    for sign in &observations {
        let identity = context
            .execution_identity
            .sign_identity(&sign.sign_id)
            .unwrap();
        assert!(identity.sequence >= 1000);
        assert_eq!(
            bind_sign(
                &sign.host_id,
                &sign.boot_id,
                sign.active_play_id.as_ref(),
                identity.sequence
            )
            .sign_id,
            sign.sign_id
        );
    }
    for _ in 0..4 {
        log.record(node, None, None, KernelEventKind::StepFuelGranted)
            .unwrap();
    }
    assert_eq!(log.bindings().count(), 0);
    let only_gap = project(&log, &mut context).unwrap();
    assert_eq!(only_gap.len(), 1);
    assert!(matches!(
        only_gap[0].kind,
        ObservationKind::ExecutionEventGap { .. }
    ));
}
