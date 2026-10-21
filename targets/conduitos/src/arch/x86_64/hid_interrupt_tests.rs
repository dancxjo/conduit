use super::*;
use alloc::vec::Vec;
use core::ops::Range;

#[test]
fn repeated_wraps_keep_two_trb_td_ownership_and_fixed_dma_extent() {
    let ring = 0x1000;
    let reports = 0x2000;
    let mut storage = [[0_u32; 4]; 64];
    for sequence in 0..1024 {
        let position = Position::at(sequence);
        let slot = position.slot * 2;
        assert_ne!(storage[slot][3] & 1, position.cycle);
        assert_ne!(storage[slot + 1][3] & 1, position.cycle);
        if sequence == 0 || position.slot == TRANSFER_RING_REPORT_SLOTS - 1 {
            storage[62] = position.link(ring);
        }
        let cookie = 0x100000000 + sequence as u64;
        publish(position.event_data(cookie), |word, value| {
            storage[slot + 1][word] = value
        });
        publish(
            position.chained_normal(reports, BOOT_REPORT_BYTES),
            |word, value| storage[slot][word] = value,
        );
        assert_eq!(
            storage[slot][0],
            reports as u32 + (sequence % REPORT_BUFFERS * BOOT_REPORT_BYTES) as u32
        );
        assert_eq!(storage[slot][2], 8);
        assert_eq!(storage[slot][3], (1 << 10) | (1 << 4) | position.cycle);
        assert_eq!(
            storage[slot + 1],
            [
                cookie as u32,
                (cookie >> 32) as u32,
                0,
                (7 << 10) | (1 << 5) | position.cycle
            ]
        );
        if position.slot == TRANSFER_RING_REPORT_SLOTS - 1 {
            assert_eq!(
                storage[62],
                [ring as u32, 0, 0, (6 << 10) | 2 | position.cycle]
            );
        }
        assert_eq!(storage[63], [0; 4], "unused cell does not increase storage");
    }
    assert_eq!(core::mem::size_of_val(&storage), 1024);
}

#[test]
fn payload_is_fully_written_before_cycle_publishes_ownership() {
    let mut writes = Vec::new();
    publish(
        Position::at(31).chained_normal(0x1000, BOOT_REPORT_BYTES),
        |word, value| writes.push((word, value)),
    );
    assert_eq!(
        writes.iter().map(|(word, _)| *word).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(writes[3].1 & 1, 0);
    assert_eq!(Position::at(30).cycle, 1);
    assert_eq!(Position::at(62).cycle, 1);
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
    // 200 followups implies publishing far past the 31-TD ring wrap.
    assert!(expected > TRANSFER_RING_REPORT_SLOTS * 3);
}

#[test]
fn stale_duplicate_foreign_and_pointer_events_never_accept_a_report() {
    use crate::arch::xhci_mechanism::Event;
    use core::sync::atomic::AtomicU32;
    let serial = AtomicU32::new(0);
    let mut interests = identity::Interests::new();
    let mut accepted = 0;
    let mut previous = None;
    for index in 0..1024 {
        interests.arm(index, index + 1, 11, &serial).unwrap();
        let cookie = interests.cookie(index, 11).unwrap();
        let valid = Event {
            event_data: true,
            event_type: 32,
            completion_code: 1,
            slot: 1,
            endpoint: 3,
            residual: 8,
            pointer: cookie,
        };
        let mut accept = |event: Event| -> Result<(), HidError> {
            let expected = interests.cookie(index, 11)?;
            validate_interrupt_event(event, 1, 3, expected)?;
            interests.retire(index, expected)?;
            accepted += 1;
            Ok(())
        };
        if let Some(old) = previous {
            assert!(accept(old).is_err());
        }
        assert!(accept(Event { slot: 2, ..valid }).is_err());
        assert!(
            accept(Event {
                endpoint: 5,
                ..valid
            })
            .is_err()
        );
        assert!(
            accept(Event {
                pointer: cookie ^ (1 << 32),
                ..valid
            })
            .is_err()
        );
        assert!(
            accept(Event {
                event_data: false,
                ..valid
            })
            .is_err()
        );
        accept(valid).unwrap();
        assert!(accept(valid).is_err());
        previous = Some(valid);
        assert_eq!(accepted, index + 1);
    }
}
