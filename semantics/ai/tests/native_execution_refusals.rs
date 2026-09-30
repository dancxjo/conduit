use conduit_ai::{DynamicsRefusal, GeneratedTextFlowRefusal};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn execution_refusals_round_trip_through_native_types() {
    for value in [
        GeneratedTextFlowRefusal::WrongSequence,
        GeneratedTextFlowRefusal::OutputOverflow,
        GeneratedTextFlowRefusal::AlreadyTerminal,
    ] {
        assert_round_trip(value);
    }
    for value in [
        DynamicsRefusal::MissingIdentity,
        DynamicsRefusal::UnsupportedStochasticProfile,
        DynamicsRefusal::WorkBoundExceeded,
    ] {
        assert_round_trip(value);
    }
}
