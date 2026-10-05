//! One bounded xHCI Normal-TRB receive reservation; no class or scheduler policy.
#![cfg_attr(not(test), allow(dead_code))] // Native composition is not installed yet.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum EndpointRingRefusal {
    Geometry,
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
/// One transfer is admitted at a time; no software timeout resets the ring.
pub(crate) struct EndpointRingCursor {
    ordinary_slots: usize,
    enqueue: usize,
    cycle: u32,
    sequence: u64,
    pending: Option<(u64, usize, u32, u16)>,
}
impl EndpointRingCursor {
    pub fn new(total_slots: usize) -> Result<Self, EndpointRingRefusal> {
        if !(2..=4096).contains(&total_slots) {
            return Err(EndpointRingRefusal::Geometry);
        }
        Ok(Self {
            ordinary_slots: total_slots - 1,
            enqueue: 0,
            cycle: 1,
            sequence: 0,
            pending: None,
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
        self.pending = Some((sequence, self.enqueue, self.cycle, length));
        self.sequence = sequence;
        self.enqueue += 1;
        if self.enqueue == self.ordinary_slots {
            self.enqueue = 0;
            self.cycle ^= 1;
        }
        Ok(reservation)
    }

    pub fn ensure_idle(&self) -> Result<(), EndpointRingRefusal> {
        if self.pending.is_some() {
            Err(EndpointRingRefusal::Pending)
        } else {
            Ok(())
        }
    }

    pub fn ensure_ready(&self) -> Result<(), EndpointRingRefusal> {
        self.ensure_idle()?;
        self.sequence
            .checked_add(1)
            .ok_or(EndpointRingRefusal::Exhausted)?;
        Ok(())
    }

    pub fn ordinary_slots(&self) -> usize {
        self.ordinary_slots
    }

    /// Compute the received extent without releasing DMA. Caller separately
    /// validates the completion's device, endpoint, TRB pointer and disposition.
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
        self.pending = None;
        Ok(())
    }

    fn exact(&self, reservation: &EndpointRingReservation) -> Result<(), EndpointRingRefusal> {
        if self.pending
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
