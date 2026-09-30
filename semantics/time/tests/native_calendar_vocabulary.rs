use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{AvailabilityState, InvitationState, ParticipantRole};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(structured.value_type(), &T::semantic_type().unwrap());
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn calendar_vocabularies_round_trip_through_their_native_types() {
    for role in [
        ParticipantRole::Organizer,
        ParticipantRole::Required,
        ParticipantRole::Optional,
    ] {
        assert_round_trip(role);
    }
    for invitation in [
        InvitationState::NeedsAction,
        InvitationState::Accepted,
        InvitationState::Declined,
        InvitationState::Tentative,
    ] {
        assert_round_trip(invitation);
    }
    for availability in [
        AvailabilityState::Free,
        AvailabilityState::Tentative,
        AvailabilityState::Busy,
        AvailabilityState::Unavailable,
    ] {
        assert_round_trip(availability);
    }
}
