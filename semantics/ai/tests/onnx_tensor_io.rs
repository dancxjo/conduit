use conduit_ai::{imported_onnx::*, onnx_tensor_io::ImportedOnnxTensorRefusal as R, *};
use conduit_data::{NamedTensorBundle, TensorAxisRole, TensorElement};
use sha2::{Digest, Sha256};
#[path = "common/onnx_fixture.rs"]
mod proto;
#[path = "common/onnx_resource_fixture.rs"]
mod resource_fixture;
#[path = "common/model_signature_fixture.rs"]
mod signature_fixture;
#[path = "common/named_tensor_fixture.rs"]
mod value_fixture;

fn imported() -> ImportedOnnxResource {
    let original = signature_fixture::signature();
    let ports = |ports: &[ModelPortConstraint]| {
        ports
            .iter()
            .map(|port| {
                let mut port = port.clone();
                let ModelValueConstraint::Tensor(v) = &port.value else {
                    panic!("tensor fixture")
                };
                port.value = ModelValueConstraint::tensor(
                    ModelTensorConstraint::from_parts(
                        v.constraint().elements.get().as_slice().to_vec(),
                        vec![ModelAxisConstraint::new(
                            ModelDimensionConstraint::bounded(32, 1).unwrap(),
                            TensorAxisRole::Feature,
                        )
                        .unwrap()],
                        4096,
                    )
                    .unwrap(),
                )
                .unwrap();
                port
            })
            .collect()
    };
    let signature = ModelSignature::from_parts(
        "test/symbolic-model@1".into(),
        1,
        vec![ModelOperation::Infer],
        ports(original.inputs.get().as_slice()),
        ports(original.outputs.get().as_slice()),
    )
    .unwrap();
    let graph = [
        proto::bytes(2, b"symbolic-mixed"),
        proto::bytes(11, &proto::port("tokens", 7, proto::bytes(2, b"sequence"))),
        proto::bytes(11, &proto::port("scale", 1, proto::bytes(2, b"sequence"))),
        proto::bytes(12, &proto::port("result", 1, proto::bytes(2, b"sequence"))),
    ]
    .concat();
    let model = [
        proto::number(1, 9),
        proto::bytes(8, &proto::number(2, 18)),
        proto::bytes(7, &graph),
    ]
    .concat();
    let binding = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&model).into(),
        graph_name: "symbolic-mixed".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("identifiers", "tokens"), binding("gain", "scale")],
        outputs: vec![binding("prediction", "result")],
    };
    ImportedOnnxResource::bind(resource_fixture::resource(signature, model), &sidecar).unwrap()
}

fn inputs(ids: usize, gain: usize) -> NamedTensorBundle {
    NamedTensorBundle::from_values(vec![
        value_fixture::tensor("identifiers", TensorElement::I64, &vec![0; ids * 8]),
        value_fixture::tensor("gain", TensorElement::F32, &vec![0; gain * 4]),
    ])
    .unwrap()
}
fn outputs(count: usize) -> NamedTensorBundle {
    NamedTensorBundle::from_values(vec![value_fixture::tensor(
        "prediction",
        TensorElement::F32,
        &vec![0; count * 4],
    )])
    .unwrap()
}

#[test]
fn one_symbol_must_have_one_actual_dimension_across_inputs_and_outputs() {
    let model = imported();
    assert_eq!(model.sidecar().inputs[0].graph_name, "tokens");
    model.validate_named_inputs(&inputs(2, 2)).unwrap();
    model
        .validate_named_outputs(&inputs(2, 2), &outputs(2))
        .unwrap();
    assert_eq!(
        model.validate_named_inputs(&inputs(2, 3)),
        Err(R::SymbolMismatch)
    );
    assert_eq!(
        model.validate_named_outputs(&inputs(2, 2), &outputs(3)),
        Err(R::SymbolMismatch)
    );
    assert_eq!(
        model.validate_named_inputs(&inputs(33, 33)),
        Err(R::Semantic(ModelTensorIoRefusal::Dimension))
    );
}
