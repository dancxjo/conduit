use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn value(s: &str) -> MathematicalValue {
    MathematicalValue::parse_literal(s, &mut work()).unwrap()
}
fn limits() -> Binary64AlgorithmLimits {
    Binary64AlgorithmLimits {
        maximum_steps: 32,
        maximum_retained_bytes: 1_000_000,
    }
}
#[test]
fn exact_and_binary64_share_source_and_precision_is_checked_before_admission() {
    let source = value("0.3");
    let algorithm = NumericalAlgorithm::Binary64 {
        rounding: Binary64Rounding::NearestTiesEven,
    };
    let float =
        PreparedNumericalRealization::prepare(&source, algorithm, ExactRational::ONE, &mut work())
            .unwrap();
    let exact = PreparedNumericalRealization::prepare(
        &source,
        NumericalAlgorithm::ExactRational,
        ExactRational::ZERO,
        &mut work(),
    )
    .unwrap();
    assert_eq!(float.source(), exact.source());
    let NumericalTarget::Binary64(program) = float.target() else {
        panic!("IEEE target");
    };
    assert_eq!(program.target().bits(), 0x3fd3333333333333);
    let strict = NumericalRealizationRequest::new(source.clone(), ExactRational::ZERO).unwrap();
    assert_eq!(
        float.admits(&strict, &mut work()),
        Err(NumericalRealizationRefusal::ToleranceExceeded)
    );
    assert!(exact.admits(&strict, &mut work()).is_ok());
    assert_eq!(
        PreparedNumericalRealization::from_property(&float.property().unwrap(), &mut work())
            .unwrap(),
        float
    );
}
#[test]
fn property_retains_complete_operation_order_and_rechecks_cancellation_error() {
    let a = ExactRational::parse_decimal("10000000000000000", &mut work()).unwrap();
    let ops = [
        Binary64Operation::Constant(Box::new(a)),
        Binary64Operation::Constant(Box::new(ExactRational::ONE)),
        Binary64Operation::Constant(Box::new(a)),
        Binary64Operation::Add { left: 0, right: 1 },
        Binary64Operation::Subtract { left: 3, right: 2 },
    ];
    let prepared = PreparedNumericalRealization::prepare_binary64(
        &value("1"),
        &ops,
        Binary64Rounding::NearestTiesEven,
        ExactRational::ONE,
        limits(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(prepared.maximum_error(), ExactRational::ONE);
    let read =
        PreparedNumericalRealization::from_property(&prepared.property().unwrap(), &mut work())
            .unwrap();
    assert_eq!(read, prepared);
    let NumericalTarget::Binary64(trace) = read.target() else {
        panic!("trace");
    };
    assert_eq!(trace.target().bits(), 0);
    assert_eq!(trace.steps().len(), 5);
    assert_eq!(
        trace
            .steps()
            .iter()
            .map(|s| s.operation().clone())
            .collect::<Vec<_>>(),
        ops
    );
    assert!(trace.steps()[3].projection().status().inexact);
    assert!(prepared
        .admits(
            &NumericalRealizationRequest::new(value("1"), ExactRational::ZERO).unwrap(),
            &mut work()
        )
        .is_err());
    assert_eq!(
        PreparedNumericalRealization::prepare_binary64(
            &value("2"),
            &ops,
            Binary64Rounding::NearestTiesEven,
            ExactRational::ONE,
            limits(),
            &mut work()
        ),
        Err(NumericalRealizationRefusal::SourceMismatch)
    );
    assert!(PreparedNumericalRealization::from_property(
        &prepared.property().unwrap(),
        &mut ExactNumericWork::new(1)
    )
    .is_err());
}
#[test]
fn ieee_signed_zero_survives_property_and_forged_target_or_error_kind_refuse() {
    let p = PreparedNumericalRealization::prepare_binary64(
        &value("0"),
        &[Binary64Operation::IeeeConstant(IeeeF64::from_bits(1 << 63))],
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    let property = p.property().unwrap();
    let read = PreparedNumericalRealization::from_property(&property, &mut work()).unwrap();
    let NumericalTarget::Binary64(trace) = read.target() else {
        panic!();
    };
    assert_eq!(trace.target().bits(), 1 << 63);
    let v = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = v.shape() else {
        panic!();
    };
    for offset in [1, 8] {
        let mut forged = bytes.to_vec();
        let index = forged.len() - offset;
        forged[index] ^= 1;
        let v = StructuredInfoValue::leaf(v.value_type().clone(), forged).unwrap();
        let property = StructuredConfigurationValue::new(
            property.profile().clone(),
            v.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert!(PreparedNumericalRealization::from_property(&property, &mut work()).is_err());
    }
}

#[test]
fn failed_recomputation_spends_the_outer_verifiers_actual_arithmetic_budget() {
    let prepared = PreparedNumericalRealization::prepare(
        &value("0.3"),
        NumericalAlgorithm::Binary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        },
        ExactRational::ONE,
        &mut work(),
    )
    .unwrap();
    let property = prepared.property().unwrap();
    let leaf = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = leaf.shape() else {
        panic!();
    };
    // Replace the admitted error limit with zero while keeping the rest of the
    // hostile certificate. Its arithmetic must run, then reject the tolerance.
    let source_length = u16::from_le_bytes(bytes[19..21].try_into().unwrap()) as usize;
    let start = 21 + source_length;
    let old_length = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()) as usize;
    let mut zero = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = ExactRational::ZERO.encode(&mut zero).unwrap();
    let mut forged = bytes[..start].to_vec();
    forged.extend_from_slice(&(length as u16).to_le_bytes());
    forged.extend_from_slice(&zero[..length]);
    forged.extend_from_slice(&bytes[start + 2 + old_length..]);
    let leaf = StructuredInfoValue::leaf(leaf.value_type().clone(), forged).unwrap();
    let property = StructuredConfigurationValue::new(
        property.profile().clone(),
        leaf.canonical_bytes().unwrap(),
    )
    .unwrap();
    let mut verification = work();
    assert!(PreparedNumericalRealization::from_property(&property, &mut verification).is_err());
    assert!(
        verification.spent() >= prepared.spent_limb_work() / 2,
        "rejected recomputation must be charged, not just the encoded-input parsing"
    );
}
