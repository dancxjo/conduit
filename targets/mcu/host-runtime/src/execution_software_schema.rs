//! Schema support follows the actual prepared owner, including canonical fallback.
use super::*;
impl SoftwareBack<'_> {
    pub(super) fn input_schema<const P: usize>(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Selector(back) => {
                <SelectorBack as StepBack<P>>::canonical_input_schema(back, port)
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::canonical_input_schema(
                    back, port,
                )
            }
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::canonical_input_schema(
                    back, port,
                )
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::canonical_input_schema(back, port)
            }
            _ => None,
        }
    }
    pub(super) fn output_schema<const P: usize>(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::LinkedSource(back) => <conduit_typed_retention::PreparedLinkedSource as StepBack<P>>::canonical_output_schema(back, port),
            Self::Selector(back) => {
                <SelectorBack as StepBack<P>>::canonical_output_schema(back, port)
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::canonical_output_schema(
                    back, port,
                )
            }
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::canonical_output_schema(
                    back, port,
                )
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::canonical_output_schema(back, port)
            }
            _ => None,
        }
    }
}

#[cfg(all(test, feature = "schema-value-storage"))]
mod tests {
    use super::*;
    #[test]
    fn prepared_software_zip_exposes_exact_schema_hooks_and_raw_leaf_boundary() {
        use conduit_core::{
            kind_id, CheckedValueContract, StructuredFieldType, StructuredInfoType,
        };
        let leaf = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
        let record = StructuredInfoType::record(
            kind_id("test/software-zip"),
            alloc::vec![StructuredFieldType::new("number", leaf.clone()).unwrap()],
        )
        .unwrap();
        let left = CheckedValueContract::new(
            record.profile().unwrap().value_kind().clone(),
            1024,
            alloc::vec![],
        )
        .unwrap();
        let right = CheckedValueContract::new(kind_id("value/u64"), 8, alloc::vec![]).unwrap();
        let expected = conduit_core::tuple_info_type(alloc::vec![record.clone(), leaf.clone()])
            .unwrap()
            .canonical_bytes()
            .unwrap();
        for feedback in [false, true] {
            let back =
                SoftwareBack::zip(&left, record.clone(), &right, leaf.clone(), feedback).unwrap();
            let schema = record.canonical_bytes().unwrap();
            assert_eq!(
                <SoftwareBack as StepBack<2>>::canonical_input_schema(&back, PortId(0)),
                Some(schema.as_slice())
            );
            assert_eq!(
                <SoftwareBack as StepBack<2>>::canonical_input_schema(&back, PortId(1)),
                None
            );
            assert_eq!(
                <SoftwareBack as StepBack<2>>::canonical_input_schema(&back, PortId(2)),
                None
            );
            assert_eq!(
                <SoftwareBack as StepBack<2>>::canonical_output_schema(&back, PortId(0)),
                Some(expected.as_slice())
            );
            assert_eq!(
                <SoftwareBack as StepBack<2>>::canonical_output_schema(&back, PortId(1)),
                None
            );
        }
    }
}
