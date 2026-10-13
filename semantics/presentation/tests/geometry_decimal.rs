use conduit_core::{Quantity, Unit};
use conduit_presentation::{apply_transform2, point2_value, transform2_value, GeometryRefusal};

#[test]
fn adding_zero_preserves_high_exponent_length_coordinates() {
    let large = Quantity::from_decimal(1, 100, Unit::Meter).unwrap();
    let zero = Quantity::new(0, Unit::Meter);
    let point = point2_value("source", large.clone(), zero.clone()).unwrap();
    let transform = transform2_value("source", "target", zero, large.clone()).unwrap();
    let actual = apply_transform2(&point, &transform).unwrap();
    assert_eq!(
        actual,
        point2_value("target", large.clone(), large).unwrap()
    );
}

#[test]
fn geometry_cannot_add_temperature_points_as_offsets() {
    assert_eq!(
        transform2_value(
            "source",
            "target",
            Quantity::new(21, Unit::Celsius),
            Quantity::new(0, Unit::Meter)
        ),
        Err(GeometryRefusal::IncompatibleUnit)
    );
}

#[test]
fn coordinates_outside_the_selected_geometry_profile_refuse_without_panicking() {
    let large = conduit_core::MathValue::from_integer_decimal(&"9".repeat(1152)).unwrap();
    let quantity = Quantity::from_math(large, &Unit::Meter).unwrap();
    assert!(point2_value("source", quantity, Quantity::new(0, Unit::Meter)).is_err());
}

#[test]
fn affine_length_coordinates_preserve_points_and_require_delta_offsets() {
    use conduit_core::{MathValue, PrefixPolicy, QuantityFamilyDefinition, QuantityRole};
    let family =
        QuantityFamilyDefinition::point_delta("Position", "PositionDelta", Unit::Meter.dimension())
            .unwrap();
    let origin = Unit::new_math_role(
        family,
        "position",
        MathValue::ONE,
        MathValue::ZERO,
        PrefixPolicy::NONE,
        QuantityRole::Point,
    )
    .unwrap();
    let point = Quantity::new(2, origin.clone());
    assert_eq!(
        transform2_value("source", "target", point.clone(), point.clone()),
        Err(GeometryRefusal::IncompatibleUnit)
    );
    let delta = Quantity::from_math_role(MathValue::ONE, &origin, QuantityRole::Delta).unwrap();
    let transform = transform2_value("source", "target", delta.clone(), delta.clone()).unwrap();
    let actual = apply_transform2(
        &point2_value("source", point.clone(), point).unwrap(),
        &transform,
    )
    .unwrap();
    let expected_point = Quantity::new(3, origin.clone());
    assert_eq!(
        actual,
        point2_value("target", expected_point.clone(), expected_point).unwrap()
    );
    let actual_delta = apply_transform2(
        &point2_value("source", delta.clone(), delta).unwrap(),
        &transform,
    )
    .unwrap();
    let expected_delta =
        Quantity::from_math_role(MathValue::integer(2), &origin, QuantityRole::Delta).unwrap();
    assert_eq!(
        actual_delta,
        point2_value("target", expected_delta.clone(), expected_delta).unwrap()
    );
}
