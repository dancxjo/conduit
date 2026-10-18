//! Exact controller command completion and original endpoint retirement.
use super::{
    COMMAND_TRBS, DMA, DmaStorage, EVENT_TRBS, Event, XhciError, XhciReady, transfer_publication,
    write32,
};
use core::ptr::write_volatile;

impl XhciReady {
    pub(crate) fn ring_endpoint(&self, slot: u8, endpoint: u8) {
        unsafe { write32(self.doorbell + usize::from(slot) * 4, u32::from(endpoint)) }
    }

    pub(crate) fn command(&mut self, mut trb: [u32; 4]) -> Result<Event, XhciError> {
        if self.command_enqueue >= COMMAND_TRBS - 1 {
            return Err(XhciError::CommandRingFull);
        }
        trb[3] = (trb[3] & !1) | self.command_cycle;
        let index = self.command_enqueue;
        let pointer = self.dma_physical
            + core::mem::offset_of!(DmaStorage, command_ring) as u64
            + (index * 16) as u64;
        unsafe { transfer_publication::write_command_slot(index, trb) };
        self.command_enqueue += 1;
        unsafe { write32(self.doorbell, 0) };
        for _ in 0..EVENT_TRBS {
            let event = self.next_command_event()?;
            if event.event_type == 34 {
                continue;
            }
            if event.event_type != 33 || event.pointer != pointer {
                return Err(XhciError::UnexpectedCompletion);
            }
            return Ok(event);
        }
        Err(XhciError::UnexpectedCompletion)
    }

    /// Retires one removed slot while boundedly draining only its stale
    /// transfer completions. Port-change events are controller observations,
    /// not command completions or semantic input.
    pub(crate) fn disable_removed_slot(&mut self, slot: u8) -> Result<u8, XhciError> {
        if slot == 0 || self.command_enqueue >= COMMAND_TRBS - 1 {
            return Err(XhciError::CommandRingFull);
        }
        let index = self.command_enqueue;
        let pointer = self.dma_physical
            + core::mem::offset_of!(DmaStorage, command_ring) as u64
            + (index * 16) as u64;
        let command = [
            0,
            0,
            0,
            (10 << 10) | (u32::from(slot) << 24) | self.command_cycle,
        ];
        unsafe { transfer_publication::write_command_slot(index, command) };
        self.command_enqueue += 1;
        unsafe { write32(self.doorbell, 0) };
        let mut stale_transfers = 0_u8;
        for _ in 0..EVENT_TRBS {
            let event = self.next_event()?;
            match event.event_type {
                34 => continue,
                32 if event.slot == slot => {
                    stale_transfers = stale_transfers
                        .checked_add(1)
                        .ok_or(XhciError::UnexpectedCompletion)?;
                }
                33 if event.pointer == pointer
                    && event.completion_code == 1
                    && event.slot == slot =>
                {
                    unsafe {
                        write_volatile(core::ptr::addr_of_mut!(DMA.dcbaa[usize::from(slot)]), 0)
                    };
                    return Ok(stale_transfers);
                }
                _ => return Err(XhciError::UnexpectedCompletion),
            }
        }
        Err(XhciError::UnexpectedCompletion)
    }

    /// Stop one exact original endpoint without disabling its slot or EP0.
    /// Only the caller's still-retained TD may be drained before the exact
    /// command acknowledgement. This does not retire software leases, reset a
    /// halted endpoint, or authorize publishing another transfer.
    ///
    /// # Safety
    /// Caller exclusively owns the original slot/DCI and controller event ring,
    /// has closed submissions, and retains all DMA and obligations on failure.
    /// `original_td` must name the sole admitted pending TD on this queue, or be
    /// None for an idle queue. No other queue may have an outstanding transfer.
    #[allow(dead_code)] // Endpoint recovery composition is not installed yet.
    pub(crate) unsafe fn stop_original_endpoint(
        &mut self,
        slot: u8,
        endpoint: u8,
        original_td: Option<u64>,
    ) -> Result<u8, XhciError> {
        if slot == 0
            || slot > self.admitted_slots
            || !(2..=31).contains(&endpoint)
            || original_td.is_some_and(|pointer| pointer == 0 || pointer & 15 != 0)
        {
            return Err(XhciError::InvalidLayout);
        }
        if self.command_enqueue >= COMMAND_TRBS - 1 {
            return Err(XhciError::CommandRingFull);
        }
        let index = self.command_enqueue;
        let pointer = self.dma_physical
            + core::mem::offset_of!(DmaStorage, command_ring) as u64
            + (index * 16) as u64;
        let command = [
            0,
            0,
            0,
            (15 << 10) | (u32::from(endpoint) << 16) | (u32::from(slot) << 24) | self.command_cycle,
        ];
        unsafe { transfer_publication::write_command_slot(index, command) };
        self.command_enqueue += 1;
        unsafe { write32(self.doorbell, 0) };
        let mut drained = 0;
        for _ in 0..EVENT_TRBS {
            let event = self.next_event()?;
            match event.event_type {
                34 => continue,
                32 if event.slot == slot
                    && event.endpoint == endpoint
                    && Some(event.pointer) == original_td
                    && drained == 0 =>
                {
                    drained = 1;
                }
                33 if event.pointer == pointer
                    && event.slot == slot
                    && event.completion_code == 1 =>
                {
                    return Ok(drained);
                }
                _ => return Err(XhciError::UnexpectedCompletion),
            }
        }
        Err(XhciError::UnexpectedCompletion)
    }
}
