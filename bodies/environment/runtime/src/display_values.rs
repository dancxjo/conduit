//! Finite value budget for the independently pinned four-node output graph.
//! An exclusive Fore owner must enforce the same frame bound as the sign budget.
use conduit_kernel as k;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayValueBudget {
    items: u16,
    maximum_bytes: u32,
    bytes: u32,
}
impl DisplayValueBudget {
    /// Each admitted frame creates one ingress value, one encoder result and
    /// at most one output from each of two selectors: four distinct committed
    /// values over its ENTIRE lifetime. Counting both selectors is conservative
    /// even though their matching variants are exclusive. Fan-out and pending
    /// calls retain references to these values, not new physical copies.
    ///
    /// One extra slot covers the single staged generated output before a Step
    /// commits or rolls back. The pinned graph has one output per node and no
    /// terminal/abnormal output generation. Native write emits no value. Keeping
    /// every historical value therefore covers queue pressure, stalled calls,
    /// refused brightness and cancellation without assuming prompt consumption.
    /// Larger frame counts retain the previously admitted twelve-slot profile.
    /// This bound does not authorize a Fore, input, Mask, native call or Play.
    pub fn derive(frames: u32, maximum_bytes: u32) -> Result<Self, k::StorageError> {
        if frames == 0 || maximum_bytes == 0 || maximum_bytes > 1024 {
            return Err(k::StorageError::InvalidBudget);
        }
        let items = frames.saturating_mul(4).saturating_add(1).min(12) as u16;
        let bytes = u32::from(items)
            .checked_mul(maximum_bytes)
            .ok_or(k::StorageError::InvalidBudget)?;
        Ok(Self {
            items,
            maximum_bytes,
            bytes,
        })
    }
    pub fn items(self) -> u16 {
        self.items
    }
    pub fn bytes(self) -> u32 {
        self.bytes
    }
    pub fn storage_bytes(self) -> Result<usize, k::StorageError> {
        k::HostedValueStore::canonical_storage_bytes((0..self.items).map(|_| self.maximum_bytes))
    }
    pub fn prepare(self) -> Result<k::HostedValueStore, k::StorageError> {
        k::HostedValueStore::new(self.items, self.maximum_bytes, self.bytes)
    }
}
