//! Capacity-one custody: one Source completion per exact Mask Play.
//! Private record minting is callable only by the enclosing scheduler admission.
use conduit_kernel::{scheduler::HostCallRequest, HostCallId, NodeId, ValueRef};

pub(crate) struct CompletedParsecInstruction {
    request: HostCallRequest,
    value: ValueRef,
    scope: [u8; 32],
}
impl CompletedParsecInstruction {
    pub(crate) fn request(&self) -> HostCallRequest {
        self.request
    }
    pub(crate) fn value(&self) -> ValueRef {
        self.value
    }
}
struct Slot {
    completed: CompletedParsecInstruction,
    claimed: bool,
}
pub(super) struct InstructionCompletion {
    slot: Option<Slot>,
    transferred: Option<(HostCallRequest, ValueRef, [u8; 32])>,
    revoked: bool,
}
impl InstructionCompletion {
    pub(super) fn new() -> Self {
        Self {
            slot: None,
            transferred: None,
            revoked: false,
        }
    }
    pub(super) fn require_capacity(&self) -> Result<(), String> {
        if self.revoked || self.slot.is_some() || self.transferred.is_some() {
            return Err("instruction Source completion capacity is absent or consumed".into());
        }
        Ok(())
    }
    /// Called after the enclosing method successfully completes the actual scheduler request.
    pub(super) fn record(&mut self, request: HostCallRequest, value: ValueRef, scope: [u8; 32]) {
        debug_assert!(!self.revoked && self.slot.is_none() && self.transferred.is_none());
        self.slot = Some(Slot {
            completed: CompletedParsecInstruction {
                request,
                value,
                scope,
            },
            claimed: false,
        });
    }
    pub(super) fn claim(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<(), String> {
        if self.revoked {
            return Err("instruction Source completion revoked".into());
        }
        let slot = self
            .slot
            .as_mut()
            .ok_or("instruction value has no completed Source origin")?;
        if slot.claimed
            || slot.completed.request.node != node
            || slot.completed.request.call != call
            || slot.completed.value != value
        {
            return Err("instruction Source claim differs or repeats".into());
        }
        slot.claimed = true;
        Ok(())
    }
    pub(super) fn acknowledge(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<CompletedParsecInstruction, String> {
        if self.revoked {
            return Err("instruction Source completion revoked".into());
        }
        let slot = self
            .slot
            .as_ref()
            .ok_or("instruction acknowledgement lacks Source origin")?;
        if !slot.claimed
            || slot.completed.request.node != node
            || slot.completed.request.call != call
            || slot.completed.value != value
        {
            return Err("instruction acknowledgement differs from exact claim".into());
        }
        let completed = self
            .slot
            .take()
            .ok_or("instruction Source completion lost")?
            .completed;
        self.transferred = Some((completed.request, completed.value, completed.scope));
        Ok(completed)
    }
    pub(super) fn require_transferred(
        &self,
        completed: &CompletedParsecInstruction,
        scope: [u8; 32],
    ) -> Result<(), String> {
        if self.revoked
            || completed.scope != scope
            || self.transferred != Some((completed.request, completed.value, completed.scope))
        {
            return Err("instruction effect has stale or unacknowledged Source custody".into());
        }
        Ok(())
    }
    pub(super) fn consume_effect(
        &mut self,
        completed: CompletedParsecInstruction,
        scope: [u8; 32],
    ) -> Result<(), String> {
        self.require_transferred(&completed, scope)?;
        self.transferred = None;
        // Never reopen capacity: the admitted Source emits only once per Mask Play.
        self.revoked = true;
        Ok(())
    }
    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        self.slot = None;
        self.transferred = None;
    }
}
impl Drop for InstructionCompletion {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
#[path = "parsec_instruction_source_completion_tests.rs"]
mod tests;
