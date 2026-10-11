use conduit_core::{Quantity, Unit};
use conduit_presentation::{apply_transform2, point2_value, transform2_value, GeometryRefusal};

#[test]
fn adding_zero_preserves_high_exponent_length_coordinates() {
    let large = Quantity::from_decimal(1, 100, Unit::Meter).unwrap();
    let zero = Quantity::new(0, Unit::Meter);
    let point = point2_value("source", large, zero).unwrap();
    let transform = transform2_value("source", "target", zero, large).unwrap();
    let actual = apply_transform2(&point, &transform).unwrap();
    assert_eq!(actual, point2_value("target", large, large).unwrap());
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
