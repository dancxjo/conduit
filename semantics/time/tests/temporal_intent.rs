use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{
    CalendarDateIntent, DeadlineIntent, LocalDate, ObservanceWindowIntent, SolarLandmark,
    TemporalInstant, TemporalLandmark,
};

fn distinct<A: NativeRustBinding, B: NativeRustBinding>() {
    assert_ne!(
        A::semantic_type().unwrap(),
        B::semantic_type().unwrap(),
        "different temporal intentions must not collapse into one Type"
    );
}

#[test]
fn unresolved_intents_have_distinct_checked_types() {
    // A non-Gregorian date label is not a validated LocalDate or an instant.
    distinct::<CalendarDateIntent, LocalDate>();
    distinct::<CalendarDateIntent, TemporalInstant>();
    distinct::<TemporalLandmark, TemporalInstant>();

    // A normative deadline is not an observance window or a clock event.
    distinct::<DeadlineIntent, ObservanceWindowIntent>();
    distinct::<DeadlineIntent, TemporalInstant>();

    // This test validates Conduitese-owned bindings. No time/place resolution
    // or runtime support is implied by the presence of a portable Type.
}

#[test]
fn solar_landmark_vocabulary_round_trips_without_a_host() {
    for landmark in [
        SolarLandmark::Sunrise,
        SolarLandmark::Sunset,
        SolarLandmark::Nightfall,
        SolarLandmark::SolarNoon,
    ] {
        let structured = landmark.into_structured().unwrap();
        assert_eq!(structured.value_type(), &SolarLandmark::semantic_type().unwrap());
        assert_eq!(SolarLandmark::from_structured(structured).unwrap(), landmark);
    }
}
