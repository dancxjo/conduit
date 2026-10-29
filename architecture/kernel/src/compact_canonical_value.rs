//! Physical layout identity, not semantic eligibility or execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactCanonicalLayout {
    identity: [u8; 32],
    physical_bytes: u16,
    logical_bytes: u32,
}
impl CompactCanonicalLayout {
    /// Trusted Make/composition material only. The identity must bind the exact
    /// canonical Type, authored Forms and codec version. Preparation must also
    /// verify the actual producer/consumer against their checked Source/Plan.
    ///
    /// # Safety
    /// The selected codec must losslessly reconstruct every canonical value it
    /// emits, including failure branches, within these physical/logical bounds.
    /// Caller-supplied strings or Back declarations alone do not prove this.
    pub unsafe fn from_trusted_make(
        identity: [u8; 32],
        physical_bytes: u16,
        logical_bytes: u32,
    ) -> Result<Self, crate::StorageError> {
        if identity == [0; 32] || physical_bytes == 0 || u32::from(physical_bytes) > logical_bytes {
            return Err(crate::StorageError::InvalidBudget);
        }
        Ok(Self {
            identity,
            physical_bytes,
            logical_bytes,
        })
    }
    pub const fn physical_bytes(self) -> u16 {
        self.physical_bytes
    }
    pub const fn logical_bytes(self) -> u32 {
        self.logical_bytes
    }
}
