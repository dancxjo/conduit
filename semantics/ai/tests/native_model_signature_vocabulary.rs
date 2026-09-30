use conduit_ai::{ModelOperation, ModelPortPresence};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn model_signature_vocabularies_round_trip_through_their_native_types() {
    for operation in [
        ModelOperation::Infer,
        ModelOperation::Encode,
        ModelOperation::Decode,
        ModelOperation::Sample,
        ModelOperation::LogProbability,
        ModelOperation::Evaluate,
        ModelOperation::Train,
    ] {
        assert_round_trip(operation);
    }
    for presence in [ModelPortPresence::Required, ModelPortPresence::Optional] {
        assert_round_trip(presence);
    }
}
