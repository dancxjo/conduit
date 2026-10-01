use conduit_ai::{
    GroundedAnswerRefusal, InterpretationDisposition, InterpretationProvenance, ModelFailure,
    ModelRefusal, ModelResultDisposition, ModelResultInvalidity, ModelResultProvenance,
};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn model_result_vocabularies_round_trip_through_their_native_types() {
    assert_round_trip(InterpretationProvenance::ModelDerived);
    for disposition in [
        InterpretationDisposition::Interpreted,
        InterpretationDisposition::InsufficientEvidence,
        InterpretationDisposition::ContradictoryEvidence,
    ] {
        assert_round_trip(disposition);
    }
    assert_round_trip(ModelResultProvenance::ModelDerived);
    for refusal in [
        ModelRefusal::UnsupportedRequest,
        ModelRefusal::PolicyDenied,
        ModelRefusal::ContextUnavailable,
        ModelRefusal::CapacityUnavailable,
    ] {
        assert_round_trip(refusal);
    }
    for failure in [
        ModelFailure::MalformedResult,
        ModelFailure::ImplementationFailure,
        ModelFailure::ResourceExhausted,
        ModelFailure::OutputBoundExceeded,
    ] {
        assert_round_trip(failure);
    }
    for disposition in [
        ModelResultDisposition::Produced,
        ModelResultDisposition::Refused(ModelRefusal::PolicyDenied),
        ModelResultDisposition::Failed(ModelFailure::ImplementationFailure),
        ModelResultDisposition::ProviderLost,
    ] {
        assert_round_trip(disposition);
    }
    for refusal in [
        GroundedAnswerRefusal::InvalidModelResult(ModelResultInvalidity::InvalidConfidence),
        GroundedAnswerRefusal::ModelDidNotProduce(ModelResultDisposition::Cancelled),
        GroundedAnswerRefusal::CitationNotInContext,
    ] {
        assert_round_trip(refusal);
    }
}
