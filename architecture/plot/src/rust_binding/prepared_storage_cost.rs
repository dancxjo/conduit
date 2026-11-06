//! Checked preparation accounting shared by current generated storage layouts.
use super::{NativePathStep, PreparedDecodeRefusal, PreparedStorageBounds, PreparedStorageProfile};

/// Conservative payload accounting, excluding allocator bookkeeping. Inline
/// child storage is charged again when composed, so this can overestimate but
/// never discounts backing because a future alternative might be inactive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedStorageCost {
    pub storage_bytes: usize,
    pub text_bytes: usize,
    pub sequence_items: usize,
    pub staging_nodes: usize,
    pub maximum_steps: usize,
}
impl PreparedStorageCost {
    #[doc(hidden)]
    pub fn node(storage_bytes: usize) -> Self {
        Self {
            storage_bytes,
            text_bytes: 0,
            sequence_items: 0,
            staging_nodes: 1,
            maximum_steps: 1,
        }
    }
    #[doc(hidden)]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        Some(Self {
            storage_bytes: self.storage_bytes.checked_add(rhs.storage_bytes)?,
            text_bytes: self.text_bytes.checked_add(rhs.text_bytes)?,
            sequence_items: self.sequence_items.checked_add(rhs.sequence_items)?,
            staging_nodes: self.staging_nodes.checked_add(rhs.staging_nodes)?,
            maximum_steps: self.maximum_steps.checked_add(rhs.maximum_steps)?,
        })
    }
    #[doc(hidden)]
    pub fn checked_mul(self, count: usize) -> Option<Self> {
        Some(Self {
            storage_bytes: self.storage_bytes.checked_mul(count)?,
            text_bytes: self.text_bytes.checked_mul(count)?,
            sequence_items: self.sequence_items.checked_mul(count)?,
            staging_nodes: self.staging_nodes.checked_mul(count)?,
            maximum_steps: self.maximum_steps.checked_mul(count)?,
        })
    }
    /// Charge retained profile backing and the canonical buffer, then apply
    /// unchanged implementation ceilings and the owner's explicit envelope.
    /// The actual-work budget may be smaller than the schema's work ceiling.
    pub fn admit(
        mut self,
        profile: &PreparedStorageProfile,
        bounds: PreparedStorageBounds,
    ) -> Result<Self, PreparedDecodeRefusal> {
        let bytes = (|| {
            let mut bytes = self
                .storage_bytes
                .checked_add(bounds.decode.canonical_bytes)?;
            bytes = bytes.checked_add(core::mem::size_of::<PreparedStorageProfile>())?;
            bytes = bytes.checked_add(profile.root.0.capacity())?;
            bytes = bytes.checked_add(
                profile
                    .capacities
                    .capacity()
                    .checked_mul(core::mem::size_of::<super::PreparedStorageCapacity>())?,
            )?;
            for site in &profile.capacities {
                bytes = bytes.checked_add(
                    site.path
                        .capacity()
                        .checked_mul(core::mem::size_of::<NativePathStep>())?,
                )?;
                for part in &site.path {
                    if let NativePathStep::Field(name) | NativePathStep::Variant(name) = part {
                        bytes = bytes.checked_add(name.capacity())?;
                    }
                }
            }
            Some(bytes)
        })()
        .ok_or(PreparedDecodeRefusal::Preparation)?;
        self.storage_bytes = bytes;
        if self.staging_nodes > 4096
            || self.maximum_steps > 4096
            || self.text_bytes > 65536
            || self.text_bytes > bounds.decode.text_bytes
            || self.sequence_items > bounds.sequence_items
            || self.storage_bytes > bounds.storage_bytes
            || bounds.decode.leaf_steps == 0
            || bounds.decode.leaf_steps > 4096
            || bounds.decode.canonical_bytes == 0
            || bounds.decode.canonical_bytes > 65536
            || bounds.decode.text_bytes > 65536
        {
            return Err(PreparedDecodeRefusal::Preparation);
        }
        Ok(self)
    }
}
