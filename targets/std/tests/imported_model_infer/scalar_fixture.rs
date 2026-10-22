//! Exact pinned Identity graphs, not an arbitrary ONNX interpreter.
use conduit_ai::{imported_onnx::*, *};
use conduit_data::TensorElement;
use conduit_std_host::{
    hosted_model_compute::{
        ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeInvocation,
        ReferenceModelComputeAdapter,
    },
    hosted_model_infer::PreparedModelInfer,
};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[path = "profile.rs"]
mod profile;
#[path = "../../../../semantics/ai/tests/common/onnx_resource_fixture.rs"]
mod resource;
pub struct Probe {
    inner: ReferenceModelComputeAdapter,
    pub calls: Arc<AtomicUsize>,
    reply: Option<Vec<u8>>,
    reader: conduit_data::PreparedNamedTensorBundleView,
}
impl ModelComputeAdapter for Probe {
    fn offer(&self) -> &ModelComputeOffer {
        self.inner.offer()
    }
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.inner.runtime_identity()
    }
    fn load(&mut self, id: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        self.inner.load(id, bytes)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(reply) = &self.reply {
            let Ok(outputs) = self.reader.read(reply) else {
                return ModelComputeAdapterTerminal::Failed;
            };
            return ModelComputeAdapterTerminal::Produced(
                conduit_std_host::hosted_model_compute::ModelComputeExecution {
                    request_identity: invocation.request_identity,
                    artifact_identity: invocation.artifact_identity,
                    checkpoint_identity: None,
                    inputs_identity: invocation.inputs.semantic_digest(),
                    outputs: outputs.encoded_ref(),
                    consumed_work_units: 1,
                    runtime: self.inner.runtime_identity(),
                },
            );
        }
        self.inner.execute(invocation)
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.inner.cancel()
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.inner.unload()
    }
}
pub fn graph(element: TensorElement) -> &'static [u8] {
    match element {
        TensorElement::I64 => include_bytes!("../fixtures/model-scalar-i64.onnx"),
        TensorElement::F32 => include_bytes!("../fixtures/model-scalar-f32.onnx"),
        _ => panic!("the fixture has two reviewed dtype profiles"),
    }
}
pub fn prepare(
    signature: ModelSignature,
    element: TensorElement,
    calls: Arc<AtomicUsize>,
) -> PreparedModelInfer<Probe> {
    prepare_reply(signature, element, calls, None)
}
/// Fault injection preserves the real imported admission and actual identity;
/// only the provider reply is deliberately invalid.
pub fn prepare_reply(
    signature: ModelSignature,
    element: TensorElement,
    calls: Arc<AtomicUsize>,
    reply: Option<Vec<u8>>,
) -> PreparedModelInfer<Probe> {
    let bytes = graph(element);
    let binding = |graph: &str| ImportedPortBinding {
        semantic_port: "scalar".into(),
        graph_name: graph.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: Sha256::digest(bytes).into(),
        graph_name: "bounded-scalar-identity".into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![binding("x")],
        outputs: vec![binding("y")],
    };
    let model = ImportedOnnxResource::bind(resource::resource(signature, bytes.to_vec()), &sidecar)
        .unwrap();
    assert_eq!(model.resource().bytes(), bytes);
    assert!(model.metadata().inputs[0].dimensions.is_empty());
    let mut offer = super::common::offer();
    offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
    offer.supported_elements = vec![element];
    offer.limits.maximum_rank = 0;
    let adapter = Probe {
        inner: ReferenceModelComputeAdapter::new(
            offer,
            super::common::runtime("exact-scalar-identity-fixture"),
        )
        .unwrap(),
        calls,
        reply,
        reader: conduit_data::PreparedNamedTensorBundleView::prepare().unwrap(),
    };
    let mut requirement = super::common::requirement();
    requirement.model_format = ONNX_FORMAT_PROFILE.into();
    requirement.model_bytes = bytes.len() as u64;
    requirement.elements = vec![element];
    requirement.rank = 0;
    requirement.input_bytes = element.byte_width();
    requirement.output_bytes = element.byte_width();
    let artifact = "adapter/exact-scalar-identity-fixture/1";
    let profile = profile::exact(&model, adapter.runtime_identity(), artifact, &["Identity"]);
    PreparedModelInfer::prepare_imported(model, adapter, requirement, artifact.into(), &profile)
        .unwrap()
}
