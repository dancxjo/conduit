use conduit_ai::ModelTensorIoRefusal as R;
use conduit_data::{NamedTensor, NamedTensorBundle, TensorAxis, TensorAxisRole, TensorElement};
use conduit_plot::rust_binding::BoundedSequence;
#[path = "common/model_signature_fixture.rs"]
mod fixture;

#[path = "common/named_tensor_fixture.rs"]
mod value_fixture;
use value_fixture::tensor;
fn axis() -> TensorAxis {
    TensorAxis {
        role: TensorAxisRole::Feature,
        identity: None,
        unit: None,
    }
}
fn inputs() -> Vec<NamedTensor> {
    vec![
        tensor(
            "identifiers",
            TensorElement::I64,
            &[7_i64.to_le_bytes(), 11_i64.to_le_bytes()].concat(),
        ),
        tensor("gain", TensorElement::F32, &0.5_f32.to_le_bytes()),
    ]
}

#[test]
fn semantic_names_accept_mixed_dtypes_and_arbitrary_bundle_order() {
    let signature = fixture::signature();
    let mut values = inputs();
    signature
        .validate_named_inputs(&NamedTensorBundle::from_values(values.clone()).unwrap())
        .unwrap();
    values.reverse();
    signature
        .validate_named_inputs(&NamedTensorBundle::from_values(values).unwrap())
        .unwrap();
    let output =
        NamedTensorBundle::from_values(vec![tensor("prediction", TensorElement::F32, &[0; 16])])
            .unwrap();
    signature.validate_named_outputs(&output).unwrap();
    assert_eq!(
        signature.validate_named_outputs(&NamedTensorBundle::from_values(vec![]).unwrap()),
        Err(R::MissingPort)
    );
}

#[test]
fn wrong_names_types_rank_dimensions_and_axis_meaning_refuse() {
    let signature = fixture::signature();
    let cases = [
        (
            {
                let mut v = inputs();
                v.pop();
                v
            },
            R::MissingPort,
        ),
        (
            {
                let mut v = inputs();
                v[0].name = "tokens".into();
                v
            },
            R::UnknownPort,
        ),
        (
            {
                let mut v = inputs();
                v[0] = tensor("identifiers", TensorElement::F32, &[0; 8]);
                v
            },
            R::Dtype,
        ),
        (
            {
                let mut v = inputs();
                v[0] = tensor("identifiers", TensorElement::I64, &[0; 264]);
                v
            },
            R::Dimension,
        ),
        (
            {
                let mut v = inputs();
                v[0].value.dimensions = BoundedSequence::try_from_iter([1, 2]).unwrap();
                v[0].value.axes = BoundedSequence::try_from_iter([axis(), axis()]).unwrap();
                v
            },
            R::Rank,
        ),
        (
            {
                let mut v = inputs();
                v[0].value.axes = BoundedSequence::try_from_iter([TensorAxis {
                    role: TensorAxisRole::Time,
                    ..axis()
                }])
                .unwrap();
                v
            },
            R::AxisRole,
        ),
    ];
    for (values, expected) in cases {
        let bundle = NamedTensorBundle::from_values(values).unwrap();
        assert_eq!(signature.validate_named_inputs(&bundle), Err(expected));
    }
}
