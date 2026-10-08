//! Generic exact integer categorical inference at the hosted model boundary.
use crate::hosted_model::{HostedModelAdapter, HostedModelRefusal};
use conduit_ai::{
    integer_categorical::IntegerCategoricalModel, ModelArtifact, ModelComputeOffer,
    ModelComputeRequirement, ModelComputeRuntimeIdentity, ModelComputeSession,
    ModelRuntimeRealization, ModelSignature,
};
use conduit_data::TensorValue;

pub struct HostedIntegerCategorical {
    model: IntegerCategoricalModel,
    session: ModelComputeSession,
    requirement: ModelComputeRequirement,
    realization: ModelRuntimeRealization,
}
impl HostedIntegerCategorical {
    pub fn prepare(
        artifact: &ModelArtifact,
        signature: &ModelSignature,
        bytes: &[u8],
        offer: ModelComputeOffer,
        runtime: ModelComputeRuntimeIdentity,
        requirement: ModelComputeRequirement,
        realization: ModelRuntimeRealization,
    ) -> Result<Self, HostedModelRefusal> {
        let model = IntegerCategoricalModel::prepare(artifact, signature, bytes)
            .map_err(|_| HostedModelRefusal::AdapterRefused)?;
        realization
            .admit(artifact)
            .map_err(HostedModelRefusal::Compatibility)?;
        if requirement.working_memory_bytes < model.working_bytes()
            || requirement.model_bytes != bytes.len() as u64
        {
            return Err(HostedModelRefusal::WorkNotAdmitted);
        }
        offer
            .admits(&requirement)
            .map_err(|_| HostedModelRefusal::WorkNotAdmitted)?;
        let mut session = ModelComputeSession::discovered(offer, runtime)
            .map_err(|_| HostedModelRefusal::WorkNotAdmitted)?;
        session
            .begin_load(model.identity(), bytes.len() as u64)
            .and_then(|()| session.begin_warming())
            .and_then(|()| session.ready())
            .map_err(|_| HostedModelRefusal::WorkNotAdmitted)?;
        Ok(Self {
            model,
            session,
            requirement,
            realization,
        })
    }
    pub fn work_units(&self) -> u64 {
        self.model.work_units()
    }
}
impl HostedModelAdapter for HostedIntegerCategorical {
    fn realization(&self) -> &ModelRuntimeRealization {
        &self.realization
    }
    fn invoke(
        &mut self,
        artifact_bytes: &[u8],
        checkpoint: Option<&[u8]>,
        operation: conduit_ai::ModelOperation,
        input: &[u8],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, HostedModelRefusal> {
        if checkpoint.is_some()
            || operation != conduit_ai::ModelOperation::Infer
            || conduit_ai::model_content_digest(artifact_bytes) != self.model.identity()
        {
            return Err(HostedModelRefusal::AdapterRefused);
        }
        let tensor = TensorValue::decode(input).map_err(|_| HostedModelRefusal::AdapterRefused)?;
        if tensor.byte_count().ok() != Some(self.requirement.input_bytes) {
            return Err(HostedModelRefusal::WorkNotAdmitted);
        }
        self.session
            .begin(&self.requirement, 0)
            .map_err(|_| HostedModelRefusal::WorkNotAdmitted)?;
        let result = self.model.infer(&tensor);
        self.session
            .finish()
            .map_err(|_| HostedModelRefusal::AdapterFailed)?;
        let output = result.map_err(|_| HostedModelRefusal::AdapterRefused)?;
        if output.byte_count().ok() != Some(self.requirement.output_bytes) {
            return Err(HostedModelRefusal::WorkNotAdmitted);
        }
        let bytes = output
            .encode()
            .map_err(|_| HostedModelRefusal::AdapterFailed)?;
        if bytes.len() > maximum_output_bytes {
            return Err(HostedModelRefusal::WorkNotAdmitted);
        }
        Ok(bytes)
    }
}
