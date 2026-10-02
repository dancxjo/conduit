use conduit_alife::GardenEvolutionRefusal;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn garden_refusals_round_trip_through_their_exact_native_type() {
    for refusal in [
        GardenEvolutionRefusal::MalformedState,
        GardenEvolutionRefusal::MalformedClockObservation,
        GardenEvolutionRefusal::MalformedContactObservation,
        GardenEvolutionRefusal::MalformedEnrichedObservation,
        GardenEvolutionRefusal::StepCapacityExceeded,
        GardenEvolutionRefusal::ArithmeticOverflow,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            GardenEvolutionRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
