//! Fixed control-ring reservations. No controller, protocol or scheduling policy.
#![cfg_attr(not(target_arch = "x86_64"), allow(dead_code))] // native binding is x86_64 today

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RingRefusal {
    Geometry,
    Pending,
    Uncertain,
    Exhausted,
    StaleCompletion,
}

/// One private native reservation, retained until exact hardware quiescence.
#[derive(Debug)]
pub(crate) struct RingReservation {
    pub start: usize,
    pub count: usize,
    pub cycle: u32,
    pub link: Option<(usize, u32)>,
    sequence: u64,
}

/// DMA-slot-owned producer truth. A failed transfer cannot reset this cursor.
#[derive(Debug)]
pub(crate) struct ControlRingCursor {
    enqueue: usize,
    cycle: u32,
    sequence: u64,
    pending: Option<u64>,
    uncertain: bool,
}
impl ControlRingCursor {
    pub const fn new() -> Self {
        Self {
            enqueue: 0,
            cycle: 1,
            sequence: 0,
            pending: None,
            uncertain: false,
        }
    }

    #[cfg(any(test, feature = "scripted-keyboard-proof"))]
    pub fn position(&self) -> (usize, u32) {
        (self.enqueue, self.cycle)
    }

    /// Leave room for a Link TRB; every complete control TD stays contiguous.
    /// The native publisher writes the complete new TD before exposing a link.
    pub fn reserve(
        &mut self,
        capacity: usize,
        count: usize,
    ) -> Result<RingReservation, RingRefusal> {
        self.ensure_idle()?;
        if capacity < 4 || !(2..=3).contains(&count) || self.enqueue >= capacity {
            return Err(RingRefusal::Geometry);
        }
        let next = self.sequence.checked_add(1).ok_or(RingRefusal::Exhausted)?;
        let link = if count > capacity - 1 - self.enqueue {
            let link = Some((self.enqueue, self.cycle));
            self.enqueue = 0;
            self.cycle ^= 1;
            link
        } else {
            None
        };
        let reservation = RingReservation {
            start: self.enqueue,
            count,
            cycle: self.cycle,
            link,
            sequence: next,
        };
        self.enqueue += count;
        self.sequence = next;
        self.pending = Some(next);
        Ok(reservation)
    }

    /// A new selected owner cannot inherit an unacknowledged physical operation.
    pub fn ensure_idle(&self) -> Result<(), RingRefusal> {
        if self.uncertain {
            Err(RingRefusal::Uncertain)
        } else if self.pending.is_some() {
            Err(RingRefusal::Pending)
        } else {
            Ok(())
        }
    }

    /// # Safety
    /// The native owner must have observed final status completion or an
    /// acknowledged stop for this exact reservation. Timeout/loss is insufficient.
    pub unsafe fn complete_quiesced(
        &mut self,
        reservation: &RingReservation,
    ) -> Result<(), RingRefusal> {
        if self.uncertain || self.pending != Some(reservation.sequence) {
            return Err(RingRefusal::StaleCompletion);
        }
        self.pending = None;
        Ok(())
    }

    /// Preserve pending ownership after any unacknowledged outcome. There is
    /// deliberately no software reset; native teardown must first stop DMA.
    pub fn retain_uncertain(&mut self) {
        self.uncertain = true;
    }
}

#[cfg(test)]
mod tests;
