use super::*;
#[test]
fn lease_never_overrides_pending_host_fact_or_unavailable_hardware() {
    assert!(can_acquire(false, true));
    assert!(!can_acquire(true, true));
    assert!(!can_acquire(false, false));
    assert!(!can_acquire(true, false));
}
#[test]
fn actual_interrupt_count_expires_exactly_without_wrap_or_refill() {
    for quantum in [1, 2, 16] {
        let mut count = 0;
        for n in 1..=quantum {
            let (next, expired) = count_interrupt(count, quantum).unwrap();
            assert_eq!(next, n);
            assert_eq!(expired, n == quantum);
            count = next;
        }
        assert_eq!(count_interrupt(count, quantum), None);
    }
    assert_eq!(count_interrupt(0, 0), None);
    assert_eq!(count_interrupt(u32::MAX, u32::MAX), None);
    assert_eq!(
        count_interrupt(u32::MAX - 1, u32::MAX),
        Some((u32::MAX, true))
    );
}
