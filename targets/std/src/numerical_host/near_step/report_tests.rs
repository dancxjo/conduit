//! Every declared rational/native route retains the actual bound report.
use super::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(900_000_000)
}
fn rational_wire(unit: Unit, role: QuantityRole) -> Vec<u8> {
    let source =
        RationalQuantity::new(ExactRational::ratio(1, 3, &mut work()).unwrap(), unit, role)
            .unwrap();
    let mut bytes = vec![0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(source, &mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn report(encoded: &[u8]) -> &[u8] {
    let value = validate_canonical_structured_value(encoded)
        .unwrap()
        .record_field("projection-evidence")
        .unwrap()
        .unwrap();
    &value.value_node()[5..]
}
#[test]
fn every_native_and_rational_route_emits_a_bound_report_without_growth() {
    let high = ExactRational::from_i128(2)
        .checked_power(4095, &mut work())
        .unwrap();
    let coordinate = high
        .checked_divide(
            high.checked_subtract(ExactRational::ONE, &mut work())
                .unwrap(),
            &mut work(),
        )
        .unwrap();
    let wide = RationalQuantity::new(coordinate, Unit::Meter, QuantityRole::Linear).unwrap();
    let mut wide_wire = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(wide, &mut wide_wire).unwrap();
    wide_wire.truncate(n);
    let denominator = high
        .checked_divide(ExactRational::from_i128(2), &mut work())
        .unwrap()
        .checked_multiply(ExactRational::from_i128(3), &mut work())
        .unwrap()
        .checked_subtract(ExactRational::ONE, &mut work())
        .unwrap();
    let bounded = RationalQuantity::new(
        high.checked_divide(denominator, &mut work()).unwrap(),
        Unit::Meter,
        QuantityRole::Linear,
    )
    .unwrap();
    let mut bounded_wire = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(bounded, &mut bounded_wire).unwrap();
    bounded_wire.truncate(n);
    for (case, (wire, request)) in [
        (wide_wire, "0.000001m"),
        (bounded_wire, "0.000001m"),
        (
            Quantity::parse_plot_literal("1m")
                .unwrap()
                .encode()
                .to_vec(),
            "0.000001m",
        ),
        (
            rational_wire(Unit::Meter, QuantityRole::Linear),
            "0.000001m",
        ),
        (
            rational_wire(Unit::Celsius, QuantityRole::Point),
            "TemperatureDelta(0.000001, °C)",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let requested = Quantity::parse_plot_literal(request).unwrap();
        for algorithm in [
            NearQuantityAlgorithm::ExactRational,
            NearQuantityAlgorithm::Binary32,
            NearQuantityAlgorithm::EnclosedBinary32,
        ] {
            let mut back = NearQuantityBack::new(requested, algorithm).unwrap();
            let capacity = back.retained_capacity_bytes();
            let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
                &conduit_semantic_catalog::near_quantity_type(),
                mathematical::requirements(),
                Binary32Rounding::NearestTiesEven,
                RationalQuantity::from_quantity(requested, &mut work()).unwrap(),
                900_000_000,
            )
            .unwrap();
            let guard = crate::allocation_probe::begin();
            back.evaluate(&wire).unwrap();
            let encoded = back.state.encoding.encoded();
            if case == 1 && algorithm != NearQuantityAlgorithm::ExactRational {
                assert_eq!(
                    validate_canonical_structured_value(encoded)
                        .unwrap()
                        .record_field("claim")
                        .unwrap()
                        .unwrap()
                        .variant_tag()
                        .unwrap(),
                    "certified_bound"
                );
            }
            let evidence = report(encoded);
            assert_eq!(&evidence[..4], b"CPE\x01");
            let view = inspect_projection_evidence(evidence).unwrap();
            assert_ne!(*view.configuration_digest, [0; 32]);
            assert_eq!(view.facts().count(), 3);
            verifier
                .verify_admitted(encoded, QUANTITY_INFO_ID, None, &mut work())
                .unwrap();
            assert_eq!(back.retained_capacity_bytes(), capacity);
            assert_eq!(guard.finish(), 0);
        }
    }
}
#[test]
fn changed_native_report_payloads_refuse_even_when_the_outer_record_is_canonical() {
    let wire = rational_wire(Unit::Meter, QuantityRole::Linear);
    let requested = Quantity::parse_plot_literal("0.000001m").unwrap();
    for algorithm in [
        NearQuantityAlgorithm::ExactRational,
        NearQuantityAlgorithm::Binary32,
        NearQuantityAlgorithm::EnclosedBinary32,
    ] {
        let mut back = NearQuantityBack::new(requested, algorithm).unwrap();
        back.evaluate(&wire).unwrap();
        let original = back.state.encoding.encoded();
        let evidence = report(original);
        let start = evidence.as_ptr() as usize - original.as_ptr() as usize;
        let view = inspect_projection_evidence(evidence).unwrap();
        let relative = |bytes: &[u8]| bytes.as_ptr() as usize - evidence.as_ptr() as usize;
        let mut positions = vec![
            3,
            evidence.len() - 1,
            relative(view.source_bytes),
            relative(view.policy_bytes),
        ];
        if let Some(target) = view.target_bytes {
            positions.push(relative(target));
        }
        positions.push(relative(view.source_bytes) + view.source_bytes.len() / 2);
        let verifier =
            PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type())
                .unwrap();
        let limit = RationalQuantity::from_quantity(requested, &mut work()).unwrap();
        for offset in positions {
            let mut forged = original.to_vec();
            forged[start + offset] ^= 1;
            validate_canonical_structured_value(&forged).unwrap();
            assert!(matches!(
                verifier.verify(&forged, QUANTITY_INFO_ID, limit, None, &mut work()),
                Err(NearQuantityEvidenceRefusal::ProjectionEvidence(_))
            ));
        }
    }
}
