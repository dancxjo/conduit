use conduit_ai::{BatchOrder, ObjectiveParticipation};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn training_vocabularies_round_trip_through_native_types() {
    for participation in [
        ObjectiveParticipation::Optimize,
        ObjectiveParticipation::ObserveOnly,
    ] {
        assert_round_trip(participation);
    }
    for order in [BatchOrder::Stable, BatchOrder::Shuffled] {
        assert_round_trip(order);
    }
}
