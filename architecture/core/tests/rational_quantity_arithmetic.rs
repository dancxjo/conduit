use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn quantity(value: i128, suffix: &str, role: QuantityRole) -> RationalQuantity {
    RationalQuantity::new(
        ExactRational::from_i128(value),
        Unit::resolve(suffix).unwrap(),
        role,
    )
    .unwrap()
}
#[test]
fn exact_addition_converts_units_and_retains_left_coordinates() {
    let left = quantity(1, "s", QuantityRole::Linear);
    let right = quantity(250, "ms", QuantityRole::Linear);
    let sum = left.checked_add(right, &mut work()).unwrap();
    assert_eq!(sum.unit(), left.unit());
    assert_eq!(
        sum.coordinate(),
        ExactRational::ratio(5, 4, &mut work()).unwrap()
    );
    assert_eq!(sum.checked_subtract(right, &mut work()).unwrap(), left);
}
#[test]
fn rational_thirds_remain_exact() {
    let third = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work()).unwrap(),
        Unit::resolve("s").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let sum = third
        .checked_add(third, &mut work())
        .unwrap()
        .checked_add(third, &mut work())
        .unwrap();
    assert_eq!(sum.coordinate(), ExactRational::ONE);
}
#[test]
fn incompatible_dimensions_and_affine_points_refuse() {
    let time = quantity(1, "s", QuantityRole::Linear);
    assert!(time
        .checked_add(quantity(1, "m", QuantityRole::Linear), &mut work())
        .is_err());
    let point = quantity(20, "°C", QuantityRole::Point);
    assert!(point.checked_add(point, &mut work()).is_err());
    assert!(point.checked_subtract(point, &mut work()).is_err());
    let delta = quantity(5, "°C", QuantityRole::Delta);
    assert!(point.checked_add(delta, &mut work()).is_err());
    assert_eq!(
        delta.checked_add(delta, &mut work()).unwrap().coordinate(),
        ExactRational::from_i128(10)
    );
}
#[test]
fn numeric_work_limit_is_not_silently_rounded() {
    let value = quantity(1, "s", QuantityRole::Linear);
    assert!(value
        .checked_add(value, &mut ExactNumericWork::new(0))
        .is_err());
}
