//! Full selected weakening Back construction reservation; profile prep separate.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeakeningBackStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_accounted_heap_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeakeningBackPreparationRefusal {
    Capacity,
    Validation,
}
fn checked_sum(values: &[usize]) -> Result<usize, WeakeningBackPreparationRefusal> {
    values.iter().try_fold(0usize, |a, b| {
        a.checked_add(*b)
            .ok_or(WeakeningBackPreparationRefusal::Capacity)
    })
}
impl NominalWeakeningBack {
    pub fn storage_reservation(
        profile: &PreparedNominalWeakening,
    ) -> Result<WeakeningBackStorageReceipt, WeakeningBackPreparationRefusal> {
        let input = PreparedStructuredValueValidator::storage_reservation(
            &profile.input,
            profile.input_maximum as usize,
        )
        .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let output = PreparedStructuredValueValidator::storage_reservation(
            &profile.output,
            profile.output_maximum as usize,
        )
        .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let prefix_bytes = checked_sum(&[
            profile
                .input
                .canonical_byte_length()
                .map_err(|_| WeakeningBackPreparationRefusal::Validation)?,
            profile
                .output
                .canonical_byte_length()
                .map_err(|_| WeakeningBackPreparationRefusal::Validation)?,
        ])?;
        Ok(WeakeningBackStorageReceipt {
            preparation_requested_bytes_bound: checked_sum(&[
                input.preparation_requested_bytes_bound,
                output.preparation_requested_bytes_bound,
                prefix_bytes,
                profile.output_maximum as usize,
            ])?,
            retained_heap_bytes_bound: checked_sum(&[
                input.retained_heap_bytes_bound,
                output.retained_heap_bytes_bound,
                prefix_bytes,
                profile.output_maximum as usize,
            ])?,
            retained_accounted_heap_bytes: 0,
        })
    }
    /// Same explicit weakening operation as the legacy planned constructor. Does
    /// not establish Native refinements/laws or independently verify a placement.
    pub fn prepare_selected_with_storage_limits(
        profile: &PreparedNominalWeakening,
        flow: bool,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, WeakeningBackStorageReceipt), WeakeningBackPreparationRefusal> {
        let mut receipt = Self::storage_reservation(profile)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(WeakeningBackPreparationRefusal::Capacity);
        }
        let input =
            PreparedStructuredValueValidator::new(&profile.input, profile.input_maximum as usize)
                .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let output_validator =
            PreparedStructuredValueValidator::new(&profile.output, profile.output_maximum as usize)
                .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let input_prefix = profile
            .input
            .canonical_bytes()
            .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let output_prefix = profile
            .output
            .canonical_bytes()
            .map_err(|_| WeakeningBackPreparationRefusal::Validation)?;
        let back = Self {
            input,
            output_validator,
            input_prefix,
            output_prefix,
            output: vec![0; profile.output_maximum as usize],
            length: 0,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed: 0,
        };
        receipt.retained_accounted_heap_bytes = back.local_accounted_heap_bytes();
        Ok((back, receipt))
    }
}
