//! One admitted native CPU Back behind the existing named-inference boundary.
//! One pending operation retains its exact transport generation until take or retirement.
mod profile;
use crate::{
    hosted_model_compute::{
        ModelComputeAdapter, ModelComputeAdapterTerminal, ModelComputeExecution,
        ModelComputeInvocation,
    },
    hosted_model_infer::{ImportedModelPrepareRefusal, PreparedModelInfer},
    native_model_backing::{
        NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile, NativeRequestId,
        PreparedNativeModel,
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
    pending: Option<PendingNative>,
    retired: bool,
    cancelled: bool,
}

#[derive(Clone, Copy)]
struct PendingNative {
    token: NativeRequestId,
    request: [u8; 32],
    inputs: [u8; 32],
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
        pending: None,
        retired: false,
        cancelled: false,
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
        if self.cancelled {
            return ModelComputeAdapterTerminal::Cancelled;
        }
        if self.retired {
            return ModelComputeAdapterTerminal::ProviderLost;
        }
        if self.pending.is_some() {
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::QueueFull);
        }
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
            // A reference carries no tensor-read permission. This installed
            // profile owns only materialized inline I/O; never open storage.
            return ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::UnsupportedFormat);
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
        let result = native.begin_f32(f32::from_le_bytes(
            payload.try_into().expect("exact scalar width"),
        ));
        match result {
            Ok(token) => {
                self.pending = Some(PendingNative {
                    token,
                    request: invocation.request_identity,
                    inputs: invocation.inputs.semantic_digest(),
                });
                ModelComputeAdapterTerminal::Pending
            }
            Err(NativeBackingRefusal::Capacity) => {
                let _ = self.session.finish();
                ModelComputeAdapterTerminal::Refused(ModelComputeRefusal::ResourceBoundExceeded)
            }
            Err(error) => self.fail(error),
        }
    }
    fn poll(&mut self) -> ModelComputeAdapterTerminal<'_> {
        if self.cancelled {
            return ModelComputeAdapterTerminal::Cancelled;
        }
        if self.retired {
            return ModelComputeAdapterTerminal::ProviderLost;
        }
        let Some(pending) = self.pending else {
            return ModelComputeAdapterTerminal::Failed;
        };
        let Some(native) = self.native.as_mut() else {
            return self.fail(NativeBackingRefusal::Protocol);
        };
        match native.poll_f32(pending.token) {
            Ok(false) => return ModelComputeAdapterTerminal::Pending,
            Ok(true) => {}
            Err(error) => return self.fail(error),
        }
        let value = match native.take_f32(pending.token) {
            Ok(Some(value)) => value,
            Ok(None) => return ModelComputeAdapterTerminal::Pending,
            Err(error) => return self.fail(error),
        };
        self.pending = None;
        if self.session.finish().is_err()
            || self.output.write("scalar", &value.to_le_bytes()).is_err()
        {
            return self.fail(NativeBackingRefusal::InvalidOutput);
        }
        let Ok(outputs) = self.output.view() else {
            return ModelComputeAdapterTerminal::Failed;
        };
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: pending.request,
            artifact_identity: self.artifact,
            checkpoint_identity: None,
            inputs_identity: pending.inputs,
            outputs: outputs.encoded_ref(),
            consumed_work_units: 1,
            runtime: self.session.runtime(),
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        // Revoke the publication token before backend cancellation/pipe disposal.
        self.cancelled = true;
        self.retired = true;
        let pending = self.pending.take();
        if let Some(mut native) = self.native.take() {
            if let Some(pending) = pending {
                let _ = native.cancel(pending.token);
            }
            let _ = native.unload();
        }
        self.output.reset();
        self.session.provider_lost();
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        if self.retired && self.native.is_none() {
            return Ok(());
        }
        self.retired = true;
        self.pending = None;
        // The generic machine permits clean Ready -> Unloading -> Shutdown.
        // Active/lost retirement remains Lost; never pretend the worker is reusable.
        let clean = self.session.begin_unload().is_ok();
        if let Some(mut native) = self.native.take() {
            let _ = native.unload();
        }
        self.output.reset();
        if clean {
            self.session.shutdown()
        } else {
            self.session.provider_lost();
            Ok(())
        }
    }
}

impl NativeCpuModelAdapter {
    fn fail(&mut self, error: NativeBackingRefusal) -> ModelComputeAdapterTerminal<'_> {
        self.pending = None;
        self.retired = true;
        self.session.provider_lost();
        if let Some(mut native) = self.native.take() {
            let _ = native.unload();
        }
        self.output.reset();
        if error == NativeBackingRefusal::InvalidOutput {
            ModelComputeAdapterTerminal::Failed
        } else {
            ModelComputeAdapterTerminal::ProviderLost
        }
    }
}
impl Drop for NativeCpuModelAdapter {
    fn drop(&mut self) {
        let _ = self.unload();
    }
}
