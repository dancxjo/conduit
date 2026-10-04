use super::*;

#[test]
fn fixed_ring_reuses_both_cycles_under_sustained_mixed_control_geometry() {
    let mut cursor = ControlRingCursor::new();
    let mut producer = 0;
    let mut cycle = 1;
    let mut wraps = 0;
    for n in 0..100_000 {
        let count = if n % 3 == 0 { 2 } else { 3 };
        let reservation = cursor.reserve(32, count).unwrap();
        if let Some((index, old_cycle)) = reservation.link {
            assert_eq!((index, old_cycle), (producer, cycle));
            assert!(index < 32);
            producer = 0;
            cycle ^= 1;
            wraps += 1;
        }
        assert_eq!((reservation.start, reservation.cycle), (producer, cycle));
        assert_eq!(reservation.count, count);
        assert!(reservation.start + count < 32);
        assert!(matches!(cursor.reserve(32, 2), Err(RingRefusal::Pending)));
        // SAFETY: this is a deterministic owner model without hardware.
        unsafe { cursor.complete_quiesced(&reservation) }.unwrap();
        assert_eq!(
            unsafe { cursor.complete_quiesced(&reservation) },
            Err(RingRefusal::StaleCompletion)
        );
        producer += count;
    }
    assert!(wraps > 8000);
}

#[test]
fn geometry_and_sequence_exhaustion_refuse_without_consuming_storage() {
    let mut cursor = ControlRingCursor::new();
    for (capacity, count) in [(0, 2), (3, 2), (32, 0), (32, 1), (32, 4)] {
        assert!(matches!(
            cursor.reserve(capacity, count),
            Err(RingRefusal::Geometry)
        ));
        assert_eq!(cursor.sequence, 0);
        assert_eq!(cursor.enqueue, 0);
    }
    cursor.sequence = u64::MAX;
    assert!(matches!(cursor.reserve(32, 2), Err(RingRefusal::Exhausted)));
    assert_eq!(cursor.enqueue, 0);
    assert!(cursor.pending.is_none());
}

#[test]
fn uncertain_completion_retains_pending_storage_and_forbids_reuse() {
    let mut cursor = ControlRingCursor::new();
    let reservation = cursor.reserve(32, 3).unwrap();
    cursor.retain_uncertain();
    assert!(matches!(cursor.reserve(32, 2), Err(RingRefusal::Uncertain)));
    assert_eq!(
        unsafe { cursor.complete_quiesced(&reservation) },
        Err(RingRefusal::StaleCompletion)
    );
    assert_eq!(cursor.pending, Some(reservation.sequence));
}

#[test]
fn old_reservation_cannot_acknowledge_the_next_operation() {
    let mut cursor = ControlRingCursor::new();
    let old = cursor.reserve(32, 2).unwrap();
    unsafe { cursor.complete_quiesced(&old) }.unwrap();
    let current = cursor.reserve(32, 3).unwrap();
    assert_eq!(
        unsafe { cursor.complete_quiesced(&old) },
        Err(RingRefusal::StaleCompletion)
    );
    assert!(matches!(cursor.reserve(32, 2), Err(RingRefusal::Pending)));
    unsafe { cursor.complete_quiesced(&current) }.unwrap();
}
