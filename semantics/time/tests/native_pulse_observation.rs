use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::PulseObservation;

#[test]
fn pulse_observation_round_trips_through_its_exact_native_record() {
    let observation = PulseObservation::new(240, 42).unwrap();
    let structured = observation.into_structured().unwrap();
    assert_eq!(
        structured.value_type(),
        &PulseObservation::semantic_type().unwrap()
    );
    assert_eq!(
        PulseObservation::from_structured(structured).unwrap(),
        observation
    );
}
