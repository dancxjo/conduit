use conduit_core::{KindId, QuantityUnit};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_human::{
    BoundKind, InteractionApplicationOutcome, InteractionFamily, InteractionProposalPayload,
    InteractionRefusal, InteractionTypeDigest, InteractionValue, InteractionValueKind,
    OptionAvailability, RealizationRangePolicy, ScalarQuantization,
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

#[test]
fn interaction_family_is_one_payload_rich_native_type() {
    let kind = KindId::from("interaction/value@1");
    for family in [
        InteractionFamily::Activate,
        InteractionFamily::Boolean,
        InteractionFamily::choice_one(kind.clone(), 4),
        InteractionFamily::choice_many(kind.clone(), 8, 1, 3),
        InteractionFamily::scalar_range(
            QuantityUnit::Percent,
            0,
            BoundKind::Inclusive,
            100,
            BoundKind::Exclusive,
            5,
        ),
        InteractionFamily::relative_range(QuantityUnit::One, -12, 12, 1),
        InteractionFamily::text_value(4_096, false),
        InteractionFamily::structured_value(kind, [7; 32], 8_192),
    ] {
        assert_round_trip(family);
    }

    assert_round_trip(InteractionValueKind::new("k".repeat(128)).unwrap());
    assert!(InteractionValueKind::new(String::new()).is_err());
    assert!(InteractionValueKind::new("k".repeat(129)).is_err());
    assert_round_trip(InteractionTypeDigest::new([255; 32]).unwrap());
    assert!(!include_str!("../src/human_interaction.rs").contains("pub enum InteractionFamily"));
}

#[test]
fn interaction_values_and_proposal_payloads_are_native_and_exactly_bounded() {
    let kind = KindId::from("interaction/value@1");
    let empty = InteractionValue::new(kind.clone(), Vec::new()).unwrap();
    let maximum = InteractionValue::new(kind.clone(), vec![7; 65_536]).unwrap();
    assert_round_trip(empty.clone());
    assert_round_trip(maximum.clone());
    assert_eq!(
        InteractionValue::new(kind, vec![0; 65_537]),
        Err(InteractionRefusal::ValueBoundExceeded)
    );

    assert_round_trip(InteractionProposalPayload::Activate);
    assert_round_trip(InteractionProposalPayload::selected(vec![empty.clone(), maximum]).unwrap());
    assert_round_trip(InteractionProposalPayload::relative_value(empty).unwrap());

    let source = include_str!("../src/human_interaction.rs");
    assert!(!source.contains("pub struct InteractionValue"));
    assert!(!source.contains("pub enum InteractionProposalPayload"));
}
