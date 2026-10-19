//! Exact captured bytes survive consumption; fixture digests are not crypto.
use super::*;
#[test]
fn capture_requires_exact_preplay_storage_and_refuses_extent_before_effects() {
    assert_eq!(content_value_storage_bytes(4, 8), Some(32));
    assert_eq!(content_value_storage_bytes(u16::MAX, u32::MAX), None);
    assert!(log::<4>().with_value_storage(8, [0; 31]).is_err());
    assert!(log::<4>().with_value_storage(8, [0; 33]).is_err());
    let mut late = log::<4>();
    late.record(NodeId(0), None, None, KernelEventKind::StepYielded)
        .unwrap();
    assert!(late.with_value_storage(8, [0; 32]).is_err());
    let mut capture = log::<4>().with_value_storage(8, [0; 32]).unwrap();
    assert_eq!(capture.maximum_retained_value_bytes(), Some(8));
    assert_eq!(capture.value_storage_capacity_bytes(), 32);
    let (_, alloc) = allocation_probe::observe(|| {
        assert_eq!(
            capture.record_value(NodeId(0), PortId(0), CordId(0), b"123456789"),
            Err(SignError::InvalidBudget)
        );
        assert_eq!(capture.provider().0.get(), 0);
        assert_eq!(capture.inner().len(), 0);
        assert_eq!(capture.bindings().count(), 0);
        let mut input = *b"original";
        let event = capture
            .record_value(NodeId(0), PortId(0), CordId(0), &input)
            .unwrap();
        input.fill(0);
        assert_eq!(
            capture.retained_value(event.sequence),
            Some(b"original".as_slice())
        );
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
}
#[test]
fn mixed_transient_eviction_keeps_each_retained_value_and_hides_stale_slots() {
    let mut capture = log::<4>().with_value_storage(8, [0; 32]).unwrap();
    let pinned = capture
        .record(NodeId(9), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    let mut history = [None; 128];
    let (_, alloc) = allocation_probe::observe(|| {
        for i in 0..128 {
            let mut input = (0xaabbccdd00000000u64 + i as u64).to_le_bytes();
            let length = i % 9;
            let event = capture
                .record_value(NodeId(1), PortId(2), CordId(3), &input[..length])
                .unwrap();
            history[i] = Some((event.sequence, input, length));
            input.fill(0xff);
            if i % 3 == 0 {
                capture
                    .record(NodeId(1), None, None, KernelEventKind::StepYielded)
                    .unwrap();
            }
            for binding in capture.bindings() {
                let (_, original, n) = history
                    .iter()
                    .flatten()
                    .find(|(sequence, _, _)| *sequence == binding.event_sequence)
                    .unwrap();
                let value = capture.retained_value(binding.event_sequence).unwrap();
                assert_eq!(value, &original[..*n]);
                assert_eq!(binding.digest, expected(value));
                assert_eq!(binding.byte_length as usize, value.len());
            }
            assert!(capture.inner().events().any(|e| e == pinned));
            assert_eq!(capture.retained_value(pinned.sequence), None);
        }
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    assert_eq!(capture.retained_value(history[0].unwrap().0), None);
    assert_eq!(capture.provider().0.get(), 128);
    assert!(capture.retention_gap().is_some());
}
#[test]
fn pinned_pressure_never_creates_a_value_or_hash() {
    let mut capture = log::<2>().with_value_storage(8, [0; 16]).unwrap();
    for _ in 0..2 {
        capture
            .record(NodeId(0), None, None, KernelEventKind::BackCompleted)
            .unwrap();
    }
    let (_, alloc) = allocation_probe::observe(|| {
        assert_eq!(
            capture.record_value(NodeId(0), PortId(0), CordId(0), b"value"),
            Err(SignError::ItemCapacityExceeded)
        );
        assert_eq!(capture.provider().0.get(), 0);
        assert_eq!(capture.bindings().count(), 0);
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
}
#[test]
fn empty_value_capture_is_explicit_and_distinct_from_disabled_storage() {
    let mut capture = log::<2>().with_value_storage(0, []).unwrap();
    let event = capture
        .record_value(NodeId(0), PortId(0), CordId(0), b"")
        .unwrap();
    assert_eq!(capture.maximum_retained_value_bytes(), Some(0));
    assert_eq!(capture.retained_value(event.sequence), Some(b"".as_slice()));
    let mut disabled = log::<2>();
    let event = disabled
        .record_value(NodeId(0), PortId(0), CordId(0), b"")
        .unwrap();
    assert_eq!(disabled.maximum_retained_value_bytes(), None);
    assert_eq!(disabled.retained_value(event.sequence), None);
}
#[test]
fn actual_scheduler_captures_committed_bytes_after_consumption_and_refuses_phantom_outputs() {
    for cancel in [false, true] {
        let gate = Cell::new(false);
        let capture = log::<32>().with_value_storage(8, [0; 256]).unwrap();
        let mut scheduler = play_with_signs(b"actual", 2, Some(&gate), capture);
        let (_, alloc) = allocation_probe::observe(|| {
            for _ in 0..3 {
                scheduler.step().unwrap();
            }
            assert_eq!(scheduler.signs().bindings().count(), 1);
            let binding = scheduler.signs().bindings().next().unwrap();
            assert_eq!(
                scheduler.signs().retained_value(binding.event_sequence),
                Some(b"actual".as_slice())
            );
            if cancel {
                scheduler.cancel().unwrap();
            } else {
                gate.set(true);
                scheduler.run(32).unwrap();
            }
            assert_eq!(
                scheduler.signs().bindings().count(),
                if cancel { 1 } else { 2 }
            );
            assert_eq!(scheduler.values().used_items(), 0);
            for binding in scheduler.signs().bindings() {
                assert_eq!(
                    scheduler.signs().retained_value(binding.event_sequence),
                    Some(b"actual".as_slice())
                );
            }
        });
        assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    }
}
#[cfg(feature = "alloc")]
#[test]
fn hosted_capture_uses_only_prepared_storage() {
    let inner = HostedSignLog::new(4, (4 * size_of::<KernelEvent>()) as u32).unwrap();
    let mut capture = HostedContentSignLog::new(
        inner,
        FixtureDigest::default(),
        content_sign_storage_bytes(4).unwrap(),
    )
    .unwrap()
    .with_value_storage(8, vec![0; 32])
    .unwrap();
    let capacity = capture.content_allocation_capacity();
    let (_, alloc) = allocation_probe::observe(|| {
        for i in 0..128u32 {
            let bytes = i.to_le_bytes();
            let event = capture
                .record_value(NodeId(0), PortId(0), CordId(0), &bytes)
                .unwrap();
            assert_eq!(
                capture.retained_value(event.sequence),
                Some(bytes.as_slice())
            );
        }
    });
    assert_eq!((alloc.allocations, alloc.reallocations), (0, 0));
    assert_eq!(capture.content_allocation_capacity(), capacity);
    assert_eq!(capture.value_storage_capacity_bytes(), 32);
}

#[test]
fn undersized_custody_refuses_before_scheduler_construction_even_through_debug_wrapper() {
    let capture = log::<32>().with_value_storage(4, [0; 128]).unwrap();
    assert!(matches!(
        try_play_with_signs(b"actual", 1, None, capture),
        Err(SchedulerError::Sign(SignError::InvalidBudget))
    ));
    let capture = log::<32>().with_value_storage(4, [0; 128]).unwrap();
    let observed = ObservedSignSink::<_, 2, 1, 1>::detached(
        capture,
        DebugExecutionIdentity {
            body: [1; 32],
            plan: [2; 32],
            play: [3; 32],
        },
        [DebugNodeBinding { plot: 0, host: 0 }; 2],
        [[Some(0)]; 2],
    );
    assert!(matches!(
        try_play_with_signs(b"actual", 1, None, observed),
        Err(SchedulerError::Sign(SignError::InvalidBudget))
    ));
}
