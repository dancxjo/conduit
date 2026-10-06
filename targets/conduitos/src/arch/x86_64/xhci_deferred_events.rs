//! Retain controller observations consumed while waiting for an exact command ACK.
//! This queue neither interprets class reports nor retries commands.
use super::{EVENT_TRBS, Event, XhciError};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct DeferredEvents {
    slots: [Option<Event>; EVENT_TRBS],
    head: usize,
    len: usize,
}

impl DeferredEvents {
    pub(super) const fn new() -> Self {
        Self {
            slots: [None; EVENT_TRBS],
            head: 0,
            len: 0,
        }
    }

    pub(super) fn ensure_room(&self) -> Result<(), XhciError> {
        if self.len == EVENT_TRBS {
            Err(XhciError::EventPressure)
        } else {
            Ok(())
        }
    }

    pub(super) fn retain(&mut self, event: Event) -> Result<(), XhciError> {
        self.ensure_room()?;
        self.slots[(self.head + self.len) % EVENT_TRBS] = Some(event);
        self.len += 1;
        Ok(())
    }

    /// Only an acknowledged Slot disable permits discarding its stale transfers.
    pub(super) fn retire_slot(&mut self, slot: u8) -> u8 {
        let mut retired = 0;
        let count = self.len;
        for _ in 0..count {
            let event = self.pop().expect("retained count");
            if event.event_type == 32 && event.slot == slot {
                retired += 1;
            } else {
                // A pop has reserved the space for this unchanged observation.
                self.slots[(self.head + self.len) % EVENT_TRBS] = Some(event);
                self.len += 1;
            }
        }
        retired
    }

    pub(super) fn pop(&mut self) -> Option<Event> {
        if self.len == 0 {
            return None;
        }
        let event = self.slots[self.head].take();
        self.head = (self.head + 1) % EVENT_TRBS;
        self.len -= 1;
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(pointer: u64) -> Event {
        Event {
            event_type: if pointer.is_multiple_of(2) { 32 } else { 34 },
            completion_code: 1,
            slot: 2,
            endpoint: 3,
            residual: 7,
            pointer,
        }
    }

    #[test]
    fn acknowledged_retirement_discards_only_the_exact_slots_transfers() {
        let mut queue = DeferredEvents::new();
        let retired = event(0);
        let port_change = event(1);
        let mut foreign = event(2);
        foreign.slot = 3;
        for item in [retired, port_change, foreign, retired] {
            queue.retain(item).unwrap();
        }
        assert_eq!(queue.retire_slot(2), 2);
        assert_eq!(queue.pop(), Some(port_change));
        assert_eq!(queue.pop(), Some(foreign));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn retiring_another_slot_preserves_a_full_queue_and_its_pressure() {
        let mut queue = DeferredEvents::new();
        for pointer in 0..EVENT_TRBS as u64 {
            queue.retain(event(pointer)).unwrap();
        }
        assert_eq!(queue.retire_slot(3), 0);
        assert_eq!(queue.ensure_room(), Err(XhciError::EventPressure));
        for pointer in 0..EVENT_TRBS as u64 {
            assert_eq!(queue.pop(), Some(event(pointer)));
        }
    }

    #[test]
    fn pressure_retains_every_foreign_completion_and_port_change_across_repeated_wraps() {
        let mut queue = DeferredEvents::new();
        for lap in 0..128_u64 {
            for index in 0..EVENT_TRBS {
                queue
                    .retain(event(lap * EVENT_TRBS as u64 + index as u64))
                    .unwrap();
            }
            assert_eq!(queue.ensure_room(), Err(XhciError::EventPressure));
            assert_eq!(queue.retain(event(u64::MAX)), Err(XhciError::EventPressure));
            for index in 0..EVENT_TRBS {
                assert_eq!(
                    queue.pop(),
                    Some(event(lap * EVENT_TRBS as u64 + index as u64))
                );
            }
            assert_eq!(queue.pop(), None);
            assert_eq!(queue.ensure_room(), Ok(()));
        }
    }
}
