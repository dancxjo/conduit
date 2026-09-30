use conduit_ai::{ModelOperation, ModelOperationCode, ModelPortPresence, ModelPortPresenceCode};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn model_signature_codes_preserve_the_established_digest_tags() {
    let operations = [
        ModelOperation::Infer,
        ModelOperation::Encode,
        ModelOperation::Decode,
        ModelOperation::Sample,
        ModelOperation::LogProbability,
        ModelOperation::Evaluate,
        ModelOperation::Train,
    ];
    for (tag, operation) in (0_u8..).zip(operations) {
        assert_eq!(ModelOperationCode::encode(operation), [tag]);
        assert_eq!(ModelOperationCode::decode(&[tag]), Ok(operation));
    }
    assert!(ModelOperationCode::decode(&[7]).is_err());

    assert_eq!(
        ModelPortPresenceCode::encode(ModelPortPresence::Required),
        [0]
    );
    assert_eq!(
        ModelPortPresenceCode::encode(ModelPortPresence::Optional),
        [1]
    );
    assert!(ModelPortPresenceCode::decode(&[2]).is_err());
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
