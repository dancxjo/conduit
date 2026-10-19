//! Actual canonical Near record replay reproduces the producer's full receipt.
use super::*;
fn leaf(id: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(id)).unwrap(),
        bytes.to_vec(),
    )
    .unwrap()
}
fn record(id: &str, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    let ty = StructuredInfoType::record(
        kind_id(id),
        fields
            .iter()
            .map(|(n, v)| StructuredFieldType::new(*n, v.value_type().clone()).unwrap())
            .collect(),
    )
    .unwrap();
    StructuredInfoValue::record(
        ty,
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn cases(id: &str, names: &[&str]) -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id(id),
        names
            .iter()
            .map(|name| {
                StructuredVariantCase::new(
                    *name,
                    StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn tag(ty: &StructuredInfoType, name: &str) -> StructuredInfoValue {
    StructuredInfoValue::variant(ty.clone(), name, leaf(EMPTY_INFO_ID, &[])).unwrap()
}
fn number(value: ExactRational) -> StructuredInfoValue {
    let mut bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = MathematicalValue::rational(value)
        .encode(&mut bytes)
        .unwrap();
    leaf(MATHEMATICAL_VALUE_ID, &bytes[..n])
}
fn near(
    receipt: &BorrowedMathematicalQuantityBinary32<'_>,
    requested: Quantity,
) -> StructuredInfoValue {
    let rounding = cases(
        "test/rounding",
        &[
            "nearest_ties_even",
            "toward_zero",
            "toward_positive",
            "toward_negative",
        ],
    );
    let physical = RationalQuantity::new(
        ExactRational::ZERO,
        receipt.source().unit(),
        receipt.source().role(),
    )
    .unwrap();
    let ids = NearProjectionContentIds::new(
        receipt.source().encoded(),
        physical,
        requested,
        Some(receipt.midpoint()),
    );
    let domain = MathematicalQuantityBinary32Domain::new();
    let policy = MathematicalQuantityBinary32Policy {
        maximum_error: receipt.maximum_error(),
        rounding: receipt.midpoint().numeric().rounding(),
    };
    let target = receipt.target();
    let facts = receipt.facts();
    let report = ProjectionReport::new(
        &domain,
        &policy,
        ids.input(&domain, receipt, &target, &facts),
    )
    .unwrap();
    let mut evidence = vec![0; MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES];
    let n = encode_projection_evidence(
        &report,
        &MathematicalQuantityBinary32EvidenceCodec::new(policy),
        MATHEMATICAL_NEAR_PROJECTION_EVIDENCE_LIMITS,
        &mut evidence,
    )
    .unwrap();
    evidence.truncate(n);
    let float = record(
        "test/binary32",
        vec![
            ("projection-evidence", leaf(BYTES_INFO_ID, &evidence)),
            (
                "target",
                leaf(
                    F32_INFO_ID,
                    &receipt.target().coordinate().bits().to_le_bytes(),
                ),
            ),
            ("rounding", tag(&rounding, "nearest_ties_even")),
        ],
    );
    let rep = StructuredInfoType::variant(
        kind_id("test/representation"),
        vec![
            StructuredVariantCase::new(
                "exact_rational",
                StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID)).unwrap(),
            )
            .unwrap(),
            StructuredVariantCase::new("enclosed_binary32", float.value_type().clone()).unwrap(),
        ],
    )
    .unwrap();
    let role = cases("test/role", &["linear", "point", "delta"]);
    let claim = cases("test/claim", &["exact_distance", "certified_bound"]);
    record(
        "test/near",
        vec![
            (
                "source",
                leaf(
                    MATHEMATICAL_VALUE_ID,
                    receipt.source().coordinate().encoded(),
                ),
            ),
            (
                "source-wire",
                leaf(BYTES_INFO_ID, receipt.source().encoded()),
            ),
            (
                "requested-limit",
                leaf(QUANTITY_INFO_ID, &requested.encode()),
            ),
            (
                "unit",
                leaf(UNIT_INFO_ID, &receipt.source().unit().encode()),
            ),
            ("role", tag(&role, "linear")),
            ("maximum-error", number(receipt.error().maximum())),
            ("limit", number(receipt.maximum_error().coordinate())),
            ("claim", tag(&claim, "certified_bound")),
            (
                "representation",
                StructuredInfoValue::variant(rep, "enclosed_binary32", float).unwrap(),
            ),
        ],
    )
}
#[test]
fn near_reader_retains_original_policy_and_projection_work_in_full_report() {
    let requested = Quantity::parse_plot_literal("0.000001rad").unwrap();
    let original = RationalQuantity::from_quantity(requested, &mut work()).unwrap();
    let policy = MathematicalQuantityBinary32Policy {
        maximum_error: original,
        rounding: Binary32Rounding::NearestTiesEven,
    };
    let codec = MathematicalQuantityBinary32EvidenceCodec::new(policy);
    let wire = frame(true, Unit::Radian, QuantityRole::Linear);
    let mut producer = PreparedMathematicalQuantityBinary32::new(
        requirements(),
        policy.rounding,
        original,
        MATHEMATICAL_NEAR_PROJECTION_WORK,
    )
    .unwrap();
    let sent = producer
        .execute(
            &wire,
            &mut ExactNumericWork::new(MATHEMATICAL_NEAR_PROJECTION_WORK),
        )
        .unwrap();
    let value = near(&sent, requested);
    let encoded = value.canonical_bytes().unwrap();
    let bad_requested = near(&sent, Quantity::parse_plot_literal("0rad").unwrap())
        .canonical_bytes()
        .unwrap();
    let mut evidence = vec![0; super::evidence::LIMITS.total_bytes];
    let domain = MathematicalQuantityBinary32Domain::new();
    let target = sent.target();
    let facts = sent.facts();
    let physical =
        RationalQuantity::new(ExactRational::ZERO, Unit::Radian, QuantityRole::Linear).unwrap();
    let ids = NearProjectionContentIds::new(&wire, physical, requested, Some(sent.midpoint()));
    let report =
        ProjectionReport::new(&domain, &policy, ids.input(&domain, &sent, &target, &facts))
            .unwrap();
    let n = encode_projection_evidence(&report, &codec, super::evidence::LIMITS, &mut evidence)
        .unwrap();
    for consumer in [q(1, 2_000_000), q(1, 100_000), ExactRational::ZERO] {
        let limit = RationalQuantity::new(consumer, Unit::Radian, QuantityRole::Linear).unwrap();
        let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
            value.value_type(),
            requirements(),
            policy.rounding,
            limit,
            900_000_000,
        )
        .unwrap();
        let (_, allocations) = observe(|| {
            let result = verifier.verify(
                &encoded,
                ANGLE_INFO_ID,
                None,
                &mut ExactNumericWork::new(900_000_000),
            );
            if consumer == ExactRational::ZERO {
                assert!(matches!(
                    result,
                    Err(NearQuantityEvidenceRefusal::InsufficientTolerance)
                ));
            } else {
                let verified = result.unwrap();
                let replay = verified.projection_receipt();
                assert_eq!(replay.maximum_error(), original);
                let target = replay.target();
                let facts = replay.facts();
                let report = ProjectionReport::new(
                    &domain,
                    &policy,
                    ids.input(&domain, replay, &target, &facts),
                )
                .unwrap();
                verify_projection_evidence(
                    &report,
                    &codec,
                    super::evidence::LIMITS,
                    &evidence[..n],
                )
                .unwrap();
                assert!(verifier
                    .verify(
                        &bad_requested,
                        ANGLE_INFO_ID,
                        None,
                        &mut ExactNumericWork::new(900_000_000)
                    )
                    .is_err());
                assert!(matches!(
                    verifier.verify(
                        &encoded,
                        ANGLE_INFO_ID,
                        None,
                        &mut ExactNumericWork::new(
                            MATHEMATICAL_NEAR_VALIDATION_WORK + MATHEMATICAL_NEAR_PROJECTION_WORK
                                - 1
                        )
                    ),
                    Err(NearQuantityEvidenceRefusal::WorkLimit)
                ));
                verifier
                    .verify(
                        &encoded,
                        ANGLE_INFO_ID,
                        None,
                        &mut ExactNumericWork::new(900_000_000),
                    )
                    .unwrap();
            }
        });
        assert_eq!(allocations, 0);
    }
}
