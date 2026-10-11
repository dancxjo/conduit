//! Reuse independent Python Fraction equations, including coordinates which the
//! old finite-decimal profile refused. No Rust output generates the oracle.
use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn quantity(source: &str, delta: bool, budget: &mut ExactNumericWork) -> RationalQuantity {
    let mut value = Quantity::parse_plot_literal(source).unwrap();
    if delta {
        value = Quantity::from_decimal_role(
            value.coefficient(),
            value.exponent(),
            value.unit(),
            QuantityRole::Delta,
        )
        .unwrap();
    }
    RationalQuantity::from_quantity(value, budget).unwrap()
}
#[test]
fn affine_coordinate_realization_matches_all_independent_fraction_vectors() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_temperature_reference.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 972);
    for case in cases {
        let mut budget = work();
        let source = quantity(
            case["source"].as_str().unwrap(),
            case["role"] != "point",
            &mut budget,
        );
        let target = Unit::resolve(case["target"].as_str().unwrap()).unwrap();
        let result = source.convert_to_unit(target, &mut budget).unwrap();
        let n =
            ExactRational::parse_decimal(case["numerator"].as_str().unwrap(), &mut budget).unwrap();
        let d = ExactRational::parse_decimal(case["denominator"].as_str().unwrap(), &mut budget)
            .unwrap();
        assert_eq!(
            result.coordinate(),
            n.checked_divide(d, &mut budget).unwrap(),
            "{case}"
        );
        assert_eq!(
            result.convert_to_unit(source.unit(), &mut budget).unwrap(),
            source,
            "{case}"
        );
    }
}
#[test]
fn physical_comparison_matches_independent_prefix_and_affine_vectors() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_comparison_reference.json")).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let mut budget = work();
        let delta = case["role"] != "quantity";
        let left = quantity(case["left"].as_str().unwrap(), delta, &mut budget);
        let right = quantity(case["right"].as_str().unwrap(), delta, &mut budget);
        let expected = match case["result"].as_str().unwrap() {
            "less" => Ordering::Less,
            "equal" => Ordering::Equal,
            "greater" => Ordering::Greater,
            other => panic!("unknown independent comparison {other}"),
        };
        assert_eq!(left.compare(right, &mut budget), Ok(expected), "{case}");
    }
}
