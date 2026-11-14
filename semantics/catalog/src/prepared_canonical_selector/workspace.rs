//! Explicit linked selection whose output is exclusively caller-owned.
use conduit_core::{
    PreparedBorrowedStructuredSelector, StructuredCanonicalSelection as Selection,
    StructuredSelectorRefusal as Refusal, UnmatchedVariantDisposition as Disposition,
};
pub struct PreparedLinkedWorkspaceSelector {
    selector: PreparedBorrowedStructuredSelector,
    maximum_input_bytes: usize,
    maximum_output_bytes: usize,
}
impl PreparedLinkedWorkspaceSelector {
    pub fn prepare_static(
        canonical: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            selector: PreparedBorrowedStructuredSelector::new(
                canonical,
                maximum_input_bytes,
                maximum_output_bytes,
            )?,
            maximum_input_bytes,
            maximum_output_bytes,
        })
    }
    pub fn input_schema_bytes(&self) -> &'static [u8] {
        self.selector.input_type_bytes()
    }
    pub fn output_schema_bytes(&self) -> &'static [u8] {
        self.selector.output_type_bytes()
    }
    pub fn maximum_output_bytes(&self) -> usize {
        self.maximum_output_bytes
    }
    pub fn select_into(&self, input: &[u8], output: &mut [u8]) -> Result<Option<usize>, Refusal> {
        if input.len() > self.maximum_input_bytes {
            return Err(Refusal::MalformedCheckedValue);
        }
        selection_result(self.selector.select_canonical_slice_into(input, output))
    }
    pub fn select_parts_into(
        &self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
    ) -> Result<Option<usize>, Refusal> {
        selection_result(self.selector.select_parts_slice_into(schema, node, output))
    }
}
fn selection_result(result: Result<(Selection, usize), Refusal>) -> Result<Option<usize>, Refusal> {
    match result.map_err(|error| match error {
        Refusal::WrongInputType => Refusal::MalformedCheckedValue,
        other => other,
    })? {
        (Selection::Matched, length) => Ok(Some(length)),
        (Selection::Unmatched(Disposition::Drop), _) => Ok(None),
        (Selection::Unmatched(Disposition::Refuse), _) => Err(Refusal::UnmatchedVariant),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, vec};
    use conduit_core::{
        kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
        StructuredInfoValue, StructuredSelector,
    };
    #[test]
    fn caller_output_profile_preserves_linked_owned_results_and_limits() {
        let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
        let ty = StructuredInfoType::record(
            kind_id("test/workspace-selection"),
            vec![StructuredFieldType::new("value", leaf.clone()).unwrap()],
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            ty.clone(),
            vec![StructuredFieldValue::new(
                "value",
                StructuredInfoValue::leaf(leaf, vec![7]).unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let selector = StructuredSelector::field(ty, "value").unwrap();
        let material = &*Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
        let mut owned =
            crate::PreparedCanonicalSelector::prepare_static(material, 512, 512).unwrap();
        let linked = PreparedLinkedWorkspaceSelector::prepare_static(material, 512, 512).unwrap();
        let expected = owned.select(&value).unwrap().unwrap();
        let mut output = [0xa5; 512];
        assert_eq!(
            linked.select_into(&value, &mut output),
            Ok(Some(expected.len()))
        );
        assert_eq!(&output[..expected.len()], expected);
        output.fill(0xa5);
        assert!(linked
            .select_into(&value, &mut output[..expected.len() - 1])
            .is_err());
        assert_eq!(output, [0xa5; 512]);
        let limited = PreparedLinkedWorkspaceSelector::prepare_static(
            material,
            512,
            linked.output_schema_bytes().len() + 1,
        )
        .unwrap();
        assert_eq!(
            limited.select_into(&value, &mut output),
            Err(Refusal::CanonicalEncodingTooLarge)
        );
        assert_eq!(output, [0xa5; 512]);
        assert_eq!(
            linked.select_parts_into(
                linked.input_schema_bytes(),
                &value[linked.input_schema_bytes().len()..],
                &mut output
            ),
            Ok(Some(expected.len()))
        );
    }
}

#[cfg(test)]
mod unmatched_tests {
    use super::*;
    use alloc::{boxed::Box, vec};
    use conduit_core::{
        kind_id, StructuredInfoType as T, StructuredInfoValue as V, StructuredSelector,
        StructuredVariantCase as Case,
    };
    #[test]
    fn actual_unmatched_drop_and_refuse_leave_caller_output_untouched() {
        let leaf = T::leaf(kind_id("value/u8")).unwrap();
        let ty = T::variant(
            kind_id("test/workspace-filter"),
            vec![
                Case::new("wanted", leaf.clone()).unwrap(),
                Case::new("other", leaf.clone()).unwrap(),
            ],
        )
        .unwrap();
        let input = V::variant(ty.clone(), "other", V::leaf(leaf, vec![3]).unwrap())
            .unwrap()
            .canonical_bytes()
            .unwrap();
        for disposition in [Disposition::Drop, Disposition::Refuse] {
            let definition =
                StructuredSelector::variant(ty.clone(), "wanted", disposition).unwrap();
            let canonical = &*Box::leak(definition.canonical_bytes().unwrap().into_boxed_slice());
            let selector =
                PreparedLinkedWorkspaceSelector::prepare_static(canonical, 512, 512).unwrap();
            let mut output = [0xa5; 512];
            assert_eq!(
                selector.select_into(&input, &mut output),
                if disposition == Disposition::Drop {
                    Ok(None)
                } else {
                    Err(Refusal::UnmatchedVariant)
                }
            );
            assert_eq!(output, [0xa5; 512]);
        }
    }
}
