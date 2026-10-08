//! Mechanical adapter to the selected Plot owner's existing exact transport bound.
use conduit_core::StructuredInfoType;
use conduit_plot::{
    PortableExpressionEvaluationRefusal, PortableExpressionNode, PortableExpressionOperation,
    PortableExpressionProgram,
};
pub fn maximum_prepared_transport_value_bytes(
    ty: &StructuredInfoType,
) -> Result<u32, PortableExpressionEvaluationRefusal> {
    PortableExpressionProgram {
        input_type: ty.clone(),
        output_type: ty.clone(),
        root: PortableExpressionNode {
            value_type: ty.clone(),
            operation: PortableExpressionOperation::Input,
        },
    }
    .maximum_prepared_input_bytes()
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::{StructuredFieldType, kind_id};
    #[test]
    fn exact_raw_primitive_and_full_structured_transport() {
        let leaf = StructuredInfoType::leaf(kind_id("value/u16")).unwrap();
        assert_eq!(maximum_prepared_transport_value_bytes(&leaf).unwrap(), 2);
        assert!(conduit_plot::maximum_prepared_canonical_value_bytes(&leaf).unwrap() > 2);
        let record = StructuredInfoType::record(
            kind_id("probe/transport"),
            vec![StructuredFieldType::new("x", leaf).unwrap()],
        )
        .unwrap();
        assert_eq!(
            maximum_prepared_transport_value_bytes(&record).unwrap(),
            conduit_plot::maximum_prepared_canonical_value_bytes(&record).unwrap()
        );
    }
}
