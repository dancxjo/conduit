//! Coordinate allowances and affine roles remain separate from numeric rounding.
use super::*;
#[test]
fn converted_allowances_preserve_original_units_and_affine_roles_without_play_allocation() {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    let celsius = Unit::resolve("°C").unwrap();
    let kelvin = Unit::resolve("K").unwrap();
    let meter = Unit::resolve("m").unwrap();
    let cases = [
        (
            RationalQuantity::new(
                ExactRational::ratio(9, 10, &mut work).unwrap(),
                meter,
                QuantityRole::Linear,
            )
            .unwrap(),
            Quantity::parse_plot_literal("1cm").unwrap(),
            Quantity::parse_plot_literal("0.001mm").unwrap(),
        ),
        (
            RationalQuantity::new(
                ExactRational::ratio(1, 3, &mut work).unwrap(),
                celsius,
                QuantityRole::Point,
            )
            .unwrap(),
            Quantity::from_decimal_role(1, -3, kelvin, QuantityRole::Delta).unwrap(),
            Quantity::from_decimal_role(1, -4, celsius, QuantityRole::Delta).unwrap(),
        ),
        (
            RationalQuantity::new(
                ExactRational::ratio(1, 3, &mut work).unwrap(),
                celsius,
                QuantityRole::Delta,
            )
            .unwrap(),
            Quantity::from_decimal_role(1, -3, kelvin, QuantityRole::Delta).unwrap(),
            Quantity::from_decimal_role(1, -4, celsius, QuantityRole::Delta).unwrap(),
        ),
    ];
    for (source, requested, consumer) in cases {
        let mut wire = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
        let n = encode_rational_quantity_value(source, &mut wire).unwrap();
        wire.truncate(n);
        let digest = QuantityValueView::decode(&wire, &mut work)
            .unwrap()
            .representation_digest();
        let consumer = RationalQuantity::from_quantity(consumer, &mut work).unwrap();
        for algorithm in [
            NearQuantityAlgorithm::ExactRational,
            NearQuantityAlgorithm::Binary32,
        ] {
            let mut back = NearQuantityBack::new(requested, algorithm).unwrap();
            let verifier =
                PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type())
                    .unwrap();
            let capacity = back.retained_capacity_bytes();
            let guard = crate::allocation_probe::begin();
            back.evaluate(&wire).unwrap();
            let verified = verifier
                .verify(
                    back.state.encoding.encoded(),
                    QUANTITY_INFO_ID,
                    consumer,
                    Some(digest),
                    &mut ExactNumericWork::new(NEAR_TOTAL_NUMERICAL_WORK),
                )
                .unwrap();
            assert_eq!(guard.finish(), 0);
            assert_eq!(verified.source(), source);
            assert_eq!(verified.source().unit(), source.unit());
            assert_eq!(verified.source().role(), source.role());
            assert_eq!(back.retained_capacity_bytes(), capacity);
            let encoded = back.state.encoding.encoded();
            let value = validate_canonical_structured_value(encoded).unwrap();
            let original = value.record_field("requested-limit").unwrap().unwrap();
            assert_eq!(
                Quantity::decode(&original.value_node()[5..]).unwrap(),
                requested
            );
            let wrong = RationalQuantity::from_quantity(
                Quantity::parse_plot_literal("1Hz").unwrap(),
                &mut work,
            )
            .unwrap();
            assert_eq!(
                verifier.verify(
                    encoded,
                    QUANTITY_INFO_ID,
                    wrong,
                    None,
                    &mut ExactNumericWork::new(NEAR_TOTAL_NUMERICAL_WORK)
                ),
                Err(NearQuantityEvidenceRefusal::Physical)
            );
            if source.role() != QuantityRole::Linear {
                let point =
                    RationalQuantity::new(ExactRational::ONE, kelvin, QuantityRole::Point).unwrap();
                assert_eq!(
                    verifier.verify(
                        encoded,
                        QUANTITY_INFO_ID,
                        point,
                        None,
                        &mut ExactNumericWork::new(NEAR_TOTAL_NUMERICAL_WORK)
                    ),
                    Err(NearQuantityEvidenceRefusal::InvalidLimit)
                );
            }
        }
    }
}
