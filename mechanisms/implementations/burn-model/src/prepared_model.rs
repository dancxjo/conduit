//! Admit and initialize a model before publishing its content-bound base artifact.
use crate::{AuthoringDescriptor, BurnModelDefinition, DeviceRequest, Error, OptimizerRecipe};
use burn::{
    module::Module,
    store::{ModuleSnapshot, SafetensorsStore},
    tensor::Device,
};
use sha2::{Digest, Sha256};

/// Host preparation can durably publish these exact weights and construct the
/// existing ModelArtifact/TrainingSession identities before admitting the adapter.
/// Consuming the prepared model avoids repeating random initialization.
pub struct PreparedBurnModel<D: BurnModelDefinition> {
    pub(crate) definition: D,
    pub(crate) descriptor: AuthoringDescriptor,
    pub(crate) device: Device,
    pub(crate) request: DeviceRequest,
    pub(crate) recipe: OptimizerRecipe,
    pub(crate) model: D::Model,
    pub(crate) weights: Vec<u8>,
}
impl<D: BurnModelDefinition> PreparedBurnModel<D> {
    pub fn initialize(
        definition: D,
        request: DeviceRequest,
        recipe: OptimizerRecipe,
    ) -> Result<Self, Error> {
        let descriptor = definition.descriptor();
        descriptor.validate()?;
        recipe.validate()?;
        if !descriptor.supported_profiles.contains(&request.evidence()) {
            return Err(Error::UnsupportedDevice);
        }
        let device = request.prepare()?.autodiff();
        device.seed(recipe.seed);
        let mut model = definition.initialize(&device)?.train();
        for group in &descriptor.groups {
            let binding = definition.parameter_group(&model, &group.identity)?;
            if !group.trainable {
                model = model.freeze_group(binding);
            }
        }
        if (model.num_params() as u64)
            .checked_mul(4)
            .ok_or(Error::ResourceBound)?
            > descriptor.resources.model_bytes
        {
            return Err(Error::ResourceBound);
        }
        let record = model
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|_| Error::NumericFailure)?;
        if record.len() as u64 > descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        crate::record_validation::validate_record(record, false)?;
        let mut store = SafetensorsStore::from_bytes(None).clear_metadata();
        model
            .save_into(&mut store)
            .map_err(|_| Error::NumericFailure)?;
        let weights = store.get_bytes().map_err(|_| Error::NumericFailure)?;
        if weights.len() as u64 > descriptor.resources.checkpoint_bytes {
            return Err(Error::ResourceBound);
        }
        Ok(Self {
            definition,
            descriptor,
            device,
            request,
            recipe,
            model,
            weights,
        })
    }
    pub fn weights(&self) -> &[u8] {
        &self.weights
    }
    pub fn content_identity(&self) -> [u8; 32] {
        Sha256::digest(&self.weights).into()
    }
    pub fn descriptor(&self) -> &AuthoringDescriptor {
        &self.descriptor
    }
}
