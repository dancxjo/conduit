//! Ordinary checked constructor results in target-neutral expression programs.
use super::*;
use conduit_core::StructuredInfoValue;

impl PortableExpressionProgram {
    /// Retains a bounded checked value without reconstructing a Source literal.
    /// Domain preparation remains the constructor owner's responsibility.
    pub fn from_checked_value(input_type: StructuredInfoType, value: &StructuredInfoValue) -> Self {
        Self {
            input_type,
            output_type: value.value_type().clone(),
            root: PortableExpressionNode {
                value_type: value.value_type().clone(),
                operation: PortableExpressionOperation::Constant(value.clone()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedPortableExpressionEvaluator;
    use conduit_core::{kind_id, StructuredInfoValueShape, EMPTY_INFO_ID, TEXT_INFO_ID};

    fn empty() -> StructuredInfoType {
        StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap()
    }
    fn text() -> StructuredInfoValue {
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap(),
            "kʰæt".as_bytes().to_vec(),
        )
        .unwrap()
    }
    fn round_trip(value: StructuredInfoValue) {
        let program = PortableExpressionProgram::from_checked_value(empty(), &value);
        let encoded = program.canonical_bytes().unwrap();
        let decoded = PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(decoded, program);
        let expected = match (value.value_type().shape(), value.shape()) {
            (
                conduit_core::StructuredInfoTypeShape::Leaf(_),
                StructuredInfoValueShape::Leaf(bytes),
            ) => bytes.to_vec(),
            _ => value.canonical_bytes().unwrap(),
        };
        assert_eq!(decoded.evaluate(&[]).unwrap(), expected);
        let mut prepared = PreparedPortableExpressionEvaluator::new(&decoded).unwrap();
        assert_eq!(prepared.evaluate(&[]).unwrap(), expected);
        let capacity = prepared.output_capacity();
        for _ in 0..32 {
            assert_eq!(prepared.evaluate(&[]).unwrap(), expected);
        }
        assert_eq!(prepared.output_capacity(), capacity);
        assert!(decoded.evaluate(b"foreign input").is_err());
    }
    #[test]
    fn constant_program_preserves_primitive_and_nominal_unicode_values() {
        round_trip(text());
        let value = text();
        let nominal =
            StructuredInfoType::nominal(kind_id("fixture/text@1"), value.value_type().clone())
                .unwrap();
        round_trip(StructuredInfoValue::nominal(nominal, value).unwrap());
    }
    #[test]
    fn constant_program_preserves_nested_collection_and_nominal_type() {
        let leaf = text();
        let ty = StructuredInfoType::collection(leaf.value_type().clone(), Some(2)).unwrap();
        let value = StructuredInfoValue::collection(ty.clone(), vec![leaf.clone(), leaf]).unwrap();
        let nominal = StructuredInfoType::nominal(kind_id("fixture/transcription@1"), ty).unwrap();
        round_trip(StructuredInfoValue::nominal(nominal, value).unwrap());
        let leaf = text();
        let sequence_type = StructuredInfoType::sequence(leaf.value_type().clone(), 3).unwrap();
        let sequence =
            StructuredInfoValue::sequence(sequence_type.clone(), vec![leaf.clone(), leaf]).unwrap();
        let nominal =
            StructuredInfoType::nominal(kind_id("fixture/variable-transcription@1"), sequence_type)
                .unwrap();
        round_trip(StructuredInfoValue::nominal(nominal, sequence).unwrap());
    }
    #[test]
    fn constant_program_refuses_foreign_type_and_corrupt_or_excessive_encoding() {
        let mut program = PortableExpressionProgram::from_checked_value(empty(), &text());
        let encoded = program.canonical_bytes().unwrap();
        let mut corrupted = encoded.clone();
        *corrupted.last_mut().unwrap() = 0xff;
        assert!(PortableExpressionProgram::from_canonical_bytes(&corrupted).is_err());
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(PortableExpressionProgram::from_canonical_bytes(&trailing).is_err());
        let operation_offset = encoded.len() - text().canonical_bytes().unwrap().len() - 9;
        let mut excessive = encoded;
        excessive[operation_offset + 1..operation_offset + 9]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(PortableExpressionProgram::from_canonical_bytes(&excessive).is_err());
        program.root.value_type = empty();
        assert!(program.canonical_bytes().is_err());
        assert!(program.evaluate(&[]).is_err());
        assert!(PreparedPortableExpressionEvaluator::new(&program).is_err());
    }
}
