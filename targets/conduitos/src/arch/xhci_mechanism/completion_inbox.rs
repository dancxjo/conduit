//! Fixed completion retention beneath the controller's sole event consumer.
//! Prepared with the controller before device publication or any Source Play.
use super::{EVENT_TRBS, Event, XhciError};

// At most the existing hardware-ring extent is retained. This is inline Root
// storage, not another hardware ring, allocation, scheduler, or effect route.
const RETAINED_EVENT_SLOTS: usize = EVENT_TRBS;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct RetainedEvents {
    entries: [Option<Event>; RETAINED_EVENT_SLOTS],
    len: usize,
}

impl RetainedEvents {
    pub(super) const fn new() -> Self {
        Self {
            entries: [None; RETAINED_EVENT_SLOTS],
            len: 0,
        }
    }

    #[cfg(any(test, all(target_arch = "x86_64", target_os = "none")))]
    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn take_matching(&mut self, matches: &impl Fn(Event) -> bool) -> Option<Event> {
        let index = self.entries[..self.len]
            .iter()
            .position(|entry| entry.is_some_and(matches))?;
        let event = self.entries[index].take();
        self.entries.copy_within(index + 1..self.len, index);
        self.len -= 1;
        self.entries[self.len] = None;
        event
    }

    pub(super) fn pop(&mut self) -> Option<Event> {
        self.take_matching(&|_| true)
    }

    /// Check at most the admitted inbox and one hardware event. A foreign
    /// completion remains owned and ordered; pressure refuses before consuming
    /// hardware when there is no reservation to retain its possible result.
    pub(super) fn poll_with(
        &mut self,
        matches: impl Fn(Event) -> bool,
        mut receive: impl FnMut() -> Option<Event>,
    ) -> Result<Option<Event>, XhciError> {
        if let Some(event) = self.take_matching(&matches) {
            return Ok(Some(event));
        }
        if self.len == RETAINED_EVENT_SLOTS {
            return Err(XhciError::EventRetentionPressure);
        }
        let Some(event) = receive() else {
            return Ok(None);
        };
        if matches(event) {
            return Ok(Some(event));
        }
        self.entries[self.len] = Some(event);
        self.len += 1;
        Ok(None)
    }
}

#[cfg(test)]
#[path = "completion_inbox_tests.rs"]
mod tests;
