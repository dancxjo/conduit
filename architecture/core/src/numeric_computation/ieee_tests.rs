use super::*;
use alloc::vec;

fn literal(bits: u64) -> NumericComputationNode {
    NumericComputationNode {
        semantic_kind: crate::F64_INFO_ID.into(),
        unit: None,
        reference_unit: None,
        operation: NumericComputationOperation::IeeeLiteral { bits },
    }
}

#[test]
fn ieee_literal_bits_and_order_are_custody_not_mathematical_values() {
    for bits in [0, 1_u64 << 63, 0x7ff0_0000_0000_0000, 0x7ff8_0000_0000_0042] {
        let graph = NumericComputation::new(vec![literal(bits)], 0).unwrap();
        assert_eq!(
            NumericComputation::decode_validate(&graph.canonical_bytes().unwrap()).unwrap(),
            graph
        );
        assert_eq!(
            graph.evaluate_exact(&[]),
            Err(NumericEvaluationRefusal::UnsupportedOperation)
        );
    }
    assert_ne!(
        NumericComputation::new(vec![literal(0)], 0)
            .unwrap()
            .identity()
            .unwrap(),
        NumericComputation::new(vec![literal(1_u64 << 63)], 0)
            .unwrap()
            .identity()
            .unwrap()
    );
    let mut forged = literal(0x7ff8_0000_0000_0042);
    forged.semantic_kind = crate::SCALAR_INFO_ID.into();
    assert!(NumericComputation::new(vec![forged], 0).is_err());
}

#[test]
fn ieee_operations_refuse_cross_ontology_inputs() {
    let add = NumericComputationNode {
        semantic_kind: crate::F64_INFO_ID.into(),
        unit: None,
        reference_unit: None,
        operation: NumericComputationOperation::Apply {
            operation: NumericOperation::Add,
            operands: vec![0, 1],
        },
    };
    assert!(
        NumericComputation::new(vec![literal(0), literal(1_u64 << 63), add.clone()], 2).is_ok()
    );
    let mathematical =
        NumericComputationNode::literal(crate::SCALAR_INFO_ID.into(), None, &MathValue::ONE)
            .unwrap();
    assert!(NumericComputation::new(vec![literal(0), mathematical, add], 2).is_err());
}

#[test]
fn intrinsic_scalar_profile_bounds_are_checked_at_each_node() {
    let mut profile = crate::PortableNumericProfile::MAXIMAL;
    profile.maximum_encoded_bytes = 64;
    let kind = profile.info_id().unwrap();
    let small = NumericComputationNode::literal(kind.clone(), None, &MathValue::ONE).unwrap();
    assert!(NumericComputation::new(vec![small], 0).is_ok());
    let large = MathValue::integer(10).checked_pow(1000).unwrap();
    let literal = NumericComputationNode::literal(kind, None, &large).unwrap();
    assert!(NumericComputation::new(vec![literal], 0).is_err());
}
