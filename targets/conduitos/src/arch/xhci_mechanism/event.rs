//! One controller event consumer with bounded exact-endpoint retention.
use super::{DMA, DmaStorage, EVENT_TRBS, Event, POLL_STEPS, XhciError, XhciReady, write64};
use core::{
    hint::spin_loop,
    sync::atomic::{Ordering, fence},
};

impl XhciReady {
    /// Readiness only: no payload read, dequeue advance, or completion authority.
    /// Root uses this while interrupts are masked to close the idle race.
    #[cfg(any(test, all(target_arch = "x86_64", target_os = "none")))]
    pub(crate) fn has_observed_event(&self) -> bool {
        !self.retained_events.is_empty()
            || unsafe {
                core::ptr::read_volatile(core::ptr::addr_of!(DMA.event_ring[self.event_dequeue][3]))
                    & 1
                    == self.event_cycle
            }
    }

    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub(crate) const fn interrupt_registers(&self) -> (usize, usize) {
        (self.operational, self.runtime_interrupter)
    }

    /// Exclusive consumers drain retained events before new hardware observations.
    pub(crate) fn poll_event(&mut self) -> Option<Event> {
        self.retained_events.pop().or_else(|| {
            poll_hardware(
                &mut self.event_dequeue,
                &mut self.event_cycle,
                self.dma_physical,
                self.runtime_interrupter,
            )
        })
    }

    fn poll_matching(
        &mut self,
        matches: impl Fn(Event) -> bool,
    ) -> Result<Option<Event>, XhciError> {
        let Self {
            retained_events,
            event_dequeue,
            event_cycle,
            dma_physical,
            runtime_interrupter,
            ..
        } = self;
        retained_events.poll_with(matches, || {
            poll_hardware(
                event_dequeue,
                event_cycle,
                *dma_physical,
                *runtime_interrupter,
            )
        })
    }

    /// Select the endpoint, never a guessed TRB. Its owner validates the exact
    /// pending TRB and completion disposition before reading its report bytes.
    pub(crate) fn poll_transfer_event(
        &mut self,
        slot: u8,
        endpoint: u8,
    ) -> Result<Option<Event>, XhciError> {
        if slot == 0 || slot > self.admitted_slots || !(1..=31).contains(&endpoint) {
            return Err(XhciError::InvalidLayout);
        }
        let admitted_slots = self.admitted_slots;
        self.poll_matching(|event| transfer_interest(event, slot, endpoint, admitted_slots))
    }

    pub(crate) fn next_transfer_event(
        &mut self,
        slot: u8,
        endpoint: u8,
    ) -> Result<Event, XhciError> {
        if slot == 0 || slot > self.admitted_slots || !(1..=31).contains(&endpoint) {
            return Err(XhciError::InvalidLayout);
        }
        let admitted_slots = self.admitted_slots;
        self.next_matching(|event| transfer_interest(event, slot, endpoint, admitted_slots))
    }

    pub(super) fn next_command_event(&mut self) -> Result<Event, XhciError> {
        self.next_matching(|event| event.event_type != 32)
    }

    /// Original retirement contracts require an exclusive complete event view;
    /// a foreign TD must still refuse rather than being skipped to a later ack.
    pub(super) fn next_event(&mut self) -> Result<Event, XhciError> {
        for _ in 0..POLL_STEPS {
            if let Some(event) = self.poll_event() {
                return Ok(event);
            }
            spin_loop();
        }
        Err(XhciError::CommandTimeout)
    }

    fn next_matching(&mut self, matches: impl Fn(Event) -> bool) -> Result<Event, XhciError> {
        for _ in 0..POLL_STEPS {
            if let Some(event) = self.poll_matching(&matches)? {
                return Ok(event);
            }
            spin_loop();
        }
        Err(XhciError::CommandTimeout)
    }
}

fn transfer_interest(event: Event, slot: u8, endpoint: u8, admitted_slots: u8) -> bool {
    if event.event_type == 32 {
        (event.slot == slot && event.endpoint == endpoint)
            || event.slot == 0
            || event.slot > admitted_slots
    } else {
        // No command is concurrently pending in this endpoint poll. An
        // unexpected command ack must refuse now, never pre-ack a later command.
        // Port observations remain separate from this endpoint's transfer.
        event.event_type != 34
    }
}

fn poll_hardware(
    event_dequeue: &mut usize,
    event_cycle: &mut u32,
    dma_physical: u64,
    runtime_interrupter: usize,
) -> Option<Event> {
    let event = read_owned_event(*event_cycle, |word| unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(DMA.event_ring[*event_dequeue][word]))
    })?;
    *event_dequeue += 1;
    if *event_dequeue == EVENT_TRBS {
        *event_dequeue = 0;
        *event_cycle ^= 1;
    }
    let event_phys = dma_physical
        + core::mem::offset_of!(DmaStorage, event_ring) as u64
        + (*event_dequeue * 16) as u64;
    unsafe { write64(runtime_interrupter + 0x18, event_phys | 8) };
    Some(event)
}

/// The controller owns the payload until control's cycle matches the consumer.
/// Read ownership first, then acquire the published payload. Reading the whole
/// TRB before testing its cycle can mix an old pointer with a newly written
/// control word when the controller reuses a ring slot.
pub(super) fn read_owned_event(
    cycle: u32,
    mut read_word: impl FnMut(usize) -> u32,
) -> Option<Event> {
    let control = read_word(3);
    if control & 1 != cycle {
        return None;
    }
    // Hardware acquisition follows the observed cycle publication; the native
    // provider separately guarantees coherent controller-addressable memory.
    super::ordering::acquire_dma();
    fence(Ordering::Acquire);
    let low = read_word(0);
    let high = read_word(1);
    let status = read_word(2);
    Some(Event {
        event_type: ((control >> 10) & 0x3f) as u8,
        completion_code: (status >> 24) as u8,
        slot: (control >> 24) as u8,
        endpoint: ((control >> 16) & 0x1f) as u8,
        residual: status & 0x00ff_ffff,
        pointer: u64::from(low) | (u64::from(high) << 32),
    })
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
