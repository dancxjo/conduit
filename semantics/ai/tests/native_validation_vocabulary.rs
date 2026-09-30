use conduit_ai::{
    HybridRetrievalOfferInvalidity, ModelResultInvalidity, R3OfferInvalidity,
    RagAnswerOfferInvalidity, SourceExtractionOfferInvalidity, StructuredResultInvalidity,
    TemporalContextRefusal, VectorSearchExecutionProofClass, VectorSearchOfferInvalidity,
    VectorSearchProofClass,
};
use conduit_form::rust_binding::NativeRustBinding;

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn ai_validation_vocabulary_has_native_identity_and_exact_round_trips() {
    for value in [
        ModelResultInvalidity::MissingExactIdentity,
        ModelResultInvalidity::UnsupportedPayloadKind,
        ModelResultInvalidity::InvalidConfidence,
        ModelResultInvalidity::InputBoundExceeded,
        ModelResultInvalidity::ContextBoundExceeded,
        ModelResultInvalidity::OutputBoundExceeded,
        ModelResultInvalidity::WorkBoundExceeded,
        ModelResultInvalidity::HistoryBoundExceeded,
        ModelResultInvalidity::PayloadLengthMismatch,
        ModelResultInvalidity::TerminalPayloadPresent,
        ModelResultInvalidity::ProducedPayloadMissing,
    ] {
        round_trip(value);
    }

    for value in [
        StructuredResultInvalidity::Empty,
        StructuredResultInvalidity::TooManyMembers,
        StructuredResultInvalidity::MemberTooLarge,
        StructuredResultInvalidity::DuplicateMember,
        StructuredResultInvalidity::LabelNotAllowed,
        StructuredResultInvalidity::DimensionMismatch,
        StructuredResultInvalidity::NonFiniteValue,
    ] {
        round_trip(value);
    }

    for value in [
        TemporalContextRefusal::EmptyClockIdentity,
        TemporalContextRefusal::ClockIdentityTooLarge,
        TemporalContextRefusal::ReversedValidityInterval,
        TemporalContextRefusal::RetrievalAfterReference,
        TemporalContextRefusal::ClockBasisMismatch,
        TemporalContextRefusal::SourceUnavailable,
        TemporalContextRefusal::SourceAfterReference,
        TemporalContextRefusal::ReversedQueryWindow,
        TemporalContextRefusal::ArithmeticOverflow,
        TemporalContextRefusal::UncertainAge,
    ] {
        round_trip(value);
    }

    round_trip(RagAnswerOfferInvalidity::EmptyProcessIdentity);
    round_trip(RagAnswerOfferInvalidity::ProcessIdentityTooLarge);
    round_trip(R3OfferInvalidity::EmptyProcessIdentity);
    round_trip(R3OfferInvalidity::ProcessIdentityTooLarge);
    round_trip(VectorSearchOfferInvalidity::EmptyProcessIdentity);
    round_trip(VectorSearchOfferInvalidity::ProcessIdentityTooLarge);
    round_trip(SourceExtractionOfferInvalidity::EmptyProcessIdentity);
    round_trip(SourceExtractionOfferInvalidity::ProcessIdentityTooLarge);
    round_trip(HybridRetrievalOfferInvalidity::EmptyProcessIdentity);
    round_trip(HybridRetrievalOfferInvalidity::ProcessIdentityTooLarge);
    round_trip(VectorSearchExecutionProofClass::DeterministicExact);
    round_trip(VectorSearchExecutionProofClass::Approximate);
    round_trip(VectorSearchProofClass::DeterministicExactOracle);
}
