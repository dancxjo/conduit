//! Complete numerical Native closure for the mixed owner. The lifecycle
//! partitions cannot substitute for one owner of this entire fixed boundary.
//! Immutable descriptors are already inventoried by Window8's shared static
//! receipt; this owner charges its additional heap and preparation separately.
use crate::generated::{
    LanguageParserProposalWindow8FeatureContext as Query,
    LanguageParserProposalWindow8V2Features as Guard,
    LanguageParserProposalWindow8V2ModelScores as Scores,
    LanguageParserProposalWindow8V2RawFeatures as Raw,
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeFamilyStorageReceipt,
    PreparedNativeRustBinding,
};
use core::{
    cell::RefCell,
    mem::{align_of, size_of},
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericFamilyRefusal;
#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericFamilyReservation {
    pub(crate) retained_bytes: usize,
    pub(crate) preparation_bytes: usize,
    pub(crate) conversion_bytes: usize,
}
pub(crate) struct Window8NumericFamily {
    pub(crate) owner: Rc<RefCell<PreparedNativeFamily>>,
    pub(crate) receipt: PreparedNativeFamilyStorageReceipt,
    pub(crate) reservation: NumericFamilyReservation,
}
impl Window8NumericFamily {
    pub(crate) fn reservation(
        limits: PreparedNativeFamilyLimits,
    ) -> Result<NumericFamilyReservation, NumericFamilyRefusal> {
        let owner = size_of::<RefCell<PreparedNativeFamily>>()
            .checked_add(2 * size_of::<usize>())
            .and_then(|n| n.checked_add(4 * align_of::<RefCell<PreparedNativeFamily>>()))
            .ok_or(NumericFamilyRefusal)?;
        Ok(NumericFamilyReservation {
            retained_bytes: limits
                .maximum_retained_bytes
                .checked_add(owner)
                .ok_or(NumericFamilyRefusal)?,
            preparation_bytes: limits
                .maximum_preparation_peak_bytes
                .checked_add(owner)
                .ok_or(NumericFamilyRefusal)?,
            conversion_bytes: limits.maximum_conversion_requested_bytes,
        })
    }
    /// The enclosing constructor admits this complete reservation with all
    /// other Session owners before calling any allocating preparation method.
    pub(crate) fn prepare(
        limits: PreparedNativeFamilyLimits,
        maximum_preparation_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, NumericFamilyRefusal> {
        let reservation = Self::reservation(limits)?;
        if reservation.preparation_bytes > maximum_preparation_bytes
            || reservation.retained_bytes > maximum_retained_bytes
        {
            return Err(NumericFamilyRefusal);
        }
        let family = PreparedNativeFamily::prepare(
            &[
                Query::PREPARED_DESCRIPTOR,
                Guard::PREPARED_DESCRIPTOR,
                Scores::PREPARED_DESCRIPTOR,
            ],
            limits,
        )
        .map_err(|_| NumericFamilyRefusal)?;
        for descriptor in [
            Query::PREPARED_DESCRIPTOR,
            Guard::PREPARED_DESCRIPTOR,
            Raw::PREPARED_DESCRIPTOR,
            Scores::PREPARED_DESCRIPTOR,
        ] {
            if !family.contains_descriptor(descriptor) {
                return Err(NumericFamilyRefusal);
            }
        }
        let receipt = family.storage_receipt();
        Ok(Self {
            owner: Rc::new(RefCell::new(family)),
            receipt,
            reservation,
        })
    }
}
