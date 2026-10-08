//! Allocation-free reservation for selected structural guard Back construction.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardBackStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_accounted_heap_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardBackPreparationRefusal {
    Capacity,
    Validation,
}
impl FixedGuardBack {
    /// Includes validator Type/prefix clone and output capacity. Concrete Box root
    /// is added by the factory; profile ownership and its preparation are separate.
    pub fn storage_reservation(
        profile: &FixedGuardProfile,
    ) -> Result<GuardBackStorageReceipt, GuardBackPreparationRefusal> {
        let maximum = profile.maximum_bytes as usize;
        let validator =
            PreparedStructuredValueValidator::storage_reservation(&profile.value_type, maximum)
                .map_err(|_| GuardBackPreparationRefusal::Validation)?;
        Ok(GuardBackStorageReceipt {
            preparation_requested_bytes_bound: validator
                .preparation_requested_bytes_bound
                .checked_add(maximum)
                .ok_or(GuardBackPreparationRefusal::Capacity)?,
            retained_heap_bytes_bound: validator
                .retained_heap_bytes_bound
                .checked_add(maximum)
                .ok_or(GuardBackPreparationRefusal::Capacity)?,
            retained_accounted_heap_bytes: 0,
        })
    }
    /// Shape-only construction, just like the original guard. Does not grant
    /// Native-law or planned-placement admission; the selected factory retains it.
    pub fn prepare_selected_with_storage_limits(
        profile: &FixedGuardProfile,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, GuardBackStorageReceipt), GuardBackPreparationRefusal> {
        let mut receipt = Self::storage_reservation(profile)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(GuardBackPreparationRefusal::Capacity);
        }
        let maximum_bytes = profile.maximum_bytes as usize;
        let (validator, _) = PreparedStructuredValueValidator::new_with_storage_limits(
            &profile.value_type,
            maximum_bytes,
            maximum_preparation_requested_bytes - maximum_bytes,
            maximum_retained_heap_bytes - maximum_bytes,
        )
        .map_err(|_| GuardBackPreparationRefusal::Validation)?;
        let back = Self {
            validator,
            output: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
            staged: false,
            committed_frames: 0,
            cancelled: false,
        };
        receipt.retained_accounted_heap_bytes = back.local_accounted_heap_bytes();
        Ok((back, receipt))
    }
}
