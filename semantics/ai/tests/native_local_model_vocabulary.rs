use conduit_ai::{
    LocalModelCachePolicy, LocalModelFailure, LocalModelKindProfile, LocalModelLifecycleState,
    LocalModelOfferInvalidity, LocalModelRefusal, LocalModelTerminal,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn local_model_vocabularies_round_trip_through_native_types() {
    for value in [
        LocalModelKindProfile::Generate,
        LocalModelKindProfile::StreamGenerate,
        LocalModelKindProfile::PresentSemanticFront,
    ] {
        assert_round_trip(value);
    }
    assert_round_trip(LocalModelCachePolicy::OneLoadedModelUntilShutdown);
    for value in [
        LocalModelLifecycleState::Discovered,
        LocalModelLifecycleState::Ready,
        LocalModelLifecycleState::Lost,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LocalModelRefusal::NotInitialized,
        LocalModelRefusal::QueueFull,
        LocalModelRefusal::MemoryCeiling,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LocalModelFailure::Load,
        LocalModelFailure::Inference,
        LocalModelFailure::Shutdown,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LocalModelOfferInvalidity::MissingIdentity,
        LocalModelOfferInvalidity::InvalidQueue,
        LocalModelOfferInvalidity::DeterministicClaim,
    ] {
        assert_round_trip(value);
    }
    for value in [
        LocalModelTerminal::Produced,
        LocalModelTerminal::Refused(LocalModelRefusal::QueueFull),
        LocalModelTerminal::Failed(LocalModelFailure::Inference),
        LocalModelTerminal::ProviderLost,
    ] {
        assert_round_trip(value);
    }
}
