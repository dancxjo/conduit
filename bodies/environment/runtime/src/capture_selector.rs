use conduit_core::StructuredSelector;

/// Prepared pure selection invoked by the selector Back's existing Host Call.
/// The output envelope is reserved before Play; unmatched drops return None.
pub struct PreparedCaptureSelector {
    selector: StructuredSelector,
    maximum_input_bytes: usize,
    input_type: alloc::vec::Vec<u8>,
    output_type: alloc::vec::Vec<u8>,
    output: alloc::vec::Vec<u8>,
}
impl PreparedCaptureSelector {
    pub fn prepare(
        selector: StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        use conduit_core::StructuredInfoRefusal as R;
        if maximum_input_bytes == 0
            || maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(R::CanonicalEncodingTooLarge);
        }
        let input_type = selector.input_type().canonical_bytes()?;
        if input_type.len() >= maximum_input_bytes {
            return Err(R::CanonicalEncodingTooLarge);
        }
        // The complete checked Type bounds the output, including canonical
        // prefix/framing; the existing caller envelope remains the upper limit.
        let output_bytes =
            crate::canonical_storage::bytes(selector.output_type(), maximum_output_bytes)?;
        Ok(Self {
            maximum_input_bytes,
            input_type,
            output_type: selector.output_type().canonical_bytes()?,
            selector,
            output: alloc::vec::Vec::with_capacity(output_bytes),
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
        if input.len() > self.maximum_input_bytes {
            return Err(R::MalformedCheckedValue);
        }
        // Validate the full self-describing envelope with Core's bounded
        // borrowed validator, then require the independently prepared exact Type.
        // No second owned schema or validator prefix is retained.
        let validated = conduit_core::validate_canonical_structured_value(input)
            .map_err(|_| R::MalformedCheckedValue)?;
        if validated.type_bytes() != self.input_type {
            return Err(R::MalformedCheckedValue);
        }
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
