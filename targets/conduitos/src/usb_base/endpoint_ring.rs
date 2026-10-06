//! Bounded xHCI Normal-TRB receive reservations; no class or scheduler policy.
#![cfg_attr(not(test), allow(dead_code))] // Native composition is not installed yet.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum EndpointRingRefusal {
    Geometry,
    Uncertain,
    Pending,
    Exhausted,
    StaleCompletion,
    Residual,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EndpointRingReservation {
    pub slot: usize,
    pub cycle: u32,
    pub length: u16,
    sequence: u64,
}

/// Producer position lives with retained DMA, independently of software possession.
/// The default profile admits one transfer; an explicit capture profile may
/// admit up to eight. No software timeout resets the ring or releases DMA.
pub(crate) struct EndpointRingCursor {
    ordinary_slots: usize,
    enqueue: usize,
    cycle: u32,
    sequence: u64,
    pending: [Option<(u64, usize, u32, u16)>; 8],
    pending_head: usize,
    pending_count: usize,
    maximum_pending: usize,
    configuration_pending: bool,
}
impl EndpointRingCursor {
    pub fn new(total_slots: usize) -> Result<Self, EndpointRingRefusal> {
        Self::with_maximum_pending(total_slots, 1)
    }

    /// Root must admit this exact capture bound and retain each corresponding
    /// DMA buffer. This geometry creates no operation authority or submissions.
    pub fn with_maximum_pending(
        total_slots: usize,
        maximum_pending: usize,
    ) -> Result<Self, EndpointRingRefusal> {
        if !(2..=4096).contains(&total_slots)
            || !(1..=8).contains(&maximum_pending)
            || maximum_pending >= total_slots
        {
            return Err(EndpointRingRefusal::Geometry);
        }
        Ok(Self {
            ordinary_slots: total_slots - 1,
            enqueue: 0,
            cycle: 1,
            sequence: 0,
            pending: [None; 8],
            pending_head: 0,
            pending_count: 0,
            maximum_pending,
            configuration_pending: false,
        })
    }

    pub fn reserve(&mut self, length: u16) -> Result<EndpointRingReservation, EndpointRingRefusal> {
        self.ensure_ready()?;
        if length == 0 || length > super::endpoint_read_contract::ENDPOINT_READ_DATA_BYTES {
            return Err(EndpointRingRefusal::Geometry);
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EndpointRingRefusal::Exhausted)?;
        let reservation = EndpointRingReservation {
            slot: self.enqueue,
            cycle: self.cycle,
            length,
            sequence,
        };
        let tail = (self.pending_head + self.pending_count) % self.pending.len();
        self.pending[tail] = Some((sequence, self.enqueue, self.cycle, length));
        self.pending_count += 1;
        self.sequence = sequence;
        self.enqueue += 1;
        if self.enqueue == self.ordinary_slots {
            self.enqueue = 0;
            self.cycle ^= 1;
        }
        Ok(reservation)
    }

    pub fn ensure_idle(&self) -> Result<(), EndpointRingRefusal> {
        if self.configuration_pending {
            Err(EndpointRingRefusal::Uncertain)
        } else if self.pending_count != 0 {
            Err(EndpointRingRefusal::Pending)
        } else {
            Ok(())
        }
    }

    pub fn ensure_ready(&self) -> Result<(), EndpointRingRefusal> {
        if self.configuration_pending {
            return Err(EndpointRingRefusal::Uncertain);
        }
        if self.pending_count == self.maximum_pending {
            return Err(EndpointRingRefusal::Pending);
        }
        self.sequence
            .checked_add(1)
            .ok_or(EndpointRingRefusal::Exhausted)?;
        Ok(())
    }

    pub fn is_fresh(&self) -> bool {
        self.sequence == 0 && self.pending_count == 0 && !self.configuration_pending
    }

    pub fn begin_configuration(&mut self) -> Result<(), EndpointRingRefusal> {
        self.ensure_idle()?;
        self.ensure_ready()?;
        if !self.is_fresh() {
            return Err(EndpointRingRefusal::Geometry);
        }
        self.configuration_pending = true;
        Ok(())
    }

    /// # Safety
    /// Native Root validated the successful Configure Endpoint completion for
    /// this exact ring. A command timeout or unrelated event is insufficient.
    pub unsafe fn complete_configuration(&mut self) -> Result<(), EndpointRingRefusal> {
        if !self.configuration_pending || self.pending_count != 0 || self.sequence != 0 {
            return Err(EndpointRingRefusal::StaleCompletion);
        }
        self.configuration_pending = false;
        Ok(())
    }

    pub fn position(&self) -> (usize, u32) {
        (self.enqueue, self.cycle)
    }

    pub fn ordinary_slots(&self) -> usize {
        self.ordinary_slots
    }

    pub fn maximum_pending(&self) -> usize {
        self.maximum_pending
    }

    pub fn pending_count(&self) -> usize {
        self.pending_count
    }

    /// Compute the received extent without releasing DMA. Caller separately
    /// validates the completion's device, endpoint, TRB pointer and disposition.
    /// Only the oldest reservation can complete, preserving endpoint order.
    pub fn actual(
        &self,
        reservation: &EndpointRingReservation,
        residual: u32,
    ) -> Result<u16, EndpointRingRefusal> {
        self.exact(reservation)?;
        let residual = u16::try_from(residual).map_err(|_| EndpointRingRefusal::Residual)?;
        reservation
            .length
            .checked_sub(residual)
            .ok_or(EndpointRingRefusal::Residual)
    }

    /// # Safety
    /// The native owner must have validated final completion of this exact TRB,
    /// slot, endpoint and attachment generation, or acknowledged their DMA stop.
    /// Cancellation, timeout, loss and descriptive matching alone do not suffice.
    pub unsafe fn complete_quiesced(
        &mut self,
        reservation: &EndpointRingReservation,
    ) -> Result<(), EndpointRingRefusal> {
        self.exact(reservation)?;
        self.pending[self.pending_head] = None;
        self.pending_head = (self.pending_head + 1) % self.pending.len();
        self.pending_count -= 1;
        Ok(())
    }

    fn exact(&self, reservation: &EndpointRingReservation) -> Result<(), EndpointRingRefusal> {
        if self.pending[self.pending_head]
            == Some((
                reservation.sequence,
                reservation.slot,
                reservation.cycle,
                reservation.length,
            ))
        {
            Ok(())
        } else {
            Err(EndpointRingRefusal::StaleCompletion)
        }
    }
}

impl EndpointRingReservation {
    /// Zero-based submission order within this retained endpoint ring. This
    /// observation cannot reconstruct a reservation or authorize completion.
    pub fn ordinal(&self) -> u64 {
        self.sequence - 1
    }

    /// Pointer arithmetic must be checked by the native DMA owner before use.
    pub fn normal(&self, buffer: u64) -> [u32; 4] {
        [
            buffer as u32,
            (buffer >> 32) as u32,
            u32::from(self.length),
            (1 << 10) | (1 << 5) | self.cycle,
        ]
    }

    /// Install the toggling Link before publishing the final ordinary TRB.
    pub fn link(&self, ring: u64) -> [u32; 4] {
        [
            ring as u32,
            (ring >> 32) as u32,
            0,
            (6 << 10) | (1 << 1) | self.cycle,
        ]
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "endpoint_ring/window_tests.rs"]
mod window_tests;
