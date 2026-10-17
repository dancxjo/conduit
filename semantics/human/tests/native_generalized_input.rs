use conduit_core::{Quantity, Unit};
use conduit_human::{
    InputAxisSlot, InputAxisSlots, InputPressure, InputPressurePolicy, InputSurfacePoint,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn generalized_input_values_round_trip_through_authored_types() {
    let axis = InputAxisSlot::axis(
        "axis/x".into(),
        "input/normalized-bipolar@1".into(),
        Quantity::new(-250_000, Unit::Millionth),
    )
    .unwrap();
    let axes = InputAxisSlots::new([
        axis,
        InputAxisSlot::unused(),
        InputAxisSlot::unused(),
        InputAxisSlot::unused(),
    ])
    .unwrap();
    let structured = axes.clone().into_structured().unwrap();
    assert_eq!(InputAxisSlots::from_structured(structured).unwrap(), axes);

    let pressure = InputPressure::new(2, 1, InputPressurePolicy::CoalesceLatestState, 8).unwrap();
    let point = InputSurfacePoint::new(
        "input/surface-normalized".into(),
        Quantity::new(400_000, Unit::Millionth),
        Quantity::new(600_000, Unit::Millionth),
    )
    .unwrap();
    let structured = point.clone().into_structured().unwrap();
    assert_eq!(
        InputSurfacePoint::from_structured(structured).unwrap(),
        point
    );
    assert_eq!(*pressure.queue_capacity(), 8);
}
