use super::*;
use alloc::vec::Vec;

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
