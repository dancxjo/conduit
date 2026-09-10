use super::*;
use alloc::vec::Vec;
use core::ops::Range;

#[test]
fn repeated_wraps_reuse_fixed_slots_with_independent_report_buffer_and_cycle() {
    let ring = 0x1000;
    let reports = 0x2000;
    let mut storage = [[0_u32; 4]; 64];
    let mut consumer = 0;
    let mut cycle = 1;
    for sequence in 0..1024 {
        let position = Position::at(sequence);
        assert_eq!(consumer, position.slot);
        assert_eq!(cycle, position.cycle);
        assert_ne!(
            storage[consumer][3] & 1,
            cycle,
            "old slot must not be owned"
        );
        if sequence == 0 || position.slot == TRANSFER_RING_REPORT_SLOTS - 1 {
            storage[63] = position.link(ring);
        }
        publish(position.normal(reports), |word, value| {
            storage[position.slot][word] = value
        });
        let trb = storage[consumer];
        assert_eq!(trb[3] & 1, cycle);
        assert_eq!(
            trb[0],
            reports as u32 + (sequence % REPORT_BUFFERS * BOOT_REPORT_BYTES) as u32
        );
        assert_eq!(trb[2], BOOT_REPORT_BYTES as u32);
        consumer += 1;
        if consumer == 63 {
            let link = storage[consumer];
            assert_eq!(link[0], ring as u32);
            assert_eq!(link[3] >> 10, 6);
            assert_eq!(link[3] & 1, cycle);
            assert_eq!(link[3] & 2, 2);
            consumer = 0;
            cycle ^= 1;
        }
    }
    assert_eq!(core::mem::size_of_val(&storage), 1024);
}

#[test]
fn payload_is_fully_written_before_cycle_publishes_ownership() {
    let mut writes = Vec::new();
    publish(Position::at(63).normal(0x1000), |word, value| {
        writes.push((word, value))
    });
    assert_eq!(
        writes.iter().map(|(word, _)| *word).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(writes[3].1 & 1, 0);
    assert_eq!(Position::at(62).cycle, 1);
    assert_eq!(Position::at(126).cycle, 1);
    assert!(Position::at(usize::MAX).slot < TRANSFER_RING_REPORT_SLOTS);
}

fn report_window_range(index: usize) -> Range<usize> {
    let (start, end) = report_window(index).unwrap();
    start..end
}

#[test]
fn followup_submission_keeps_a_rolling_eight_report_runway() {
    let mut outstanding = alloc::collections::BTreeSet::new();
    for report in report_window_range(0) {
        assert!(outstanding.insert(report));
    }
    assert_eq!(outstanding.len(), REPORT_BUFFERS);
    for index in 0..160 {
        assert!(outstanding.contains(&index), "index {index} must be armed");
        assert_eq!(outstanding.len(), REPORT_BUFFERS);
        outstanding.remove(&index);
        if index == 159 {
            break;
        }
        for report in report_window_range(index + 1) {
            assert!(outstanding.insert(report));
        }
    }
}

#[test]
fn report_windows_cross_multiple_refills_and_link_wrap_without_gaps() {
    let mut expected = 0usize;
    for index in 0..200 {
        for report in report_window_range(index) {
            let position = Position::at(report);
            assert_eq!(report, expected);
            assert!(position.slot < TRANSFER_RING_REPORT_SLOTS);
            expected += 1;
        }
    }
    // 200 followups implies publishing far past the 63-slot data TRB ring wrap.
    assert!(expected > TRANSFER_RING_REPORT_SLOTS * 3);
}
