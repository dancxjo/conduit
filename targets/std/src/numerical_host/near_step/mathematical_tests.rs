//! Producer and independent receiver agree on the original physical source.
use super::*;
fn frame() -> Vec<u8> {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    let conversion = PreparedAngleReferenceConversion::from_checked(
        Quantity::parse_plot_literal("90°").unwrap(),
        Unit::Radian,
        "90°",
        "rad",
        &mut work,
    )
    .unwrap();
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = conversion.denotation().encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Radian,
        QuantityRole::Linear,
        &mut bytes,
        &mut work,
    )
    .unwrap();
    bytes.truncate(n);
    bytes
}
fn receiver(limit: Quantity) -> PreparedMathematicalNearQuantityVerifier {
    PreparedMathematicalNearQuantityVerifier::new(
        &conduit_semantic_catalog::near_quantity_type(),
        mathematical::requirements(),
        Binary32Rounding::NearestTiesEven,
        RationalQuantity::from_quantity(limit, &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK))
            .unwrap(),
        900_000_000,
    )
    .unwrap()
}
#[test]
fn mathematical_near_result_is_independently_recomputed_without_execution_growth() {
    let bytes = frame();
    let requested = Quantity::parse_plot_literal("0.000001rad").unwrap();
    let mut back =
        NearQuantityBack::new(requested, NearQuantityAlgorithm::EnclosedBinary32).unwrap();
    let mut verifier = receiver(requested);
    let capacity = back.retained_capacity_bytes();
    let receiver_capacity = verifier.prepared_heap_bytes();
    let digest =
        MathematicalQuantityView::decode(&bytes, &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK))
            .unwrap()
            .representation_digest();
    let guard = crate::allocation_probe::begin();
    for _ in 0..4 {
        back.evaluate(&bytes).unwrap();
        let verified = verifier
            .verify(
                back.state.encoding.encoded(),
                ANGLE_INFO_ID,
                Some(digest),
                &mut ExactNumericWork::new(900_000_000),
            )
            .unwrap();
        assert_eq!(verified.source().encoded(), bytes);
        assert_eq!(verified.source().unit(), Unit::Radian);
        assert_eq!(verified.source().role(), QuantityRole::Linear);
        assert_eq!(verified.source_wire_digest(), digest);
        assert_ne!(verified.maximum_error(), ExactRational::ZERO);
    }
    assert_eq!(guard.finish(), 0);
    assert_eq!(back.retained_capacity_bytes(), capacity);
    assert_eq!(verifier.prepared_heap_bytes(), receiver_capacity);
    let old =
        PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type()).unwrap();
    assert!(matches!(
        old.verify(
            back.state.encoding.encoded(),
            ANGLE_INFO_ID,
            RationalQuantity::from_quantity(
                requested,
                &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK)
            )
            .unwrap(),
            None,
            &mut ExactNumericWork::new(900_000_000)
        ),
        Err(NearQuantityEvidenceRefusal::UnsupportedRepresentation)
    ));
}
fn replace(value: &StructuredInfoValue, path: &[&str], bytes: &[u8]) -> StructuredInfoValue {
    if path.is_empty() {
        return StructuredInfoValue::leaf(value.value_type().clone(), bytes.to_vec()).unwrap();
    }
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => StructuredInfoValue::record(
            value.value_type().clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        if f.name() == path[0] {
                            replace(f.value(), &path[1..], bytes)
                        } else {
                            f.value().clone()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoValueShape::Variant { tag, payload } if tag == path[0] => {
            StructuredInfoValue::variant(
                value.value_type().clone(),
                tag,
                replace(payload, &path[1..], bytes),
            )
            .unwrap()
        }
        _ => panic!("fixture path"),
    }
}
#[test]
fn canonical_forgery_pin_work_and_consumer_tolerance_refusals_preserve_recovery() {
    let bytes = frame();
    let requested = Quantity::parse_plot_literal("0.000001rad").unwrap();
    let mut back =
        NearQuantityBack::new(requested, NearQuantityAlgorithm::EnclosedBinary32).unwrap();
    back.evaluate(&bytes).unwrap();
    let encoded = back.state.encoding.encoded();
    let original = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
    let mut zero = vec![0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = MathematicalValue::rational(ExactRational::ZERO)
        .encode(&mut zero)
        .unwrap();
    zero.truncate(n);
    let mut verifier = receiver(requested);
    for (path, changed) in [
        (vec!["source"], zero.clone()),
        (vec!["maximum-error"], zero),
        (vec!["unit"], Unit::Degree.encode().to_vec()),
        (
            vec!["representation", "enclosed_binary32", "target"],
            0u32.to_le_bytes().to_vec(),
        ),
    ] {
        let changed = replace(&original, &path, &changed)
            .canonical_bytes()
            .unwrap();
        assert!(verifier
            .verify(
                &changed,
                ANGLE_INFO_ID,
                None,
                &mut ExactNumericWork::new(900_000_000)
            )
            .is_err());
    }
    assert!(matches!(
        verifier.verify(
            encoded,
            ANGLE_INFO_ID,
            Some([0; 32]),
            &mut ExactNumericWork::new(900_000_000)
        ),
        Err(NearQuantityEvidenceRefusal::SourceMismatch)
    ));
    assert!(matches!(
        verifier.verify(encoded, ANGLE_INFO_ID, None, &mut ExactNumericWork::new(0)),
        Err(NearQuantityEvidenceRefusal::WorkLimit)
    ));
    assert!(matches!(
        receiver(Quantity::parse_plot_literal("0rad").unwrap()).verify(
            encoded,
            ANGLE_INFO_ID,
            None,
            &mut ExactNumericWork::new(900_000_000)
        ),
        Err(NearQuantityEvidenceRefusal::InsufficientTolerance)
    ));
    assert!(verifier
        .verify(
            encoded,
            ANGLE_INFO_ID,
            None,
            &mut ExactNumericWork::new(900_000_000)
        )
        .is_ok());
}

#[test]
fn prepared_symbolic_value_contract_enforces_tolerance_other_predicates_and_recovery() {
    let bytes = frame();
    let requested = Quantity::parse_plot_literal("0.000001rad").unwrap();
    let mut back =
        NearQuantityBack::new(requested, NearQuantityAlgorithm::EnclosedBinary32).unwrap();
    back.evaluate(&bytes).unwrap();
    let encoded = back.state.encoding.encoded();
    let schema = conduit_semantic_catalog::near_quantity_type();
    let contract = |limit: &str, minimum: u32| {
        CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
            vec![
                ValueConstraint::ByteLength {
                    minimum,
                    maximum: NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
                },
                ValueConstraint::NearQuantity {
                    source_kind: kind_id(ANGLE_INFO_ID),
                    limit: Box::new(Quantity::parse_plot_literal(limit).unwrap()),
                    schema: schema.canonical_bytes().unwrap(),
                },
            ],
        )
        .unwrap()
    };
    let authored = contract("0.000001rad", 0);
    let prepare = |c: &CheckedValueContract| {
        PreparedMathematicalNearValueContract::new(
            c,
            mathematical::requirements(),
            Binary32Rounding::NearestTiesEven,
            900_000_000,
        )
        .unwrap()
    };
    let mut valid = prepare(&authored);
    let mut limited = PreparedMathematicalNearValueContract::new(
        &authored,
        mathematical::requirements(),
        Binary32Rounding::NearestTiesEven,
        MATHEMATICAL_NEAR_VALIDATION_WORK,
    )
    .unwrap();
    let mut strict = prepare(&contract("0.00000000001rad", 0));
    let mut length = prepare(&contract("0.000001rad", encoded.len() as u32 + 1));
    assert_eq!(valid.contract(), &authored);
    assert!(authored.validate(encoded).is_err());
    let capacity = valid.prepared_verifier_heap_bytes();
    assert!(capacity > 0);
    assert_eq!(valid.maximum_numerical_work(), 900_000_000);
    let original = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
    let forged = replace(&original, &["unit"], &Unit::Degree.encode())
        .canonical_bytes()
        .unwrap();
    let guard = crate::allocation_probe::begin();
    for _ in 0..4 {
        let mut generous = ExactNumericWork::new(u64::MAX);
        assert_eq!(
            limited.validate(encoded, &mut generous),
            Err(ValueConstraintRefusal::NearQuantity(
                NearQuantityEvidenceRefusal::WorkLimit
            ))
        );
        assert!(generous.spent() <= MATHEMATICAL_NEAR_VALIDATION_WORK);
        let mut empty = ExactNumericWork::new(0);
        assert_eq!(
            valid.validate(encoded, &mut empty),
            Err(ValueConstraintRefusal::NearQuantity(
                NearQuantityEvidenceRefusal::WorkLimit
            ))
        );
        assert_eq!(empty.spent(), 0);
        assert_eq!(
            strict.validate(encoded, &mut ExactNumericWork::new(900_000_000)),
            Err(ValueConstraintRefusal::NearQuantity(
                NearQuantityEvidenceRefusal::InsufficientTolerance
            ))
        );
        assert_eq!(
            length.validate(encoded, &mut ExactNumericWork::new(900_000_000)),
            Err(ValueConstraintRefusal::ByteLength)
        );
        assert!(valid
            .validate(&forged, &mut ExactNumericWork::new(900_000_000))
            .is_err());
        assert_eq!(
            valid.validate(encoded, &mut ExactNumericWork::new(900_000_000)),
            Ok(())
        );
    }
    assert_eq!(guard.finish(), 0);
    assert_eq!(valid.prepared_verifier_heap_bytes(), capacity);
    assert_eq!(valid.contract(), &authored);
}
