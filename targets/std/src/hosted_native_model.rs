//! One admitted native CPU Back behind the existing named-inference boundary.
//! Interruptible cancellation and cross-session retirement remain separate.
mod profile;
use crate::{
    hosted_model_compute::{
        ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeExecution,
        ModelComputeInvocation,
    },
    hosted_model_infer::{ImportedModelPrepareRefusal, PreparedModelInfer},
    native_model_backing::{
        NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile, PreparedNativeModel,
    },
};
use conduit_ai::{imported_onnx::ImportedOnnxResource, *};
use conduit_data::{PreparedNamedTensorBundleOutput, TensorBackingView, TensorElement};
use std::{path::Path, sync::Arc};

#[derive(Debug)]
pub enum NativeModelPrepareRefusal {
    Native(NativeBackingRefusal),
    Compute(ModelComputeRefusal),
    Imported(ImportedModelPrepareRefusal),
}

/// The existing session owns lifecycle truth. Native storage and pipe framing
/// remain in the already prepared, single-in-flight backing resource.
pub struct NativeCpuModelAdapter {
    native: Option<PreparedNativeModel>,
    offer: ModelComputeOffer,
    session: ModelComputeSession,
    requirement: ModelComputeRequirement,
    artifact: [u8; 32],
    output: PreparedNamedTensorBundleOutput,
}

/// Host preparation only: executable/runtime paths never enter authored Source.
/// The selected capability and provider retain the same immutable model owner.
pub fn prepare_native_model(
    model: ImportedOnnxResource,
    executable: &Path,
    native_profile: NativeCpuProfile,
    grant: NativeBackingGrant,
) -> Result<PreparedModelInfer<NativeCpuModelAdapter>, NativeModelPrepareRefusal> {
    profile::validate_model(&model).map_err(NativeModelPrepareRefusal::Compute)?;
    let model = Arc::new(model);
    let native =
        PreparedNativeModel::prepare(executable, native_profile.clone(), grant, model.clone())
            .map_err(NativeModelPrepareRefusal::Native)?;
    let ledger = native.ledger();
    let bytes = ledger
        .private_bytes
        .checked_add(ledger.image_mapping_bytes)
        .and_then(|v| v.checked_add(ledger.other_mapping_bytes))
        .ok_or(NativeModelPrepareRefusal::Compute(
            ModelComputeRefusal::ResourceBoundExceeded,
        ))?;
    let offer = profile::offer(bytes);
    let runtime = profile::runtime(native_profile.runtime_sha256);
    let exact = profile::exact(&model, &runtime);
    let session = ModelComputeSession::discovered(offer.clone(), runtime)
        .map_err(NativeModelPrepareRefusal::Compute)?;
    let requirement = profile::requirement(bytes);
    let template = profile::output_template();
    let output = PreparedNamedTensorBundleOutput::prepare(&template)
        .map_err(|_| NativeModelPrepareRefusal::Compute(ModelComputeRefusal::UnsupportedShape))?;
    let adapter = NativeCpuModelAdapter {
        artifact: model.resource().artifact().content_identity(),
        native: Some(native),
        offer,
        session,
        requirement: requirement.clone(),
        output,
    };
    PreparedModelInfer::prepare_imported_shared(
        model,
        adapter,
        requirement,
        profile::ARTIFACT.into(),
        &exact,
    )
    .map_err(NativeModelPrepareRefusal::Imported)
}

impl ModelComputeAdapter for NativeCpuModelAdapter {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.session.runtime()
    }
    fn load(&mut self, artifact: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        if artifact != self.artifact || bytes != self.requirement.model_bytes {
            return Err(ModelComputeRefusal::ProviderUnavailable);
        }
        self.session.begin_load(artifact, bytes)?;
        self.session.begin_warming()?;
        self.session.ready()
    }
    fn output_template(
        &self,
    ) -> Result<Option<conduit_data::NamedTensorBundle>, ModelComputeRefusal> {
        Ok(Some(profile::output_template()))
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        if invocation.request_identity == [0; 32]
            || invocation.artifact_identity != self.artifact
            || invocation.checkpoint_identity.is_some()
            || invocation.requirement != &self.requirement
        {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ProviderUnavailable);
        }
        let mut inputs = invocation.inputs.values();
        let Some(input) = inputs.next() else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        };
        let TensorBackingView::Inline(payload) = input.backing else {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        };
        if inputs.next().is_some()
            || input.name != "scalar"
            || input.element != TensorElement::F32
            || !input.dimensions().is_empty()
            || payload.len() != 4
        {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedShape);
        }
        if let Err(error) = self.session.begin(invocation.requirement, 0) {
            return ModelComputeAdapterTerminal::Refused(error);
        }
        self.output.reset();
        let Some(native) = self.native.as_mut() else {
            self.session.provider_lost();
            return ModelComputeAdapterTerminal::ProviderLost;
        };
        let result = native.infer_f32(f32::from_le_bytes(
            payload.try_into().expect("exact scalar width"),
        ));
        let value = match result {
            Ok(value) => value,
            Err(NativeBackingRefusal::Capacity) => {
                let _ = self.session.finish();
                return ModelComputeAdapterTerminal::Refused(
                    ModelComputeRefusal::ResourceBoundExceeded,
                );
            }
            Err(error) => {
                self.session.provider_lost();
                drop(self.native.take());
                return if error == NativeBackingRefusal::InvalidOutput {
                    ModelComputeAdapterTerminal::Failed
                } else {
                    ModelComputeAdapterTerminal::ProviderLost
                };
            }
        };
        if self.session.finish().is_err()
            || self.output.write("scalar", &value.to_le_bytes()).is_err()
        {
            return ModelComputeAdapterTerminal::Failed;
        }
        let Ok(outputs) = self.output.view() else {
            return ModelComputeAdapterTerminal::Failed;
        };
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: invocation.request_identity,
            artifact_identity: self.artifact,
            checkpoint_identity: None,
            inputs_identity: invocation.inputs.semantic_digest(),
            outputs: outputs.encoded_ref(),
            consumed_work_units: 1,
            runtime: self.session.runtime(),
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        // This synchronous profile does not advertise interruptible cancellation.
        Err(ModelComputeRefusal::CancellationUnsupported)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.session.begin_unload()?;
        drop(self.native.take());
        self.session.shutdown()
    }
}
