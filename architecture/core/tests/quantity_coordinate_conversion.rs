//! Actual bounded conversion of existing Quantity wires; no Host/Play claim.
use conduit_core::projection::*;
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn ratio(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn rational() -> Vec<u8> {
    let q = RationalQuantity::new(ratio(1, 3), Unit::Meter, QuantityRole::Linear).unwrap();
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(q, &mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn symbolic() -> Vec<u8> {
    let v = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let mut b = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = v.encode(&mut b).unwrap();
    let view = MathematicalValueView::decode(&b[..n], &mut work()).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Meter,
        QuantityRole::Linear,
        &mut output,
        &mut work(),
    )
    .unwrap();
    output.truncate(n);
    output
}

fn report_input<'a, D: ProjectionDomain>(
    source: &'a D::Source,
    target: &'a D::Target,
    facts: &'a [ProjectionFact<'a, D>],
) -> ProjectionInput<'a, D> {
    let text = |s| ProjectionText::new(s).unwrap();
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("quantity/conversion-report@1"),
            source: text("quantity/input@1"),
            target: Some(text("quantity/output@1")),
            projector: text("physical/exact-unit-coordinates@1"),
            requested_route: text("quantity/conversion@1"),
            boundary: None,
        },
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}

#[test]
fn native_rational_and_symbolic_coordinates_convert_without_growth_or_erasing_source() {
    let native = Quantity::parse_plot_literal("1m")
        .unwrap()
        .encode()
        .to_vec();
    for (source, expected) in [
        (native, QuantityCoordinateRepresentation::Decimal),
        (rational(), QuantityCoordinateRepresentation::Rational),
        (symbolic(), QuantityCoordinateRepresentation::Mathematical),
    ] {
        let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
        let mut verifier = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
        let mut scratch = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
        let retained = converter.retained_payload_bytes();
        let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
        let limits = QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS;
        let mut evidence = vec![0; limits.total_bytes];
        let domain = QuantityCoordinateConversionDomain::new();
        let policy = QuantityCoordinateConversionPolicy {
            target_unit: Unit::Centimeter,
        };
        let codec = QuantityCoordinateConversionEvidenceCodec::new(policy);
        let (_, allocations) = observe(|| {
            for _ in 0..3 {
                let mut meter = work();
                let receipt = converter
                    .execute(&source, Unit::Centimeter, &mut output, &mut meter)
                    .unwrap();
                assert_eq!(receipt.source().encoded(), source);
                assert_eq!(receipt.target_representation(), expected);
                assert_eq!(receipt.scale(), ExactRational::from_i128(100));
                assert_eq!(receipt.offset(), ExactRational::ZERO);
                assert_eq!(receipt.spent_work(), meter.spent());
                assert_eq!(receipt.admitted_work(), 200_000_000);
                let bytes = &output[..receipt.encoded_bytes()];
                let view = QuantityValueProfile::Mathematical
                    .decode(QUANTITY_INFO_ID, bytes, &mut work())
                    .unwrap();
                assert_eq!(view.unit(), Unit::Centimeter);
                assert_eq!(view.role(), QuantityRole::Linear);
                assert_eq!(
                    view.representation_digest(),
                    receipt.target_representation_digest()
                );
                let facts = domain.facts(&receipt);
                let report =
                    ProjectionReport::new(&domain, &policy, report_input(&receipt, &view, &facts))
                        .unwrap();
                report
                    .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
                    .unwrap();
                let n = encode_projection_evidence(&report, &codec, limits, &mut evidence).unwrap();
                let decoded = inspect_projection_evidence(&evidence[..n]).unwrap();
                assert_eq!(&decoded.source_bytes[4..4 + source.len()], &source);
                assert_eq!(&decoded.target_bytes.unwrap()[4..], bytes);
                verify_projection_evidence(&report, &codec, limits, &evidence[..n]).unwrap();
                let basis = report_input(&receipt, &view, &facts).basis;
                verifier
                    .verify_evidence(
                        &source,
                        bytes,
                        policy,
                        200_000_000,
                        basis,
                        &evidence[..n],
                        &mut scratch,
                        &mut ExactNumericWork::new(300_000_000),
                    )
                    .unwrap();
                assert!(verifier
                    .verify_evidence(
                        &source,
                        bytes,
                        policy,
                        199_999_999,
                        basis,
                        &evidence[..n],
                        &mut scratch,
                        &mut ExactNumericWork::new(300_000_000),
                    )
                    .is_err());
                for (claimed_target, claimed_policy, claimed_basis, claimed_evidence) in [
                    (source.as_slice(), policy, basis, &evidence[..n]),
                    (
                        bytes,
                        QuantityCoordinateConversionPolicy {
                            target_unit: Unit::Meter,
                        },
                        basis,
                        &evidence[..n],
                    ),
                    (
                        bytes,
                        policy,
                        ProjectionBasis {
                            source: ProjectionText::new("quantity/other-input@1").unwrap(),
                            ..basis
                        },
                        &evidence[..n],
                    ),
                    (bytes, policy, basis, &evidence[..0]),
                ] {
                    assert!(verifier
                        .verify_evidence(
                            &source,
                            claimed_target,
                            claimed_policy,
                            200_000_000,
                            claimed_basis,
                            claimed_evidence,
                            &mut scratch,
                            &mut ExactNumericWork::new(300_000_000),
                        )
                        .is_err());
                }
                evidence[n - 1] ^= 1;
                assert!(verifier
                    .verify_evidence(
                        &source,
                        bytes,
                        policy,
                        200_000_000,
                        basis,
                        &evidence[..n],
                        &mut scratch,
                        &mut ExactNumericWork::new(300_000_000),
                    )
                    .is_err());
                evidence[n - 1] ^= 1;
                if expected == QuantityCoordinateRepresentation::Mathematical {
                    let symbolic = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
                    assert!(symbolic
                        .coordinate()
                        .as_rational(&mut work())
                        .unwrap()
                        .is_none());
                } else {
                    let roundtrip = view
                        .rational(&mut work())
                        .unwrap()
                        .convert_to_unit(Unit::Meter, &mut work())
                        .unwrap();
                    assert_eq!(
                        roundtrip.coordinate(),
                        if expected == QuantityCoordinateRepresentation::Decimal {
                            ExactRational::ONE
                        } else {
                            ratio(1, 3)
                        }
                    );
                }
                assert_eq!(converter.retained_payload_bytes(), retained);
            }
        });
        assert_eq!(allocations, 0);
    }
}

#[test]
fn native_exponent_boundary_preserves_native_coordinates() {
    let source =
        Quantity::from_decimal_role(1, QUANTITY_MAX_EXPONENT, Unit::Meter, QuantityRole::Linear)
            .unwrap();
    let bytes = source.encode();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let (_, allocations) = observe(|| {
        let receipt = converter
            .execute(&bytes, Unit::Centimeter, &mut output, &mut work())
            .unwrap();
        assert_eq!(
            receipt.target_representation(),
            QuantityCoordinateRepresentation::Decimal
        );
        let value = Quantity::decode(&output[..receipt.encoded_bytes()]).unwrap();
        assert_eq!(value.convert(Unit::Meter).unwrap(), source);
    });
    assert_eq!(allocations, 0);
}

#[test]
fn exact_rational_result_survives_native_coordinate_overflow() {
    let source = Quantity::from_decimal_role(
        10_i128.pow(QUANTITY_MAX_SIGNIFICANT_DIGITS as u32) - 1,
        QUANTITY_MAX_EXPONENT,
        Unit::Meter,
        QuantityRole::Linear,
    )
    .unwrap();
    assert_eq!(
        source.convert(Unit::Centimeter),
        Err(QuantityConversionRefusal::Overflow)
    );
    let bytes = source.encode();
    let expected = RationalQuantity::from_quantity(source, &mut work()).unwrap();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut evidence = vec![0; QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS.total_bytes];
    let domain = QuantityCoordinateConversionDomain::new();
    let policy = QuantityCoordinateConversionPolicy {
        target_unit: Unit::Centimeter,
    };
    let codec = QuantityCoordinateConversionEvidenceCodec::new(policy);
    let (_, allocations) = observe(|| {
        let receipt = converter
            .execute(&bytes, Unit::Centimeter, &mut output, &mut work())
            .unwrap();
        assert_eq!(
            receipt.target_representation(),
            QuantityCoordinateRepresentation::Rational
        );
        let target =
            QuantityValueView::decode(&output[..receipt.encoded_bytes()], &mut work()).unwrap();
        let value = target
            .rational(&mut work())
            .unwrap()
            .convert_to_unit(Unit::Meter, &mut work())
            .unwrap();
        assert_eq!(value, expected);
        let facts = domain.facts(&receipt);
        let report =
            ProjectionReport::new(&domain, &policy, report_input(&receipt, &target, &facts))
                .unwrap();
        report
            .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
            .unwrap();
        let limits = QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS;
        let n = encode_projection_evidence(&report, &codec, limits, &mut evidence).unwrap();
        verify_projection_evidence(&report, &codec, limits, &evidence[..n]).unwrap();
        let mut wrong = domain.facts(&receipt);
        wrong[3] = ProjectionFact::Preserved {
            obligation: ProjectionText::new("coordinate-wire-representation").unwrap(),
        };
        assert!(!domain.validate(
            &receipt,
            Some(&target),
            &wrong,
            &[],
            &[],
            ProjectionMechanism::Completed
        ));
        let wrong_codec =
            QuantityCoordinateConversionEvidenceCodec::new(QuantityCoordinateConversionPolicy {
                target_unit: Unit::Meter,
            });
        assert_eq!(
            verify_projection_evidence(&report, &wrong_codec, limits, &evidence[..n]),
            Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn affine_point_and_delta_use_exact_rational_fallback_and_refusal_does_not_publish() {
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    for role in [QuantityRole::Point, QuantityRole::Delta] {
        let source = Quantity::from_decimal_role(1, 0, Unit::Fahrenheit, role).unwrap();
        let input = source.encode();
        let receipt = converter
            .execute(&input, Unit::Kelvin, &mut output, &mut work())
            .unwrap();
        assert_eq!(
            receipt.target_representation(),
            QuantityCoordinateRepresentation::Rational
        );
        let view =
            QuantityValueView::decode(&output[..receipt.encoded_bytes()], &mut work()).unwrap();
        assert_eq!(view.role(), role);
        assert_eq!(
            view.rational(&mut work()).unwrap().coordinate(),
            if role == QuantityRole::Point {
                ratio(46067, 180)
            } else {
                ratio(5, 9)
            }
        );
    }
    output.fill(0xa5);
    let source = Quantity::parse_plot_literal("1m").unwrap().encode();
    assert!(converter
        .execute(&source, Unit::Kelvin, &mut output, &mut work())
        .is_err());
    assert!(output.iter().all(|b| *b == 0xa5));
    let mut short = [0xa5; 1];
    assert!(converter
        .execute(&source, Unit::Centimeter, &mut short, &mut work())
        .is_err());
    assert_eq!(short, [0xa5; 1]);
    assert!(converter
        .execute(&source, Unit::Centimeter, &mut output, &mut work())
        .is_ok());
}

#[test]
fn work_exhaustion_is_charged_without_publishing_and_prepared_converter_recovers() {
    let source = rational();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut successful = work();
    let receipt = converter
        .execute(&source, Unit::Centimeter, &mut output, &mut successful)
        .unwrap();
    let expected = output[..receipt.encoded_bytes()].to_vec();
    let spent = successful.spent();
    assert!(spent > 1);
    for allowance in [0, 1, spent / 2, spent - 1] {
        output.fill(0xa5);
        let mut limited = ExactNumericWork::new(allowance);
        assert!(converter
            .execute(&source, Unit::Centimeter, &mut output, &mut limited)
            .is_err());
        assert!(output.iter().all(|b| *b == 0xa5));
        assert!(limited.spent() <= allowance);
        if allowance > 1 {
            assert!(limited.spent() > 0);
        }
        let mut recovery = work();
        let receipt = converter
            .execute(&source, Unit::Centimeter, &mut output, &mut recovery)
            .unwrap();
        assert_eq!(&output[..receipt.encoded_bytes()], expected);
        assert_eq!(recovery.spent(), spent);
    }
}
