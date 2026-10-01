use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{
    BoundKind, InteractionApplicationOutcome, InteractionRefusal, OptionAvailability,
    RealizationRangePolicy, ScalarQuantization,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn interaction_classifications_round_trip_through_exact_native_types() {
    for value in [BoundKind::Inclusive, BoundKind::Exclusive] {
        assert_round_trip(value);
    }
    for value in [
        RealizationRangePolicy::Refuse,
        RealizationRangePolicy::Clamp,
    ] {
        assert_round_trip(value);
    }
    for value in [ScalarQuantization::Exact, ScalarQuantization::Nearest] {
        assert_round_trip(value);
    }
    for value in [
        InteractionRefusal::InvalidIdentity,
        InteractionRefusal::InvalidContract,
        InteractionRefusal::InvalidDomain,
        InteractionRefusal::InvalidCurrentState,
        InteractionRefusal::ValueBoundExceeded,
        InteractionRefusal::WrongValueKind,
        InteractionRefusal::MalformedValue,
        InteractionRefusal::StaleState,
        InteractionRefusal::RemovedOption,
        InteractionRefusal::UnavailableOption,
        InteractionRefusal::InvalidCardinality,
        InteractionRefusal::InvalidCombination,
        InteractionRefusal::ConcurrentStateChange,
        InteractionRefusal::OutOfRange,
        InteractionRefusal::UnsupportedGranularity,
        InteractionRefusal::DuplicateProposal,
        InteractionRefusal::QueuePressure,
        InteractionRefusal::ResultPressure,
        InteractionRefusal::UnknownProposal,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn interaction_payloads_round_trip_at_the_exact_text_bound() {
    assert_round_trip(OptionAvailability::Available);
    assert_round_trip(OptionAvailability::unavailable("u".repeat(128)).unwrap());
    assert_round_trip(InteractionApplicationOutcome::accepted("s".repeat(128)).unwrap());
    assert_round_trip(InteractionApplicationOutcome::refused("r".repeat(128)).unwrap());
    assert_round_trip(InteractionApplicationOutcome::failed("f".repeat(128)).unwrap());
    assert_round_trip(InteractionApplicationOutcome::Cancelled);
}

#[test]
fn interaction_payloads_refuse_empty_and_oversize_identity_text() {
    for text in [String::new(), "x".repeat(129)] {
        assert!(OptionAvailability::unavailable(text.clone()).is_err());
        assert!(InteractionApplicationOutcome::accepted(text.clone()).is_err());
        assert!(InteractionApplicationOutcome::refused(text.clone()).is_err());
        assert!(InteractionApplicationOutcome::failed(text.clone()).is_err());
    }
}
