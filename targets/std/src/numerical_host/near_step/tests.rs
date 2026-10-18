//! The complete runtime calculation, canonical writer and consumer verification.
use super::*;
#[test]
fn runtime_projection_and_certificate_checks_reuse_all_prepared_storage() {
    let mut work = ExactNumericWork::new(NEAR_NUMERICAL_WORK);
    let unit = Unit::resolve("m").unwrap();
    let source = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        unit,
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(source, &mut input).unwrap();
    input.truncate(n);
    let requested = Quantity::parse_plot_literal("0.0001m").unwrap();
    let tolerance = RationalQuantity::from_quantity(requested, &mut work).unwrap();
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
        for _ in 0..32 {
            back.evaluate(&input).unwrap();
            verifier
                .verify(
                    back.state.encoding.encoded(),
                    QUANTITY_INFO_ID,
                    tolerance,
                    None,
                    &mut ExactNumericWork::new(NEAR_NUMERICAL_WORK),
                )
                .unwrap();
        }
        assert_eq!(guard.finish(), 0);
        assert_eq!(back.retained_capacity_bytes(), capacity);
    }
}

#[path = "certificate_tests.rs"]
mod certificate_tests;
