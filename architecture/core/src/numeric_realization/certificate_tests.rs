use super::*;
use crate::numeric_projection::*;
use crate::{
    MathValue, NumericComputation, NumericComputationNode, NumericComputationOperation,
    NumericOperation,
};
use alloc::{string::ToString, vec, vec::Vec};
fn requirement(representation: NumericRepresentation) -> NumericRequirement {
    NumericRequirement {
        semantics: NumericSemantics::Mathematical,
        representation,
        rounding: Binary64Rounding::NearestEven,
        accuracy: NumericAccuracy::Absolute(MathValue::integer(1)),
        parameters: Vec::new(),
        budget: NumericExecutionBudget {
            storage_bytes: 1_048_576,
            work_steps: 100_000,
            limbs: 128,
            symbolic_nodes: 128,
        },
        reproducible: true,
        exceptions: NumericExceptionPolicy {
            inexact: true,
            ..Default::default()
        },
    }
}
fn cancellation() -> NumericComputation {
    let literal = |n| {
        NumericComputationNode::literal(
            crate::MATH_VALUE_INFO_ID.to_string(),
            None,
            &MathValue::integer(n),
        )
        .unwrap()
    };
    let apply = |operation, operands| NumericComputationNode {
        semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
        unit: None,
        reference_unit: None,
        operation: NumericComputationOperation::Apply {
            operation,
            operands,
        },
    };
    NumericComputation::new(
        vec![
            literal(10_000_000_000_000_000),
            literal(1),
            apply(NumericOperation::Add, vec![0, 1]),
            apply(NumericOperation::Subtract, vec![2, 0]),
        ],
        3,
    )
    .unwrap()
}
#[test]
fn ordered_rounding_cancellation_and_forged_claims_are_checked() {
    let graph = cancellation();
    let req = requirement(NumericRepresentation::Binary64);
    let witness = certify_computation(&graph, &req).unwrap();
    assert_eq!(witness.claim().steps.len(), 4);
    assert_eq!(
        witness.claim().result.interval().unwrap().lower(),
        &MathValue::integer(0)
    );
    assert_eq!(witness.claim().error.lower(), &MathValue::integer(-1));
    assert!(witness.claim().flags.inexact);
    verify_computation_claim(witness.claim(), &graph, &req).unwrap();
    let bytes = witness.claim().canonical_bytes().unwrap();
    let decoded = NumericCertificateClaim::decode(&bytes).unwrap();
    assert_eq!(&decoded, witness.claim());
    verify_computation_claim(&decoded, &graph, &req).unwrap();
    verify_encoded_computation_claim(&bytes, &graph, &req).unwrap();
    let mut wrong_algorithm = decoded.clone();
    wrong_algorithm.algorithm = NumericAlgorithm::OrderedRationalV1;
    assert!(verify_computation_claim(&wrong_algorithm, &graph, &req).is_err());
    assert!(NumericCertificateClaim::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(NumericCertificateClaim::decode(&appended).is_err());
    let mut forged = witness.claim().clone();
    forged.steps[2].output += 1;
    assert_eq!(
        verify_computation_claim(&forged, &graph, &req),
        Err(NumericEngineRefusal::ClaimMismatch)
    );
    let mut exact = req.clone();
    exact.accuracy = NumericAccuracy::Exact;
    assert_eq!(
        certify_computation(&graph, &exact),
        Err(NumericEngineRefusal::AccuracyUnproved)
    );
    let rational =
        certify_computation(&graph, &requirement(NumericRepresentation::ExactRational)).unwrap();
    assert_eq!(
        rational.claim().result.mathematical().unwrap(),
        &MathValue::integer(1)
    );
    assert_ne!(
        rational.requirement_identity(),
        witness.requirement_identity()
    );
}
#[test]
fn finite_interval_proofs_narrow_and_preserve_squared_inequalities() {
    let mut work = NumericRealizationWork::new(100_000);
    let wide = enclose_pi(4, &mut work).unwrap();
    let narrow = enclose_pi(12, &mut work).unwrap();
    assert_eq!(
        narrow.lower().compare(wide.lower()).unwrap(),
        Some(core::cmp::Ordering::Greater)
    );
    assert_eq!(
        narrow.upper().compare(wide.upper()).unwrap(),
        Some(core::cmp::Ordering::Less)
    );
    let square_root = enclose_sqrt(&MathValue::integer(2), 64, &mut work).unwrap();
    assert_eq!(
        square_root
            .lower()
            .checked_mul(square_root.lower())
            .unwrap()
            .compare(&MathValue::integer(2))
            .unwrap(),
        Some(core::cmp::Ordering::Less)
    );
    assert_eq!(
        square_root
            .upper()
            .checked_mul(square_root.upper())
            .unwrap()
            .compare(&MathValue::integer(2))
            .unwrap(),
        Some(core::cmp::Ordering::Greater)
    );
    assert!(enclose_pi(12, &mut NumericRealizationWork::new(1)).is_err());
}

#[test]
fn uniform_range_enclosures_are_not_singleton_observations() {
    let parameter = NumericComputationNode {
        semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
        unit: None,
        reference_unit: None,
        operation: NumericComputationOperation::Parameter(0),
    };
    let graph = NumericComputation::new(
        vec![
            parameter,
            NumericComputationNode {
                semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
                unit: None,
                reference_unit: None,
                operation: NumericComputationOperation::Apply {
                    operation: NumericOperation::Subtract,
                    operands: vec![0, 0],
                },
            },
        ],
        1,
    )
    .unwrap();
    let mut req = requirement(NumericRepresentation::CertifiedInterval);
    req.parameters = vec![NumericParameterRange {
        slot: 0,
        lower: MathValue::integer(1),
        upper: MathValue::integer(2),
    }];
    req.accuracy = NumericAccuracy::EnclosureWidth(MathValue::integer(2));
    let witness = certify_computation(&graph, &req).unwrap();
    assert_eq!(
        witness.claim().result.interval().unwrap().lower(),
        &MathValue::integer(-1)
    );
    assert_eq!(
        witness.claim().result.interval().unwrap().upper(),
        &MathValue::integer(1)
    );
    assert!(witness.claim().steps.is_empty());
    req.accuracy = NumericAccuracy::EnclosureWidth(MathValue::integer(1));
    assert_eq!(
        certify_computation(&graph, &req),
        Err(NumericEngineRefusal::AccuracyUnproved)
    );
    req.representation = NumericRepresentation::Binary64;
    req.accuracy = NumericAccuracy::Absolute(MathValue::integer(1));
    assert_eq!(
        certify_computation(&graph, &req),
        Err(NumericEngineRefusal::UnsupportedRange)
    );
}
#[test]
fn symbolic_trig_enclosure_uses_a_proved_remainder() {
    let mut work = NumericRealizationWork::new(100_000);
    let value = MathValue::rational("1", "7").unwrap().sin_turns().unwrap();
    let interval = enclose_math_value(&value, &mut work).unwrap();
    let lower = MathValue::rational("78", "100").unwrap();
    let upper = MathValue::rational("79", "100").unwrap();
    assert_eq!(
        interval.lower().compare(&lower).unwrap(),
        Some(core::cmp::Ordering::Greater)
    );
    assert_eq!(
        interval.upper().compare(&upper).unwrap(),
        Some(core::cmp::Ordering::Less)
    );
}

#[test]
fn exact_comparison_never_uses_an_error_tolerance_as_truth() {
    let literal = |value| {
        NumericComputationNode::literal(crate::MATH_VALUE_INFO_ID.to_string(), None, &value)
            .unwrap()
    };
    let graph = NumericComputation::new(
        vec![
            literal(MathValue::rational("1", "3").unwrap()),
            literal(MathValue::rational("2", "6").unwrap()),
            NumericComputationNode {
                semantic_kind: crate::BOOL_INFO_ID.to_string(),
                unit: None,
                reference_unit: None,
                operation: NumericComputationOperation::Apply {
                    operation: NumericOperation::Equal,
                    operands: vec![0, 1],
                },
            },
        ],
        2,
    )
    .unwrap();
    let mut req = requirement(NumericRepresentation::ExactRational);
    req.accuracy = NumericAccuracy::Exact;
    let witness = certify_computation(&graph, &req).unwrap();
    assert_eq!(witness.claim().result.boolean(), Some(true));
    assert!(witness.claim().result.interval().is_none());
    req.accuracy = NumericAccuracy::Absolute(MathValue::integer(1));
    assert_eq!(
        certify_computation(&graph, &req),
        Err(NumericEngineRefusal::AccuracyUnproved)
    );
}

#[test]
fn operational_graph_keeps_nan_payloads_signed_zero_and_comparison_flags() {
    let literal = |bits| NumericComputationNode {
        semantic_kind: crate::F64_INFO_ID.to_string(),
        unit: None,
        reference_unit: None,
        operation: NumericComputationOperation::IeeeLiteral { bits },
    };
    let graph = |a, b, operation, kind: &str| {
        NumericComputation::new(
            vec![
                literal(a),
                literal(b),
                NumericComputationNode {
                    semantic_kind: kind.to_string(),
                    unit: None,
                    reference_unit: None,
                    operation: NumericComputationOperation::Apply {
                        operation,
                        operands: vec![0, 1],
                    },
                },
            ],
            2,
        )
        .unwrap()
    };
    let mut req = requirement(NumericRepresentation::IeeeBinary64);
    req.semantics = NumericSemantics::IeeeOperational;
    req.accuracy = NumericAccuracy::Exact;
    let zeros = graph(1 << 63, 0, NumericOperation::Equal, crate::BOOL_INFO_ID);
    let witness = certify_computation(&zeros, &req).unwrap();
    assert_eq!(witness.claim().result.boolean(), Some(true));
    assert_eq!(witness.claim().steps[0].output, 1 << 63);
    assert_eq!(witness.claim().boolean_steps.len(), 1);
    let signaling = graph(
        0x7ff0_0000_0000_0123,
        0,
        NumericOperation::Add,
        crate::F64_INFO_ID,
    );
    assert_eq!(
        certify_computation(&signaling, &req),
        Err(NumericEngineRefusal::ExceptionPolicy)
    );
    req.exceptions.invalid = true;
    let witness = certify_computation(&signaling, &req).unwrap();
    assert_eq!(
        witness.claim().result,
        NumericResultEvidence::IeeeBits(Binary64Value::from_bits(0x7ff8_0000_0000_0123))
    );
    assert!(witness.claim().flags.invalid);
    let quiet = graph(
        0x7ff8_0000_0000_0123,
        0,
        NumericOperation::Equal,
        crate::BOOL_INFO_ID,
    );
    let witness = certify_computation(&quiet, &req).unwrap();
    assert_eq!(witness.claim().result.boolean(), Some(false));
    assert!(!witness.claim().flags.invalid);
    let less = graph(
        0x7ff8_0000_0000_0123,
        0,
        NumericOperation::Less,
        crate::BOOL_INFO_ID,
    );
    assert!(
        certify_computation(&less, &req)
            .unwrap()
            .claim()
            .flags
            .invalid
    );
}

#[test]
fn irrational_root_rounding_has_certified_error_and_proved_ulp_reference() {
    let graph = NumericComputation::new(
        vec![
            NumericComputationNode::literal(
                crate::MATH_VALUE_INFO_ID.to_string(),
                None,
                &MathValue::integer(2),
            )
            .unwrap(),
            NumericComputationNode {
                semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
                unit: None,
                reference_unit: None,
                operation: NumericComputationOperation::Apply {
                    operation: NumericOperation::SquareRoot,
                    operands: vec![0],
                },
            },
        ],
        1,
    )
    .unwrap();
    let mut req = requirement(NumericRepresentation::Binary64);
    req.accuracy =
        NumericAccuracy::Absolute(MathValue::rational("1", "10000000000000000").unwrap());
    let witness = certify_computation(&graph, &req).unwrap();
    assert_eq!(witness.claim().steps[1].output, 0x3ff6_a09e_667f_3bcd);
    assert_eq!(
        witness
            .claim()
            .error
            .lower()
            .compare(witness.claim().error.upper())
            .unwrap(),
        Some(core::cmp::Ordering::Less)
    );
    req.accuracy = NumericAccuracy::Ulps(0);
    certify_computation(&graph, &req).unwrap();
    let mut cancellation_req = requirement(NumericRepresentation::Binary64);
    cancellation_req.accuracy = NumericAccuracy::Ulps(1);
    assert_eq!(
        certify_computation(&cancellation(), &cancellation_req),
        Err(NumericEngineRefusal::AccuracyUnproved)
    );
}

#[test]
fn automatic_candidate_proofs_retain_original_requirement_and_check_native_overflow() {
    let graph = cancellation();
    let mut req = requirement(NumericRepresentation::Automatic);
    req.accuracy = NumericAccuracy::Exact;
    req.budget.limbs = 1;
    let native =
        certify_computation_for_algorithm(&graph, &req, NumericAlgorithm::OrderedCheckedI64V1)
            .unwrap();
    assert_eq!(native.requirement(), &req);
    assert_eq!(*native.requirement_identity(), req.identity().unwrap());
    assert_eq!(
        native.claim().result.mathematical(),
        Some(&MathValue::integer(1))
    );
    assert_eq!(native.claim().integer_steps.len(), 4);
    verify_encoded_computation_claim(&native.claim().canonical_bytes().unwrap(), &graph, &req)
        .unwrap();
    let literal = |n| {
        NumericComputationNode::literal(
            crate::MATH_VALUE_INFO_ID.to_string(),
            None,
            &MathValue::integer(n),
        )
        .unwrap()
    };
    let overflow = NumericComputation::new(
        vec![
            literal(i64::MAX),
            literal(1),
            NumericComputationNode {
                semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
                unit: None,
                reference_unit: None,
                operation: NumericComputationOperation::Apply {
                    operation: NumericOperation::Add,
                    operands: vec![0, 1],
                },
            },
        ],
        2,
    )
    .unwrap();
    assert_eq!(
        certify_computation_for_algorithm(&overflow, &req, NumericAlgorithm::OrderedCheckedI64V1),
        Err(NumericEngineRefusal::IntegerOverflow)
    );
    req.budget.limbs = 128;
    certify_computation_for_algorithm(&overflow, &req, NumericAlgorithm::OrderedRationalV1)
        .unwrap();
    let pi = NumericComputation::new(
        vec![NumericComputationNode::literal(
            crate::MATH_VALUE_INFO_ID.to_string(),
            None,
            &MathValue::pi(),
        )
        .unwrap()],
        0,
    )
    .unwrap();
    let symbolic =
        certify_computation_for_algorithm(&pi, &req, NumericAlgorithm::SymbolicRetentionV1)
            .unwrap();
    assert_eq!(
        symbolic.claim().result.mathematical(),
        Some(&MathValue::pi())
    );
    assert_eq!(
        symbolic.claim().algorithm,
        NumericAlgorithm::SymbolicRetentionV1
    );
    verify_encoded_computation_claim(&symbolic.claim().canonical_bytes().unwrap(), &pi, &req)
        .unwrap();
}

#[test]
fn symbolic_physical_result_retains_immutable_unit_and_role() {
    let unit = crate::builtin_angle_unit();
    let node = NumericComputationNode::literal(
        crate::ANGLE_INFO_ID.to_string(),
        Some(alloc::boxed::Box::new(unit.clone())),
        &MathValue::rational("1", "4").unwrap(),
    )
    .unwrap();
    let graph = NumericComputation::new(vec![node], 0).unwrap();
    let mut req = requirement(NumericRepresentation::Symbolic);
    req.accuracy = NumericAccuracy::Exact;
    let witness = certify_computation(&graph, &req).unwrap();
    let NumericResultEvidence::ExactPhysical(value) = &witness.claim().result else {
        panic!("physical result lost")
    };
    assert_eq!(value.unit().encoded(), unit.encoded());
    assert_eq!(
        value.coordinate().to_owned().unwrap(),
        MathValue::rational("1", "4").unwrap()
    );
    let encoded = witness.claim().canonical_bytes().unwrap();
    verify_encoded_computation_claim(&encoded, &graph, &req).unwrap();
}

#[test]
fn physical_cardinal_trig_and_symbolic_literal_rounding_are_verified() {
    let angle = crate::builtin_angle_unit();
    let literal = NumericComputationNode::literal(
        crate::ANGLE_INFO_ID.into(),
        Some(alloc::boxed::Box::new(angle.clone())),
        &MathValue::rational("1", "4").unwrap(),
    )
    .unwrap();
    let sin = NumericComputationNode {
        semantic_kind: crate::MATH_VALUE_INFO_ID.into(),
        unit: None,
        reference_unit: Some(alloc::boxed::Box::new(angle)),
        operation: NumericComputationOperation::Apply {
            operation: NumericOperation::Sin,
            operands: vec![0],
        },
    };
    let graph = NumericComputation::new(vec![literal, sin], 1).unwrap();
    let mut req = requirement(NumericRepresentation::Symbolic);
    req.accuracy = NumericAccuracy::Exact;
    let witness = certify_computation(&graph, &req).unwrap();
    assert_eq!(witness.claim().result.mathematical(), Some(&MathValue::ONE));
    assert!(witness.required_memory_reservation_bytes(&graph).unwrap() < 1048576);
    let output = PreparedNumericOutput::new(&graph, witness).unwrap();
    assert_eq!(
        MathValue::decode(output.canonical()).unwrap(),
        MathValue::ONE
    );
    let pi = NumericComputation::new(
        vec![NumericComputationNode::literal(
            crate::MATH_VALUE_INFO_ID.into(),
            None,
            &MathValue::pi(),
        )
        .unwrap()],
        0,
    )
    .unwrap();
    let mut req = requirement(NumericRepresentation::Binary64);
    req.accuracy = NumericAccuracy::Ulps(0);
    let witness = certify_computation(&pi, &req).unwrap();
    assert_eq!(witness.claim().steps[0].output, 0x4009_21fb_5444_2d18);
    assert!(witness.claim().flags.inexact);
    let mut forged = witness.claim().clone();
    forged.engine_identity[0] ^= 1;
    assert_eq!(
        verify_computation_claim(&forged, &pi, &req),
        Err(NumericEngineRefusal::ClaimMismatch)
    );
}
