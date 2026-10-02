use conduit_ai::{
    ModelAxisConstraint, ModelDimensionConstraint, ModelOperation, ModelOperationCode,
    ModelPortConstraint, ModelPortPresence, ModelPortPresenceCode, ModelSignature,
    ModelTensorConstraint, ModelValueConstraint,
};
use conduit_data::{TensorAxisRole, TensorElement};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

fn tensor(axes: Vec<ModelAxisConstraint>) -> ModelTensorConstraint {
    ModelTensorConstraint::from_parts(vec![TensorElement::F32], axes, 4_096).unwrap()
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

    let constraint = tensor(vec![
        ModelAxisConstraint::new(
            ModelDimensionConstraint::bounded(64, 1).unwrap(),
            TensorAxisRole::Time,
        )
        .unwrap(),
        ModelAxisConstraint::new(
            ModelDimensionConstraint::fixed(16).unwrap(),
            TensorAxisRole::Feature,
        )
        .unwrap(),
    ]);
    assert_round_trip(constraint.clone());
    assert_round_trip(ModelValueConstraint::sampled_signal(constraint.clone()).unwrap());

    let signature = ModelSignature::from_parts(
        "model/native-signature@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![ModelPortConstraint::from_parts(
            "samples".into(),
            "data/sampled-signal@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::sampled_signal(constraint.clone()).unwrap(),
        )
        .unwrap()],
        vec![ModelPortConstraint::from_parts(
            "embedding".into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(constraint).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    assert_round_trip(signature.clone());
    signature.validate().unwrap();
    assert_ne!(signature.semantic_digest().unwrap(), [0; 32]);
}

#[test]
fn model_signature_native_bounds_refuse_invalid_construction() {
    assert!(ModelTensorConstraint::from_parts(Vec::new(), Vec::new(), 1).is_err());
    let axes = (0..9)
        .map(|_| {
            ModelAxisConstraint::new(
                ModelDimensionConstraint::fixed(1).unwrap(),
                TensorAxisRole::Feature,
            )
            .unwrap()
        })
        .collect();
    assert!(ModelTensorConstraint::from_parts(vec![TensorElement::F32], axes, 1).is_err());
    assert!(ModelPortConstraint::from_parts(
        "x".repeat(129),
        "data/tensor@1".into(),
        ModelPortPresence::Required,
        ModelValueConstraint::tensor(tensor(vec![ModelAxisConstraint::new(
            ModelDimensionConstraint::fixed(1).unwrap(),
            TensorAxisRole::Feature,
        )
        .unwrap()]))
        .unwrap(),
    )
    .is_err());
    assert!(ModelSignature::from_parts(
        "model/empty-operations@1".into(),
        1,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .is_err());
}
