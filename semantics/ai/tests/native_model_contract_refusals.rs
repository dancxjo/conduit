use conduit_ai::{
    ModelCompatibilityRefusal, ModelEvidenceRefusal, ModelInvocationTerminal, ModelTextRefusal,
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
fn model_contract_refusals_round_trip_through_native_types() {
    for terminal in [
        ModelInvocationTerminal::Produced,
        ModelInvocationTerminal::Refused(ModelCompatibilityRefusal::SignatureMismatch),
        ModelInvocationTerminal::Failed,
        ModelInvocationTerminal::RuntimeLost,
    ] {
        assert_round_trip(terminal);
    }
    for refusal in [
        ModelEvidenceRefusal::MissingArtifactIdentity,
        ModelEvidenceRefusal::InvalidRuntimeIdentity,
        ModelEvidenceRefusal::WorkNotAdmitted,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        ModelCompatibilityRefusal::InvalidArtifact,
        ModelCompatibilityRefusal::SignatureMismatch,
        ModelCompatibilityRefusal::UnsupportedPrecision,
        ModelCompatibilityRefusal::RuntimeLoadedDifferentCheckpoint,
    ] {
        assert_round_trip(refusal);
    }
    for refusal in [
        ModelTextRefusal::EnvelopeBoundExceeded,
        ModelTextRefusal::NonCanonicalEnvelope,
        ModelTextRefusal::NotProduced,
        ModelTextRefusal::TextBoundExceeded,
    ] {
        assert_round_trip(refusal);
    }
}
