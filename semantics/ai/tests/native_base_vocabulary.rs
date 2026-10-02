use conduit_ai::{BaseProofClass, DataHandling, Metering};
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn base_facts_round_trip_through_their_exact_native_types() {
    assert_round_trip(BaseProofClass::DeterministicConformanceFixture);
    assert_round_trip(DataHandling::LocalOnly);
    assert_round_trip(DataHandling::RemoteEgress);
    assert_round_trip(Metering::UnmeteredFixture);
    assert_round_trip(Metering::MeteredFixture);
}
