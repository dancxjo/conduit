//! Only this exact bounded Identity checkpoint is realized by the echo fixture.
use conduit_ai::{imported_onnx::*, *};
use conduit_std_host::hosted_model_compute::ModelComputeAdapter;
use conduit_std_host::{
    hosted_model_compute::ReferenceModelComputeAdapter, hosted_model_infer::PreparedModelInfer,
};
use sha2::{Digest, Sha256};
#[path = "profile.rs"]
mod profile;
#[path = "../../../../semantics/ai/tests/common/onnx_resource_fixture.rs"]
mod resource;
#[path = "../../../../semantics/ai/tests/common/onnx_fixture.rs"]
#[allow(dead_code)]
mod wire;

fn graph() -> Vec<u8> {
    let node = [
        wire::bytes(1, b"x"),
        wire::bytes(2, b"y"),
        wire::bytes(4, b"Identity"),
    ]
    .concat();
    let graph = [
        wire::bytes(1, &node),
        wire::bytes(2, b"bounded-identity"),
        wire::bytes(11, &wire::port("x", 1, wire::bytes(2, b"sequence"))),
        wire::bytes(12, &wire::port("y", 1, wire::bytes(2, b"sequence"))),
    ]
    .concat();
    [
        wire::number(1, 9),
        wire::bytes(8, &wire::number(2, 18)),
        wire::bytes(7, &graph),
    ]
    .concat()
}

pub fn prepare(signature: ModelSignature) -> PreparedModelInfer<ReferenceModelComputeAdapter> {
    let bytes = graph();
    let binding = |graph_name: &str| ImportedPortBinding {
        semantic_port: "input".into(),
        graph_name: graph_name.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(&bytes).into(),
        graph_name: "bounded-identity".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("x")],
        outputs: vec![binding("y")],
    };
    let imported =
        ImportedOnnxResource::bind(resource::resource(signature, bytes), &sidecar).unwrap();
    // The reference echo is correct only for this exact Identity graph and
    // this correspondence, never an arbitrary imported ONNX operator graph.
    assert_eq!(imported.resource().bytes(), graph());
    assert_eq!(imported.sidecar().inputs, vec![binding("x")]);
    assert_eq!(imported.sidecar().outputs, vec![binding("y")]);
    let mut offer = super::common::offer();
    offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
    let mut requirement = super::common::requirement();
    requirement.model_format = ONNX_FORMAT_PROFILE.into();
    requirement.model_bytes = imported.resource().bytes().len() as u64;
    println!("imported-dynamic graph-sha256={:?} signature={:?} sidecar={:?} input-map={:?} output-map={:?}",imported.metadata().sha256,imported.resource().signature().semantic_digest().unwrap(),imported.sidecar_identity(),imported.sidecar().inputs,imported.sidecar().outputs);
    let adapter = ReferenceModelComputeAdapter::new(
        offer,
        super::common::runtime("exact-bounded-identity-fixture"),
    )
    .unwrap();
    let profile = profile::exact(
        &imported,
        adapter.runtime_identity(),
        "adapter/exact-bounded-identity-fixture/1",
        &["Identity"],
    );
    PreparedModelInfer::prepare_imported(
        imported,
        adapter,
        requirement,
        "adapter/exact-bounded-identity-fixture/1".into(),
        &profile,
    )
    .unwrap()
}
