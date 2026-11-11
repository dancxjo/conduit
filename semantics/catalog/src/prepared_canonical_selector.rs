//! Prepared pure structured selection, independent of any Body or device.
use alloc::vec::Vec;
use conduit_core::StructuredSelector;

/// Prepared pure selection invoked by the selector Back's existing Host Call.
/// The output envelope is reserved before Play; unmatched drops return None.
/// Preparation allocates within caller-admitted input/output bounds. This
/// value owns validation and selection storage, not a Host offer or grant.
pub struct PreparedCanonicalSelector {
    selector: StructuredSelector,
    validator: conduit_core::PreparedStructuredValueValidator,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
}
impl PreparedCanonicalSelector {
    pub fn prepare(
        selector: StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        Ok(Self {
            validator: conduit_core::PreparedStructuredValueValidator::new(
                selector.input_type(),
                maximum_input_bytes,
            )?,
            input_type: selector.input_type().canonical_bytes()?,
            output_type: selector.output_type().canonical_bytes()?,
            selector,
            output: Vec::with_capacity(maximum_output_bytes),
        })
    }
    pub fn select(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        use conduit_core::{
            StructuredCanonicalSelection as S, StructuredSelectorRefusal as R,
            UnmatchedVariantDisposition as D,
        };
        self.validator
            .validate(input)
            .map_err(|_| R::MalformedCheckedValue)?;
        match self.selector.select_canonical_into(
            input,
            &self.input_type,
            &self.output_type,
            &mut self.output,
        )? {
            S::Matched => Ok(Some(&self.output)),
            S::Unmatched(D::Drop) => Ok(None),
            S::Unmatched(D::Refuse) => Err(R::UnmatchedVariant),
        }
    }
}
