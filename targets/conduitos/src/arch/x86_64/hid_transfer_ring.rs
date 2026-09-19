//! Shared fixed xHCI interrupt-transfer ring positions and publication order.

use core::sync::atomic::{Ordering, fence};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TransferPosition {
    pub slot: usize,
    pub buffer: usize,
    pub cycle: u32,
}

impl TransferPosition {
    pub(super) fn at(sequence: usize, report_slots: usize, report_buffers: usize) -> Self {
        debug_assert!(report_slots > 0 && report_buffers > 0);
        Self {
            slot: sequence % report_slots,
            buffer: sequence % report_buffers,
            cycle: 1 ^ ((sequence / report_slots) & 1) as u32,
        }
    }

    pub(super) fn normal(self, reports: u64, report_bytes: usize) -> [u32; 4] {
        let address = reports + (self.buffer * report_bytes) as u64;
        [
            address as u32,
            (address >> 32) as u32,
            report_bytes as u32,
            (1 << 10) | (1 << 5) | self.cycle,
        ]
    }

    pub(super) fn link(self, ring: u64) -> [u32; 4] {
        // xHCI 1.2b sections 4.9.2 and 6.4.4.1: the Link toggles
        // the controller cycle at wrap while retaining fixed physical storage.
        [
            ring as u32,
            (ring >> 32) as u32,
            0,
            (6 << 10) | (1 << 1) | self.cycle,
        ]
    }
}

/// Publish payload before granting controller ownership through the cycle bit.
pub(super) fn publish(trb: [u32; 4], mut write: impl FnMut(usize, u32)) {
    for (word, value) in trb[..3].iter().copied().enumerate() {
        write(word, value);
    }
    fence(Ordering::Release);
    write(3, trb[3]);
}
