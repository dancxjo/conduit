//! Retains one actual Host output across finite storage backpressure.
//!
//! This component never invokes an owner or steps a scheduler. Its fixed buffer
//! must be charged in preparation/working inventory before the owner is called.
use conduit_kernel::{NodeId, RequestId, ValueRef};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PendingOutputRefusal {
    Occupied,
    Capacity,
}

/// One completed computation whose scheduler completion has not been accepted.
/// Store and completion retries retain the original node/request and exact bytes.
pub struct PendingHostOutput<const BYTES: usize> {
    bytes: [u8; BYTES],
    pending: Option<(NodeId, RequestId, usize)>,
    stored: Option<ValueRef>,
}
impl<const BYTES: usize> Default for PendingHostOutput<BYTES> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const BYTES: usize> PendingHostOutput<BYTES> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; BYTES],
            pending: None,
            stored: None,
        }
    }
    pub fn retain(
        &mut self,
        node: NodeId,
        request: RequestId,
        bytes: &[u8],
    ) -> Result<(), PendingOutputRefusal> {
        if self.pending.is_some() {
            return Err(PendingOutputRefusal::Occupied);
        }
        if bytes.len() > BYTES || bytes.len() > u32::MAX as usize {
            return Err(PendingOutputRefusal::Capacity);
        }
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.pending = Some((node, request, bytes.len()));
        Ok(())
    }
    pub fn original(&self) -> Option<(NodeId, RequestId, &[u8])> {
        self.pending
            .map(|(node, request, len)| (node, request, &self.bytes[..len]))
    }
    /// A failed store preserves bytes and does not manufacture a completion.
    /// Once stored, further attempts reuse that exact lease instead of allocating.
    pub fn try_store<E>(
        &mut self,
        store: impl FnOnce(&[u8]) -> Result<ValueRef, E>,
    ) -> Result<Option<ValueRef>, E> {
        if let Some(value) = self.stored {
            return Ok(Some(value));
        }
        let Some((_, _, len)) = self.pending else {
            return Ok(None);
        };
        let value = store(&self.bytes[..len])?;
        self.stored = Some(value);
        Ok(Some(value))
    }
    /// The closure must call the actual selected scheduler's completion entrance.
    /// Rejected completion retains the lease for retry or explicit cancellation.
    pub fn try_complete<E>(
        &mut self,
        complete: impl FnOnce(NodeId, RequestId, ValueRef, u32) -> Result<(), E>,
    ) -> Result<bool, E> {
        let (Some((node, request, len)), Some(value)) = (self.pending, self.stored) else {
            return Ok(false);
        };
        complete(node, request, value, len as u32)?;
        self.pending = None;
        self.stored = None;
        Ok(true)
    }
    /// Call only after the actual scheduler has cancelled the corresponding call.
    /// The release closure must reclaim an unowned store lease; refusal retains it.
    pub fn retire_cancelled<E>(
        &mut self,
        release: impl FnOnce(ValueRef) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(value) = self.stored {
            release(value)?;
        }
        self.stored = None;
        self.pending = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "pending_host_output/tests.rs"]
mod tests;
