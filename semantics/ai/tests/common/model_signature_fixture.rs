use conduit_ai::*;
use conduit_data::{TensorAxisRole, TensorElement};

pub fn signature() -> ModelSignature {
    let port = |name: &str, element, dimension| {
        ModelPortConstraint::from_parts(
            name.into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(dimension, TensorAxisRole::Feature).unwrap()],
                    4096,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    ModelSignature::from_parts(
        "test/non-speech-model@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![
            port(
                "identifiers",
                TensorElement::I64,
                ModelDimensionConstraint::bounded(32, 1).unwrap(),
            ),
            port(
                "gain",
                TensorElement::F32,
                ModelDimensionConstraint::fixed(1).unwrap(),
            ),
        ],
        vec![port(
            "prediction",
            TensorElement::F32,
            ModelDimensionConstraint::fixed(4).unwrap(),
        )],
    )
    .unwrap()
}
