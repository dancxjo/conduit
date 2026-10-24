//! Dynamic optional tensor inputs; initializer defaults and optional outputs stay distinct.
use conduit_ai::{
    imported_onnx::*, onnx_metadata::*, onnx_tensor_io::ImportedOnnxTensorRefusal,
    ModelAxisConstraint, ModelDimensionConstraint, ModelPortPresence, ModelSignature,
    ModelTensorAxes, ModelTensorElements, ModelTensorIoRefusal, ModelValueConstraint,
};
use conduit_data::{NamedTensorBundle, TensorAxisRole, TensorElement};
use conduit_plot::rust_binding::BoundedSequence;
use sha2::{Digest, Sha256};
#[path = "common/onnx_fixture.rs"]
#[allow(dead_code)]
mod proto;
#[path = "common/onnx_resource_fixture.rs"]
mod resource;
#[path = "common/model_signature_fixture.rs"]
mod signature_fixture;
#[path = "common/named_tensor_fixture.rs"]
mod values;

fn tensor_type(shape: Option<Vec<u8>>) -> Vec<u8> {
    let mut tensor = proto::number(1, 1); // FLOAT, not a new optional value carrier.
    if let Some(shape) = shape {
        tensor.extend(proto::bytes(2, &shape));
    }
    proto::bytes(1, &tensor)
}
fn dynamic_type() -> Vec<u8> {
    tensor_type(Some(proto::bytes(1, &proto::bytes(2, b"sequence"))))
}
fn optional(inner: &[u8]) -> Vec<u8> {
    proto::bytes(9, &proto::bytes(1, inner))
}
fn port(name: &str, ty: &[u8]) -> Vec<u8> {
    [proto::bytes(1, name.as_bytes()), proto::bytes(2, ty)].concat()
}
fn graph(ir: u64, input: &[u8], output: &[u8], extra: &[u8]) -> Vec<u8> {
    // Identity consumes x only; optional graph input opt is genuinely declared,
    // and does not turn into an extra output when it is present.
    let node = [
        proto::bytes(1, b"x"),
        proto::bytes(2, b"y"),
        proto::bytes(4, b"Identity"),
    ]
    .concat();
    let graph = [
        proto::bytes(1, &node),
        proto::bytes(2, b"optional-tensor-identity"),
        proto::bytes(11, &port("x", &dynamic_type())),
        proto::bytes(11, &port("opt", input)),
        proto::bytes(12, &port("y", output)),
        extra.to_vec(),
    ]
    .concat();
    [
        proto::number(1, ir),
        proto::bytes(8, &proto::number(2, 18)),
        proto::bytes(7, &graph),
    ]
    .concat()
}
fn inspect(bytes: &[u8]) -> Result<OnnxMetadata, OnnxMetadataRefusal> {
    inspect_onnx(bytes, Sha256::digest(bytes).into())
}
fn signature(presence: ModelPortPresence) -> ModelSignature {
    let original = signature_fixture::signature();
    let ports = |ports: &[conduit_ai::ModelPortConstraint]| {
        ports
            .iter()
            .map(|port| {
                let mut port = port.clone();
                if port.identity.get() == "gain" {
                    port.presence = presence;
                }
                let ModelValueConstraint::Tensor(value) = &port.value else {
                    unreachable!()
                };
                let mut tensor = value.constraint().clone();
                tensor.elements = ModelTensorElements::new(
                    BoundedSequence::try_from_iter([TensorElement::F32]).unwrap(),
                )
                .unwrap();
                tensor.axes = ModelTensorAxes::new(
                    BoundedSequence::try_from_iter([ModelAxisConstraint::new(
                        ModelDimensionConstraint::bounded(32, 1).unwrap(),
                        TensorAxisRole::Feature,
                    )
                    .unwrap()])
                    .unwrap(),
                )
                .unwrap();
                port.value = ModelValueConstraint::tensor(tensor).unwrap();
                port
            })
            .collect()
    };
    ModelSignature::from_parts(
        "test/optional-tensor-identity@1".into(),
        1,
        vec![conduit_ai::ModelOperation::Infer],
        ports(original.inputs.get().as_slice()),
        ports(original.outputs.get().as_slice()),
    )
    .unwrap()
}
fn bind(
    signature: ModelSignature,
    bytes: Vec<u8>,
) -> Result<ImportedOnnxResource, ImportedOnnxRefusal> {
    let binding = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&bytes).into(),
        graph_name: "optional-tensor-identity".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("identifiers", "x"), binding("gain", "opt")],
        outputs: vec![binding("prediction", "y")],
    };
    ImportedOnnxResource::bind(resource::resource(signature, bytes), &sidecar)
}
fn inputs(required: usize, optional_width: Option<usize>) -> NamedTensorBundle {
    let mut tensors = vec![values::tensor(
        "identifiers",
        TensorElement::F32,
        &vec![0; required * 4],
    )];
    if let Some(width) = optional_width {
        tensors.push(values::tensor(
            "gain",
            TensorElement::F32,
            &vec![0; width * 4],
        ));
    }
    NamedTensorBundle::from_values(tensors).unwrap()
}
fn outputs(width: usize) -> NamedTensorBundle {
    NamedTensorBundle::from_values(vec![values::tensor(
        "prediction",
        TensorElement::F32,
        &vec![0; width * 4],
    )])
    .unwrap()
}
#[test]
fn optional_input_presence_comes_from_the_graph_and_requires_ir8() {
    for ir in [8, 9] {
        let bytes = graph(ir, &optional(&dynamic_type()), &dynamic_type(), &[]);
        let metadata = inspect(&bytes).unwrap();
        assert_eq!(metadata.inputs[0].presence, ModelPortPresence::Required);
        assert_eq!(metadata.inputs[1].presence, ModelPortPresence::Optional);
        assert_eq!(metadata.outputs[0].presence, ModelPortPresence::Required);
    }
    assert_eq!(
        inspect(&graph(7, &optional(&dynamic_type()), &dynamic_type(), &[])),
        Err(OnnxMetadataRefusal::InvalidVersion)
    );
    assert!(inspect(&graph(7, &dynamic_type(), &dynamic_type(), &[])).is_ok());
    let scalar = tensor_type(Some(vec![]));
    let metadata = inspect(&graph(8, &optional(&scalar), &dynamic_type(), &[])).unwrap();
    assert!(metadata.inputs[1].dimensions.is_empty());
}
#[test]
fn malformed_optional_choices_and_optional_outputs_refuse() {
    use OnnxMetadataRefusal as R;
    let plain = dynamic_type();
    let opt = optional(&plain);
    for (ty, expected) in [
        ([plain.clone(), opt.clone()].concat(), R::DuplicateField),
        ([opt.clone(), opt.clone()].concat(), R::DuplicateField),
        (optional(&optional(&plain)), R::Unsupported),
        (optional(&proto::bytes(4, &[])), R::Unsupported),
        (optional(&proto::bytes(5, &[])), R::Unsupported),
        (optional(&proto::bytes(8, &[])), R::Unsupported),
        (proto::bytes(9, &[]), R::MissingField),
        (
            proto::bytes(
                9,
                &[proto::bytes(1, &plain), proto::bytes(1, &plain)].concat(),
            ),
            R::DuplicateField,
        ),
        (
            optional(&[plain.clone(), plain.clone()].concat()),
            R::DuplicateField,
        ),
        (optional(&tensor_type(None)), R::MissingField),
    ] {
        assert_eq!(inspect(&graph(9, &ty, &plain, &[])), Err(expected));
    }
    assert_eq!(inspect(&graph(9, &opt, &opt, &[])), Err(R::Unsupported));
    let duplicate = [
        proto::bytes(1, b"duplicate"),
        proto::bytes(2, &plain),
        proto::bytes(2, &opt),
    ]
    .concat();
    assert_eq!(
        inspect(&graph(9, &opt, &plain, &proto::bytes(11, &duplicate))),
        Err(R::DuplicateField)
    );
}
#[test]
fn optional_semantic_presence_cannot_weaken_required_or_defaulted_inputs() {
    let optional_graph = graph(9, &optional(&dynamic_type()), &dynamic_type(), &[]);
    assert!(bind(
        signature(ModelPortPresence::Optional),
        optional_graph.clone()
    )
    .is_ok());
    assert_eq!(
        bind(signature(ModelPortPresence::Required), optional_graph).err(),
        Some(ImportedOnnxRefusal::Presence)
    );
    let plain = graph(9, &dynamic_type(), &dynamic_type(), &[]);
    assert_eq!(
        bind(signature(ModelPortPresence::Optional), plain).err(),
        Some(ImportedOnnxRefusal::Presence)
    );
    // An inline initializer is still not permission to omit an ordinary tensor input.
    let initializer = [
        proto::number(1, 1),
        proto::number(2, 1),
        proto::bytes(8, b"opt"),
        proto::bytes(9, &0_f32.to_le_bytes()),
    ]
    .concat();
    let defaulted = graph(
        9,
        &dynamic_type(),
        &dynamic_type(),
        &proto::bytes(5, &initializer),
    );
    assert_eq!(
        inspect(&defaulted).unwrap().inputs[1].presence,
        ModelPortPresence::Required
    );
    assert_eq!(
        bind(signature(ModelPortPresence::Optional), defaulted).err(),
        Some(ImportedOnnxRefusal::Presence)
    );
    assert_ne!(
        signature(ModelPortPresence::Optional)
            .semantic_digest()
            .unwrap(),
        signature(ModelPortPresence::Required)
            .semantic_digest()
            .unwrap()
    );
}
#[test]
fn owned_optional_inputs_preserve_current_invocation_symbols_and_required_outputs() {
    use ImportedOnnxTensorRefusal as R;
    let model = bind(
        signature(ModelPortPresence::Optional),
        graph(9, &optional(&dynamic_type()), &dynamic_type(), &[]),
    )
    .unwrap();
    for (width, optional_width) in [(1, None), (2, Some(2)), (3, None)] {
        model
            .validate_named_inputs(&inputs(width, optional_width))
            .unwrap();
        model
            .validate_named_outputs(&inputs(width, optional_width), &outputs(width))
            .unwrap();
    }
    assert_eq!(
        model.validate_named_inputs(&inputs(2, Some(3))),
        Err(R::SymbolMismatch)
    );
    assert_eq!(
        model.validate_named_outputs(&inputs(2, None), &outputs(3)),
        Err(R::SymbolMismatch)
    );
    assert_eq!(
        model.validate_named_inputs(&inputs(2, Some(33))),
        Err(R::Semantic(ModelTensorIoRefusal::Dimension))
    );
    let empty = NamedTensorBundle::from_values(vec![]).unwrap();
    assert_eq!(
        model.validate_named_inputs(&empty),
        Err(R::Semantic(ModelTensorIoRefusal::MissingPort))
    );
    assert_eq!(
        model.validate_named_outputs(&inputs(2, None), &empty),
        Err(R::Semantic(ModelTensorIoRefusal::MissingPort))
    );
    let wrong_dtype = NamedTensorBundle::from_values(vec![
        values::tensor("identifiers", TensorElement::F32, &[0; 8]),
        values::tensor("gain", TensorElement::I64, &[0; 16]),
    ])
    .unwrap();
    assert_eq!(
        model.validate_named_inputs(&wrong_dtype),
        Err(R::Semantic(ModelTensorIoRefusal::Dtype))
    );
    let extra = NamedTensorBundle::from_values(vec![
        values::tensor("identifiers", TensorElement::F32, &[0; 8]),
        values::tensor("extra", TensorElement::F32, &[0; 8]),
    ])
    .unwrap();
    assert_eq!(
        model.validate_named_inputs(&extra),
        Err(R::Semantic(ModelTensorIoRefusal::UnknownPort))
    );
}
#[test]
fn optional_metadata_does_not_enable_external_data_access() {
    let external = proto::bytes(
        5,
        &[proto::bytes(8, b"weights"), proto::bytes(13, &[])].concat(),
    );
    assert_eq!(
        inspect(&graph(
            9,
            &optional(&dynamic_type()),
            &dynamic_type(),
            &external
        )),
        Err(OnnxMetadataRefusal::ExternalData)
    );
}

#[test]
fn all_optional_inputs_allow_the_existing_empty_bundle_without_inventing_a_symbol() {
    let original = signature(ModelPortPresence::Optional);
    let ports = original
        .inputs
        .get()
        .iter()
        .cloned()
        .map(|mut port| {
            port.presence = ModelPortPresence::Optional;
            port
        })
        .collect();
    let signature = ModelSignature::from_parts(
        original.identity.clone(),
        1,
        vec![conduit_ai::ModelOperation::Infer],
        ports,
        original.outputs.get().as_slice().to_vec(),
    )
    .unwrap();
    // Both optional inputs are unused; y is an explicit inline initializer.
    // This unit checks metadata/owned validation, not an ORT or Play execution.
    let constant = [
        proto::number(1, 1),
        proto::number(2, 1),
        proto::bytes(8, b"y"),
        proto::bytes(9, &0_f32.to_le_bytes()),
    ]
    .concat();
    let body = [
        proto::bytes(2, b"optional-tensor-identity"),
        proto::bytes(5, &constant),
        proto::bytes(11, &port("x", &optional(&dynamic_type()))),
        proto::bytes(11, &port("opt", &optional(&dynamic_type()))),
        proto::bytes(12, &port("y", &dynamic_type())),
    ]
    .concat();
    let bytes = [
        proto::number(1, 9),
        proto::bytes(8, &proto::number(2, 18)),
        proto::bytes(7, &body),
    ]
    .concat();
    let model = bind(signature, bytes).unwrap();
    let empty = NamedTensorBundle::from_values(vec![]).unwrap();
    model.validate_named_inputs(&empty).unwrap();
    model.validate_named_outputs(&empty, &outputs(1)).unwrap();
    // Each call starts with an empty symbol table; no previous output binds it.
    model.validate_named_outputs(&empty, &outputs(2)).unwrap();
    model.validate_named_outputs(&empty, &outputs(1)).unwrap();
}
