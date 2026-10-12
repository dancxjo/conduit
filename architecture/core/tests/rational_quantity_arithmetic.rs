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
    assert_eq!(
        point.checked_subtract(point, &mut work()).unwrap().role(),
        QuantityRole::Delta
    );
    let delta = quantity(5, "°C", QuantityRole::Delta);
    assert_eq!(
        point.checked_add(delta, &mut work()).unwrap().coordinate(),
        ExactRational::from_i128(25)
    );
    assert!(delta.checked_subtract(point, &mut work()).is_err());
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

#[test]
fn affine_offsets_cancel_for_point_difference_and_never_enter_delta() {
    let celsius = quantity(20, "°C", QuantityRole::Point);
    let kelvin = RationalQuantity::new(
        ExactRational::ratio(29315, 100, &mut work()).unwrap(),
        Unit::resolve("K").unwrap(),
        QuantityRole::Point,
    )
    .unwrap();
    let difference = celsius.checked_subtract(kelvin, &mut work()).unwrap();
    assert_eq!(difference.coordinate(), ExactRational::ZERO);
    assert_eq!(difference.role(), QuantityRole::Delta);
    let delta = quantity(5, "K", QuantityRole::Delta);
    let raised = celsius.checked_add(delta, &mut work()).unwrap();
    assert_eq!(raised.coordinate(), ExactRational::from_i128(25));
    assert_eq!(raised.role(), QuantityRole::Point);
    assert_eq!(delta.checked_add(celsius, &mut work()).unwrap(), raised);
    assert_eq!(
        raised.checked_subtract(delta, &mut work()).unwrap(),
        celsius
    );
}
