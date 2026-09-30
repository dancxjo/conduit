use conduit_form::rust_binding::NativeRustBinding;
use conduit_system_continuity::{
    LineLossDisposition, RebootDenial, RebootPendingState, RebootProgressError,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn reboot_denial_is_native_semantic_info_with_stable_rust_serde() {
    for denial in [
        RebootDenial::MalformedRequest,
        RebootDenial::Unsupported,
        RebootDenial::Unauthorized,
        RebootDenial::StaleTargetBoot,
        RebootDenial::SessionMismatch,
        RebootDenial::Replay,
        RebootDenial::AttemptLimitReached,
        RebootDenial::TransactionPending,
    ] {
        let structured = denial.into_structured().unwrap();
        assert_eq!(RebootDenial::from_structured(structured).unwrap(), denial);
        let encoded = serde_json::to_vec(&denial).unwrap();
        assert_eq!(
            serde_json::from_slice::<RebootDenial>(&encoded).unwrap(),
            denial
        );
    }
}

#[test]
fn reboot_progress_and_line_loss_vocabulary_is_native_semantic_info() {
    for state in [
        RebootPendingState::Idle,
        RebootPendingState::Accepted,
        RebootPendingState::AwaitingReplacement,
        RebootPendingState::Completed,
        RebootPendingState::UnknownProofWindowExpired,
    ] {
        round_trip(state);
    }

    for error in [
        RebootProgressError::RequestMismatch,
        RebootProgressError::NotAccepted,
        RebootProgressError::OldBootNotTerminated,
        RebootProgressError::ReplacementHostMismatch,
        RebootProgressError::ReplacementBootReused,
        RebootProgressError::ReplacementUnavailable,
        RebootProgressError::ProofWindowExpired,
    ] {
        round_trip(error);
    }

    round_trip(LineLossDisposition::IntentionalTransitionPending);
    round_trip(LineLossDisposition::OrdinaryTransportFailure);
}
