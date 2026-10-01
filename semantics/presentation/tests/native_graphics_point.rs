use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::GraphicsPoint;

#[test]
fn graphics_point_round_trips_exact_signed_coordinate_boundaries() {
    for point in [
        GraphicsPoint::new(i16::MIN, i16::MAX).unwrap(),
        GraphicsPoint::new(i16::MAX, i16::MIN).unwrap(),
        GraphicsPoint::new(0, 0).unwrap(),
    ] {
        let structured = point.into_structured().unwrap();
        assert_eq!(GraphicsPoint::from_structured(structured).unwrap(), point);
    }
}

#[test]
fn graphics_point_preserves_authored_coordinate_order() {
    let point = GraphicsPoint::new(-123, 456).unwrap();
    assert_eq!((*point.x(), *point.y()), (-123, 456));
}
