//! Private completed-request custody; no control mapping or Face authority.
use super::keyboard_input_binding::InputRealization;
use conduit_kernel::{scheduler::HostCallRequest, HostCallId, NodeId, ValueRef};

pub(super) const MAX_COMPLETED_INPUT_EVENTS: usize = 64;

pub(super) struct CompletedInputEvent {
    pub(super) descriptor: usize,
    pub(super) realization: InputRealization,
    pub(super) request: HostCallRequest,
    pub(super) value: ValueRef,
    pub(super) physical_event: u64,
    /// Captured private view epoch; this observation stamp grants no authority.
    pub(super) input_view_epoch: Option<u64>,
}

struct Slot {
    event: CompletedInputEvent,
    claimed: bool,
}

pub(super) struct KeyboardInputCompletions {
    slots: [Option<Slot>; MAX_COMPLETED_INPUT_EVENTS],
    enabled: bool,
    revoked: bool,
}
impl KeyboardInputCompletions {
    pub(super) fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
            enabled: false,
            revoked: false,
        }
    }
    pub(super) fn enable(&mut self) -> Result<(), String> {
        if self.enabled || self.revoked {
            return Err("completed input custody cannot be reused".into());
        }
        self.enabled = true;
        Ok(())
    }
    pub(super) fn enabled(&self) -> bool {
        self.enabled && !self.revoked
    }
    pub(super) fn require_capacity(&self, count: usize) -> Result<(), String> {
        if !self.enabled || self.revoked {
            return Err("completed input custody is unavailable".into());
        }
        if self.slots.iter().filter(|slot| slot.is_none()).count() < count {
            return Err("completed input custody capacity exhausted".into());
        }
        Ok(())
    }
    /// Called only after successful scheduler completion by KeyboardInputHost.
    pub(super) fn record(&mut self, event: CompletedInputEvent) -> Result<(), String> {
        self.require_capacity(1)?;
        if self.slots.iter().flatten().any(|slot| {
            slot.event.request.node == event.request.node
                && slot.event.request.request == event.request.request
        }) {
            return Err("input completion request recorded twice".into());
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or("completed input custody capacity exhausted")?;
        *slot = Some(Slot {
            event,
            claimed: false,
        });
        Ok(())
    }
    pub(super) fn claim(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<&CompletedInputEvent, String> {
        if !self.enabled() {
            return Err("completed input custody is revoked or absent".into());
        }
        let slot = self
            .slots
            .iter_mut()
            .flatten()
            .find(|slot| {
                slot.event.request.node == node
                    && slot.event.request.call == call
                    && slot.event.value == value
            })
            .ok_or("Fore value has no exact completed input origin")?;
        if slot.claimed {
            return Err("completed input event already claimed".into());
        }
        slot.claimed = true;
        Ok(&slot.event)
    }
    pub(super) fn acknowledge(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<CompletedInputEvent, String> {
        if !self.enabled() {
            return Err("completed input custody is revoked or absent".into());
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| {
                slot.as_ref().is_some_and(|slot| {
                    slot.event.request.node == node
                        && slot.event.request.call == call
                        && slot.event.value == value
                })
            })
            .ok_or("Fore acknowledgement has no exact completed input origin")?;
        if !slot.as_ref().is_some_and(|slot| slot.claimed) {
            return Err("completed input event was not claimed before acknowledgement".into());
        }
        let taken = slot.take().ok_or("completed input event disappeared")?;
        Ok(taken.event)
    }
    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        for slot in &mut self.slots {
            *slot = None;
        }
    }
}
impl Drop for KeyboardInputCompletions {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
#[path = "keyboard_input_completion_tests.rs"]
mod tests;
