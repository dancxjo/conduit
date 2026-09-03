//! Minimal std Host bridge for provider-neutral model artifacts.

use conduit_ai::{
    model_content_digest, ModelArtifact, ModelCompatibilityRefusal, ModelInvocationEvidence,
    ModelInvocationTerminal, ModelOperation, ModelRuntimeRealization, ModelSignature,
};
use conduit_core::BoundedResourceRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedModelRefusal {
    Compatibility(ModelCompatibilityRefusal),
    UnsupportedOperation,
    ResourceUnavailable,
    ResourceExtentMismatch,
    ResourceContentMismatch,
    EmptyInputIdentity,
    WorkNotAdmitted,
    AdapterRefused,
    AdapterFailed,
}

pub trait ModelArtifactStore {
    fn load(&self, reference: &BoundedResourceRef) -> Result<Vec<u8>, HostedModelRefusal>;
}

pub trait HostedModelAdapter {
    fn realization(&self) -> &ModelRuntimeRealization;

    fn invoke(
        &mut self,
        artifact_bytes: &[u8],
        operation: ModelOperation,
        input: &[u8],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, HostedModelRefusal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedModelResult {
    pub output: Vec<u8>,
    pub evidence: ModelInvocationEvidence,
}

#[allow(clippy::too_many_arguments)]
pub fn invoke_hosted_model(
    store: &impl ModelArtifactStore,
    adapter: &mut impl HostedModelAdapter,
    artifact: &ModelArtifact,
    signature: &ModelSignature,
    operation: ModelOperation,
    input_identity: [u8; 32],
    input: &[u8],
    admitted_work_units: u64,
    maximum_output_bytes: usize,
) -> Result<HostedModelResult, HostedModelRefusal> {
    artifact
        .validate(signature)
        .map_err(HostedModelRefusal::Compatibility)?;
    if !signature.operations.contains(&operation) {
        return Err(HostedModelRefusal::UnsupportedOperation);
    }
    if input_identity == [0; 32] {
        return Err(HostedModelRefusal::EmptyInputIdentity);
    }
    if admitted_work_units == 0 || maximum_output_bytes == 0 {
        return Err(HostedModelRefusal::WorkNotAdmitted);
    }
    adapter
        .realization()
        .admit(artifact)
        .map_err(HostedModelRefusal::Compatibility)?;
    let artifact_bytes = store.load(&artifact.content)?;
    if u64::try_from(artifact_bytes.len()).ok() != Some(artifact.content.extent.bytes) {
        return Err(HostedModelRefusal::ResourceExtentMismatch);
    }
    if model_content_digest(&artifact_bytes) != artifact.content_identity() {
        return Err(HostedModelRefusal::ResourceContentMismatch);
    }
    let output = adapter.invoke(&artifact_bytes, operation, input, maximum_output_bytes)?;
    if output.len() > maximum_output_bytes {
        return Err(HostedModelRefusal::AdapterFailed);
    }
    let result = HostedModelResult {
        output,
        evidence: ModelInvocationEvidence {
            artifact_identity: artifact.content_identity(),
            checkpoint_identity: None,
            signature_identity: artifact.signature_identity,
            runtime_implementation_identity: adapter.realization().implementation_identity.clone(),
            runtime_build_identity: adapter.realization().runtime_build_identity.clone(),
            precision_profile: artifact.precision_profile.clone(),
            operation,
            input_identities: vec![input_identity],
            stochastic_seed: None,
            admitted_work_units,
            terminal: ModelInvocationTerminal::Produced,
        },
    };
    result
        .evidence
        .validate()
        .map_err(|_| HostedModelRefusal::AdapterFailed)?;
    Ok(result)
}
