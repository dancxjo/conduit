//! Prepared pure structured selection, independent of any Body or device.
use alloc::borrow::Cow;
use conduit_core::StructuredSelector;

/// Prepared pure selection used by ordinary software Backs, with no Host effects.
/// The output envelope is reserved before Play; unmatched drops return None.
pub struct PreparedCanonicalSelector {
    selector: CaptureSelection,
    maximum_input_bytes: usize,
    input_type: Cow<'static, [u8]>,
    output_type: Cow<'static, [u8]>,
    output: alloc::vec::Vec<u8>,
}
enum CaptureSelection {
    Owned(StructuredSelector),
    Linked(conduit_core::PreparedBorrowedStructuredSelector),
}
impl PreparedCanonicalSelector {
    pub fn prepare(
        selector: StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredInfoRefusal> {
        let input_type = selector.input_type().canonical_bytes()?;
        if maximum_input_bytes == 0
            || maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || input_type.len() >= maximum_input_bytes
        {
            return Err(conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            maximum_input_bytes,
            input_type: Cow::Owned(input_type),
            output_type: Cow::Owned(selector.output_type().canonical_bytes()?),
            selector: CaptureSelection::Owned(selector),
            output: alloc::vec::Vec::with_capacity(maximum_output_bytes),
        })
    }
    /// Check the complete linked selector once, retaining its operation and
    /// immutable schema spans rather than duplicate decoded Type trees.
    pub fn prepare_static(
        canonical: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, conduit_core::StructuredSelectorRefusal> {
        let selector = conduit_core::PreparedBorrowedStructuredSelector::new(
            canonical,
            maximum_input_bytes,
            maximum_output_bytes,
        )?;
        Ok(Self {
            maximum_input_bytes,
            input_type: Cow::Borrowed(selector.input_type_bytes()),
            output_type: Cow::Borrowed(selector.output_type_bytes()),
            selector: CaptureSelection::Linked(selector),
            output: alloc::vec::Vec::with_capacity(maximum_output_bytes),
        })
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }
    pub fn select(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        use conduit_core::StructuredSelectorRefusal as R;
        if input.len() > self.maximum_input_bytes {
            return Err(R::MalformedCheckedValue);
        }
        let selected = match &self.selector {
            CaptureSelection::Linked(selector) => selector
                .select_canonical_into(input, &mut self.output)
                .map_err(|error| match error {
                    R::WrongInputType => R::MalformedCheckedValue,
                    other => other,
                })?,
            CaptureSelection::Owned(selector) => {
                let node = input
                    .strip_prefix(self.input_type.as_ref())
                    .ok_or(R::MalformedCheckedValue)?;
                selector
                    .input_type()
                    .validate_canonical_value_node(node)
                    .map_err(|_| R::MalformedCheckedValue)?;
                selector.select_canonical_into(
                    input,
                    &self.input_type,
                    &self.output_type,
                    &mut self.output,
                )?
            }
        };
        self.selection_result(selected)
    }
    /// Only the linked profile exposes immutable schema spans for execution.
    pub fn input_schema_bytes(&self) -> Option<&[u8]> {
        matches!(self.selector, CaptureSelection::Linked(_)).then_some(self.input_type.as_ref())
    }
    pub fn output_schema_bytes(&self) -> Option<&[u8]> {
        matches!(self.selector, CaptureSelection::Linked(_)).then_some(self.output_type.as_ref())
    }
    /// Consume separate storage only through the checked linked profile. The
    /// full input is validated; output keeps its ordinary canonical encoding.
    pub fn select_parts(
        &mut self,
        schema: &[u8],
        node: &[u8],
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        use conduit_core::StructuredSelectorRefusal as R;
        let CaptureSelection::Linked(selector) = &self.selector else {
            return Err(R::MalformedCheckedValue);
        };
        let selected = selector
            .select_parts_into(schema, node, &mut self.output)
            .map_err(|error| match error {
                R::WrongInputType => R::MalformedCheckedValue,
                other => other,
            })?;
        self.selection_result(selected)
    }
    fn selection_result(
        &self,
        selected: conduit_core::StructuredCanonicalSelection,
    ) -> Result<Option<&[u8]>, conduit_core::StructuredSelectorRefusal> {
        use conduit_core::{
            StructuredCanonicalSelection as S, StructuredSelectorRefusal as R,
            UnmatchedVariantDisposition as D,
        };
        match selected {
            S::Matched => Ok(Some(&self.output)),
            S::Unmatched(D::Drop) => Ok(None),
            S::Unmatched(D::Refuse) => Err(R::UnmatchedVariant),
        }
    }
}

#[cfg(test)]
mod linked_selector_tests {
    use super::*;
    use alloc::boxed::Box;
    use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};

    #[test]
    fn linked_selector_borrows_both_exact_type_prefixes() {
        let leaf = StructuredInfoType::leaf(kind_id("value/boolean")).unwrap();
        let input = StructuredInfoType::record(
            kind_id("test/linked-selector"),
            alloc::vec![StructuredFieldType::new("chosen", leaf).unwrap()],
        )
        .unwrap();
        let selector = StructuredSelector::field(input, "chosen").unwrap();
        let canonical = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
        let linked = PreparedCanonicalSelector::prepare_static(canonical, 512, 512).unwrap();
        assert!(matches!(linked.selector, CaptureSelection::Linked(_)));
        assert!(matches!(linked.input_type, Cow::Borrowed(_)));
        assert!(matches!(linked.output_type, Cow::Borrowed(_)));
        assert_eq!(
            linked.input_type.as_ref(),
            selector.input_type().canonical_bytes().unwrap()
        );
        assert_eq!(
            linked.output_type.as_ref(),
            selector.output_type().canonical_bytes().unwrap()
        );
    }
}

#[path = "prepared_canonical_selector/workspace.rs"]
mod workspace;
pub use workspace::PreparedLinkedWorkspaceSelector;
