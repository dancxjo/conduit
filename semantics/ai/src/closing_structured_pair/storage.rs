//! Compose the exact bounded Core tuple encoder preparation, borrowing Types.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairBackPreparationRefusal {
    Capacity,
    Selection,
    Composition(PreparedStructuredCompositionStorageRefusal),
}
impl ClosingStructuredPairBack {
    pub fn storage_reservation(
        profile: &ClosingStructuredPairProfile,
    ) -> Result<PreparedStructuredCompositionStorageReceipt, PairBackPreparationRefusal> {
        PreparedTypedTuplePairEncoder::storage_reservation(
            &profile.left,
            profile.maximum[0],
            &profile.right,
            profile.maximum[1],
        )
        .map_err(PairBackPreparationRefusal::Composition)
    }
    /// Structural constructor only. Original selected factory supplies planned
    /// guard admission; Native laws remain with their separate existing owner.
    pub fn prepare_selected_with_storage_limits(
        profile: &ClosingStructuredPairProfile,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, PreparedStructuredCompositionStorageReceipt), PairBackPreparationRefusal>
    {
        let (encoder, receipt) = PreparedTypedTuplePairEncoder::new_with_storage_limits(
            &profile.left,
            profile.maximum[0],
            &profile.right,
            profile.maximum[1],
            maximum_preparation_requested_bytes,
            maximum_retained_heap_bytes,
        )
        .map_err(PairBackPreparationRefusal::Composition)?;
        if encoder.maximum_bytes() != profile.maximum[2] {
            return Err(PairBackPreparationRefusal::Selection);
        }
        Ok((
            Self {
                encoder,
                staged: false,
                committed_pairs: 0,
                cancelled: false,
            },
            receipt,
        ))
    }
}
