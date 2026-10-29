//! One immutable Make definition; runtime owners retain only its opaque handle.
#[derive(Debug, PartialEq, Eq)]
pub struct CompactCanonicalLayoutDefinition {
    identity: [u8; 32],
    physical_bytes: u16,
    logical_bytes: u32,
}
#[derive(Clone, Copy, Debug, Eq)]
pub struct CompactCanonicalLayout(&'static CompactCanonicalLayoutDefinition);
impl PartialEq for CompactCanonicalLayout {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0) || self.0 == other.0
    }
}
impl CompactCanonicalLayoutDefinition {
    /// Trusted Make/composition material only. The identity must bind the exact
    /// canonical Type, authored Forms and codec version. Preparation must also
    /// verify the actual producer/consumer against their checked Source/Plan.
    ///
    /// # Safety
    /// The selected codec must losslessly reconstruct every canonical value it
    /// emits, including failure branches, within these physical/logical bounds.
    /// Caller-supplied strings or Back declarations alone do not prove this.
    pub const unsafe fn from_trusted_make(
        identity: [u8; 32],
        physical_bytes: u16,
        logical_bytes: u32,
    ) -> Result<Self, crate::StorageError> {
        let mut nonzero = false;
        let mut index = 0;
        while index < identity.len() {
            nonzero |= identity[index] != 0;
            index += 1;
        }
        if !nonzero || physical_bytes == 0 || physical_bytes as u32 > logical_bytes {
            return Err(crate::StorageError::InvalidBudget);
        }
        Ok(Self {
            identity,
            physical_bytes,
            logical_bytes,
        })
    }
    pub const fn layout(&'static self) -> CompactCanonicalLayout {
        CompactCanonicalLayout(self)
    }
}
impl CompactCanonicalLayout {
    pub const fn physical_bytes(self) -> u16 {
        self.0.physical_bytes
    }
    pub const fn logical_bytes(self) -> u32 {
        self.0.logical_bytes
    }
}
