use conduit_form::rust_binding::NativeRustBinding;
use conduit_text::{AddressConfigurationError, AddressValueError, MorseError, MorseKeyRefusal};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn address_refusals_have_native_identity_and_exact_round_trips() {
    for value in [
        AddressConfigurationError::Empty,
        AddressConfigurationError::TooManyNames,
        AddressConfigurationError::EmptyName,
        AddressConfigurationError::NameTooLarge,
        AddressConfigurationError::InvalidName,
        AddressConfigurationError::DuplicateName,
    ] {
        round_trip(value);
    }

    for value in [
        AddressValueError::BoundExceeded,
        AddressValueError::Malformed,
        AddressValueError::NonCanonical,
        AddressValueError::InvalidValue,
    ] {
        round_trip(value);
    }
}

#[test]
fn morse_refusals_have_native_identity_and_exact_round_trips() {
    for value in [
        MorseError::Empty,
        MorseError::TextTooLong,
        MorseError::UnsupportedCharacter,
        MorseError::InvalidWordGap,
        MorseError::InvalidUnitMillis,
        MorseError::SegmentCapacity,
        MorseError::OutputCapacity,
        MorseError::MalformedEncoding,
        MorseError::NonCanonicalEncoding,
        MorseError::InvalidPattern,
    ] {
        round_trip(value);
    }

    for value in [
        MorseKeyRefusal::InvalidUnitMillis,
        MorseKeyRefusal::InvalidTransitionCapacity,
        MorseKeyRefusal::InvalidClockBasis,
        MorseKeyRefusal::ClockBasisMismatch,
        MorseKeyRefusal::DuplicateSequence,
        MorseKeyRefusal::SequenceGap,
        MorseKeyRefusal::NonMonotonicTime,
        MorseKeyRefusal::WrongPhase,
        MorseKeyRefusal::AmbiguousDuration,
        MorseKeyRefusal::TransitionPressure,
        MorseKeyRefusal::Empty,
        MorseKeyRefusal::Incomplete,
        MorseKeyRefusal::Cancelled,
        MorseKeyRefusal::invalid_pattern(MorseError::InvalidPattern).unwrap(),
    ] {
        round_trip(value);
    }
}
