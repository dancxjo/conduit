//! Exact inline Add/Sub checkpoint, not an arbitrary ONNX interpreter.
use conduit_ai::{imported_onnx::*, *};
use conduit_std_host::hosted_model_compute::ModelComputeAdapter;
use conduit_std_host::{
    hosted_model_compute::LinearF32ModelAdapter, hosted_model_infer::PreparedModelInfer,
};
use sha2::{Digest, Sha256};
#[path = "profile.rs"]
mod profile;
#[path = "../../../../semantics/ai/tests/common/onnx_resource_fixture.rs"]
mod resource;
#[path = "../../../../semantics/ai/tests/common/onnx_fixture.rs"]
#[allow(dead_code)]
mod wire;

pub fn graph(swapped: bool) -> Vec<u8> {
    let node = |op: &[u8], output: &[u8]| {
        [
            wire::bytes(1, b"x"),
            wire::bytes(1, b"y"),
            wire::bytes(2, output),
            wire::bytes(4, op),
        ]
        .concat()
    };
    let mut nodes = [node(b"Add", b"sum"), node(b"Sub", b"difference")];
    if swapped {
        nodes.swap(0, 1);
    }
    let scalar = || wire::number(1, 1);
    let graph = [
        wire::bytes(1, &nodes[0]),
        wire::bytes(1, &nodes[1]),
        wire::bytes(2, b"tiny-add-sub"),
        wire::bytes(11, &wire::port("x", 1, scalar())),
        wire::bytes(11, &wire::port("y", 1, scalar())),
        wire::bytes(12, &wire::port("sum", 1, scalar())),
        wire::bytes(12, &wire::port("difference", 1, scalar())),
    ]
    .concat();
    [
        wire::number(1, 9),
        wire::bytes(8, &wire::number(2, 18)),
        wire::bytes(7, &graph),
    ]
    .concat()
}

pub fn imported(
    signature: ModelSignature,
    swapped: bool,
    reverse_binding: bool,
) -> ImportedOnnxResource {
    let bytes = graph(swapped);
    let bind = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    let inputs = if reverse_binding {
        vec![bind("left", "y"), bind("right", "x")]
    } else {
        vec![bind("left", "x"), bind("right", "y")]
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&bytes).into(),
        graph_name: "tiny-add-sub".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs,
        outputs: vec![bind("upper", "sum"), bind("lower", "difference")],
    };
    ImportedOnnxResource::bind(resource::resource(signature, bytes), &sidecar).unwrap()
}

pub fn prepare(model: ImportedOnnxResource) -> PreparedModelInfer<LinearF32ModelAdapter> {
    // This deliberately tiny fixture Back recognizes both exact protobuf node
    // orders. It does not claim support for any other model or ONNX operator.
    assert!(model.resource().bytes() == graph(false) || model.resource().bytes() == graph(true));
    assert_eq!(
        model
            .sidecar()
            .inputs
            .iter()
            .map(|b| (b.semantic_port.as_str(), b.graph_name.as_str()))
            .collect::<Vec<_>>(),
        if model.sidecar().inputs[0].graph_name == "y" {
            vec![("left", "y"), ("right", "x")]
        } else {
            vec![("left", "x"), ("right", "y")]
        }
    );
    assert_eq!(
        model
            .sidecar()
            .outputs
            .iter()
            .map(|b| (b.semantic_port.as_str(), b.graph_name.as_str()))
            .collect::<Vec<_>>(),
        vec![("upper", "sum"), ("lower", "difference")]
    );
    let mut offer = super::common::offer();
    offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
    let mut requirement = super::common::requirement();
    requirement.model_format = ONNX_FORMAT_PROFILE.into();
    requirement.model_bytes = model.resource().bytes().len() as u64;
    let reverse = model.sidecar().inputs[0].graph_name == "y";
    let matrix = if reverse {
        [[1., 1.], [-1., 1.]]
    } else {
        [[1., 1.], [1., -1.]]
    };
    let adapter = LinearF32ModelAdapter::new(
        offer,
        super::common::runtime("exact-add-sub-fixture"),
        matrix,
        [0., 0.],
    )
    .unwrap()
    .with_signature(model.resource().signature())
    .unwrap();
    let profile = profile::exact(
        &model,
        adapter.runtime_identity(),
        "adapter/exact-add-sub-fixture/1",
        &["Add", "Sub"],
    );
    PreparedModelInfer::prepare_imported(
        model,
        adapter,
        requirement,
        "adapter/exact-add-sub-fixture/1".into(),
        &profile,
    )
    .unwrap()
}
