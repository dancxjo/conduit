//! Two exact pinned Identity graphs; dynamic optional inputs are unused.
use conduit_ai::{imported_onnx::*, *};
use conduit_data::PreparedNamedTensorBundleOutput;
use conduit_std_host::{
    hosted_model_compute::{
        ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeExecution,
        ModelComputeInvocation, ReferenceModelComputeAdapter,
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

pub struct Identity {
    inner: ReferenceModelComputeAdapter,
    output: PreparedNamedTensorBundleOutput,
    calls: Arc<AtomicUsize>,
    constant: bool,
}
impl ModelComputeAdapter for Identity {
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
        // Retain the existing reference session's identity, lifecycle and work
        // checks. Its echo is deliberately not published as the graph reply.
        if !matches!(
            self.inner.execute(invocation.clone()),
            ModelComputeAdapterTerminal::Produced(_)
        ) {
            return ModelComputeAdapterTerminal::Failed;
        }
        self.output.reset();
        let result = if self.constant {
            let bytes = [0, 0, 224, 64, 0, 0, 48, 65];
            self.output.write("input", &bytes)
        } else {
            let Some(required) = invocation
                .inputs
                .values()
                .find(|value| value.name == "input")
            else {
                return ModelComputeAdapterTerminal::Failed;
            };
            self.output.write_view(required)
        };
        if result.is_err() {
            return ModelComputeAdapterTerminal::Failed;
        }
        let Ok(outputs) = self.output.view() else {
            return ModelComputeAdapterTerminal::Failed;
        };
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: invocation.artifact_identity,
            checkpoint_identity: None,
            inputs_identity: invocation.inputs.semantic_digest(),
            outputs: outputs.encoded_ref(),
            consumed_work_units: 1,
            runtime: self.inner.runtime_identity(),
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.inner.cancel()
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.inner.unload()
    }
}

pub fn prepare(signature: ModelSignature, calls: Arc<AtomicUsize>) -> PreparedModelInfer<Identity> {
    prepare_bounded(signature, calls, 16).unwrap()
}

pub fn prepare_bounded(
    signature: ModelSignature,
    calls: Arc<AtomicUsize>,
    input_bytes: u64,
) -> Result<PreparedModelInfer<Identity>, String> {
    prepare_variant(signature, calls, input_bytes, false)
}

pub fn prepare_constant(
    signature: ModelSignature,
    calls: Arc<AtomicUsize>,
) -> PreparedModelInfer<Identity> {
    prepare_variant(signature, calls, 16, true).unwrap()
}

fn prepare_variant(
    signature: ModelSignature,
    calls: Arc<AtomicUsize>,
    input_bytes: u64,
    constant: bool,
) -> Result<PreparedModelInfer<Identity>, String> {
    let bytes: &[u8] = if constant {
        include_bytes!("../fixtures/model-optional-constant.onnx")
    } else {
        include_bytes!("../fixtures/model-optional-identity.onnx")
    };
    let graph_name = if constant {
        "optional-constant"
    } else {
        "optional-identity"
    };
    let expected = if constant {
        [
            0xc4, 0x0c, 0x85, 0x15, 0xaf, 0xe7, 0xb8, 0xeb, 0x54, 0xc7, 0x6e, 0xe4, 0xbb, 0x54,
            0x5f, 0x7e, 0x2a, 0x8f, 0x02, 0xa7, 0x40, 0xe7, 0xb1, 0x68, 0xfd, 0xb5, 0x8c, 0xb7,
            0x95, 0x40, 0x05, 0xd7,
        ]
    } else {
        [
            0x3a, 0x2f, 0x4e, 0x7e, 0xb0, 0x5c, 0xd4, 0x02, 0xae, 0x2d, 0xab, 0x1e, 0x0f, 0x6b,
            0xf2, 0x88, 0x16, 0x86, 0xe1, 0xf3, 0x17, 0xd4, 0x3c, 0x0c, 0xad, 0xdc, 0xe8, 0x5d,
            0x26, 0xd3, 0xeb, 0x49,
        ]
    };
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    // This adapter implements exactly these graphs, not arbitrary Identity wiring.
    assert_eq!(digest, expected);
    let bind = |semantic: &str, graph: &str| ImportedPortBinding {
        semantic_port: semantic.into(),
        graph_name: graph.into(),
    };
    let sidecar = OnnxSidecar {
        graph_sha256: digest,
        graph_name: graph_name.into(),
        signature_identity: signature.semantic_digest().unwrap(),
        inputs: vec![bind("input", "x"), bind("hint", "hint")],
        outputs: vec![bind("input", "y")],
    };
    let imported =
        ImportedOnnxResource::bind(resource::resource(signature, bytes.to_vec()), &sidecar)
            .unwrap();
    assert_eq!(imported.resource().bytes(), bytes);
    assert_eq!(
        imported.metadata().inputs[1].presence,
        ModelPortPresence::Optional
    );
    let mut offer = super::common::offer();
    offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
    offer.limits.maximum_input_bytes = 16;
    offer.limits.maximum_output_bytes = 16;
    offer.limits.maximum_queue_bytes = 16;
    let adapter = Identity {
        inner: ReferenceModelComputeAdapter::new(
            offer,
            super::common::runtime("exact-optional-identity-fixture"),
        )
        .unwrap(),
        output: PreparedNamedTensorBundleOutput::prepare(&super::common::bundle(
            super::common::input([0., 0.]),
        ))
        .unwrap(),
        calls,
        constant,
    };
    let mut requirement = super::common::requirement();
    requirement.model_format = ONNX_FORMAT_PROFILE.into();
    requirement.model_bytes = bytes.len() as u64;
    // Charge every possible input before Play, including an absent hint.
    requirement.input_bytes = input_bytes;
    // The internal echo control admits both inputs; the exact graph reply is
    // still only the eight-byte required tensor, checked by its signature.
    requirement.output_bytes = 16;
    let artifact = "adapter/exact-optional-identity-fixture/1";
    let profile = profile::exact(
        &imported,
        adapter.runtime_identity(),
        artifact,
        &["Identity"],
    );
    PreparedModelInfer::prepare_imported(imported, adapter, requirement, artifact.into(), &profile)
        .map_err(|error| format!("{error:?}"))
}
