use super::*;

#[test]
fn owned_diamond_preserves_pressure_fanout_events_and_zero_play_allocations() {
    for delay in [0, 100] {
        let mut copied = prepare(true, delay, None, None);
        let mut owned = prepare_owned(delay, None);
        copied.bind_compact_f32_dag(&nodes()).unwrap();
        owned
            .bind_owned_compact_f32_dag(&nodes(), &[], &[])
            .unwrap();
        let (_, allocation) = allocation_probe::observe(|| {
            finish(&mut copied);
            finish(&mut owned);
        });
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        assert_eq!(owned.drivers()[5].result, Some([9., 6.]));
        assert_eq!(owned.drivers()[5].result, copied.drivers()[5].result);
        assert_eq!(
            owned.signs().events().collect::<Vec<_>>(),
            copied.signs().events().collect::<Vec<_>>()
        );
        assert_eq!(owned.values().owned, 3);
        assert_eq!(owned.values().owned_bytes, 24);
        assert_eq!(owned.values().compact, 0);
        assert_eq!(owned.values().used_items(), 0);
        assert_eq!(owned.values().used_bytes(), 0);
    }
}

#[test]
fn failed_fanout_acceptance_restores_the_exact_pending_producer_buffer() {
    let mut play = prepare_owned(0, None);
    play.bind_owned_compact_f32_dag(&nodes(), &[], &[]).unwrap();
    let original = play.drivers()[1].owned_output().unwrap().as_ptr();
    play.values().deny_compact_refs.set(true);
    assert!(matches!(
        play.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(0) })
    ));
    assert_eq!(
        play.step(),
        Err(SchedulerError::Storage(
            StorageError::RepresentationMismatch
        ))
    );
    assert_eq!(play.values().owned, 1);
    assert_eq!(play.values().restored, 1);
    let restored = play.drivers()[1].owned_output().unwrap();
    assert_eq!(restored.as_ptr(), original);
    assert_eq!(
        restored,
        [2.0f32.to_le_bytes(), 1.0f32.to_le_bytes()].concat()
    );
    assert_eq!(play.values().used_items(), 1); // Original input is still unconsumed.
    assert!(!play
        .signs()
        .events()
        .any(|e| e.node == NodeId(1) && e.kind == KernelEventKind::ValueRouted));
    assert!(play.drivers()[5].result.is_none());
    play.values().deny_compact_refs.set(false);
    play.cancel().unwrap();
    assert_eq!(play.values().used_items(), 0);
}

#[test]
fn selected_owner_buffer_denial_refuses_without_copying_fallback() {
    let mut play = prepare_owned(0, None);
    play.bind_owned_compact_f32_dag(&nodes(), &[], &[]).unwrap();
    play.drivers()[1].deny_owner_buffer.set(true);
    play.step().unwrap();
    assert_eq!(play.step(), Err(SchedulerError::InvalidPlan));
    assert_eq!(play.values().owned, 0);
    assert_eq!(play.values().compact, 0);
    assert_eq!(play.values().used_items(), 1);
    assert!(play.drivers()[5].result.is_none());
    play.cancel().unwrap();
    assert_eq!(play.values().used_items(), 0);
}

#[test]
fn cancellation_at_every_logical_frontier_releases_owned_and_canonical_values() {
    for frontier in 0..=5 {
        let mut copied = prepare(true, 0, None, None);
        let mut owned = prepare_owned(0, None);
        copied.bind_compact_f32_dag(&nodes()).unwrap();
        owned
            .bind_owned_compact_f32_dag(&nodes(), &[], &[])
            .unwrap();
        for play in [&mut copied, &mut owned] {
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
            assert!(stopped, "frontier {frontier}");
            assert!(play.drivers()[5].result.is_none());
            play.cancel().unwrap();
            assert_eq!(play.values().used_items(), 0);
            assert_eq!(play.values().used_bytes(), 0);
            let publications = play.values().owned;
            assert_eq!(play.step().unwrap(), SchedulerStatus::Cancelled);
            assert_eq!(play.values().owned, publications);
        }
        assert_eq!(
            copied.signs().events().collect::<Vec<_>>(),
            owned.signs().events().collect::<Vec<_>>()
        );
    }
}

#[test]
fn sign_admission_failure_after_transfer_restores_the_pending_output() {
    let mut play = prepare_owned(0, None);
    play.bind_owned_compact_f32_dag(&nodes(), &[], &[]).unwrap();
    let original = play.drivers()[1].owned_output().unwrap().as_ptr();
    play.values().fail_signs_after_transfer.set(true);
    play.step().unwrap();
    assert_eq!(
        play.step(),
        Err(SchedulerError::Sign(SignError::ItemCapacityExceeded))
    );
    assert_eq!(play.values().owned, 1);
    assert_eq!(play.values().restored, 1);
    assert_eq!(play.drivers()[1].owned_output().unwrap().as_ptr(), original);
    assert_eq!(play.values().used_items(), 1);
    assert!(play.drivers()[5].result.is_none());
    play.values().signs_refusal.set(false);
    play.values().fail_signs_after_transfer.set(false);
    play.cancel().unwrap();
    assert_eq!(play.values().used_items(), 0);
}
