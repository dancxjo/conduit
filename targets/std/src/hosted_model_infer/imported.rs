//! Exact imported Back admission precedes provider load and selected inspection.
use super::*;
use conduit_ai::{
    imported_back_admission::{admit_imported_back, ExactImportedBackProfile, ImportedBackRefusal},
    imported_model_descriptor::{ImportedInspectionRefusal, ImportedModelInspection},
    imported_onnx::ImportedOnnxResource,
    ModelComputeRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedModelPrepareRefusal {
    Back(ImportedBackRefusal),
    Compute(ModelComputeRefusal),
    Inspection(ImportedInspectionRefusal),
    Preparation(String),
}

impl<A: ModelComputeAdapter> PreparedModelInfer<A> {
    /// The Host's reviewed profile names an exact checkpoint and actual Back.
    /// It is preparation evidence, not authority supplied by authored Source.
    pub fn prepare_imported(
        model: ImportedOnnxResource,
        adapter: A,
        requirement: ModelComputeRequirement,
        adapter_artifact: ArtifactId,
        profile: &ExactImportedBackProfile,
    ) -> Result<Self, ImportedModelPrepareRefusal> {
        adapter
            .offer()
            .admits(&requirement)
            .map_err(ImportedModelPrepareRefusal::Compute)?;
        let admission = admit_imported_back(
            &model,
            profile,
            MODEL_INFER_IMPLEMENTATION,
            adapter_artifact.as_str(),
            adapter.runtime_identity(),
            MODEL_INFER_PROFILE,
        )
        .map_err(ImportedModelPrepareRefusal::Back)?;
        let property = ImportedModelInspection::from_resource(&model, &admission)
            .and_then(|value| value.to_property())
            .map_err(ImportedModelPrepareRefusal::Inspection)?;
        let mut prepared = Self::prepare_owned(
            ModelCustody::Imported { model, admission },
            adapter,
            requirement,
            adapter_artifact,
        )
        .map_err(ImportedModelPrepareRefusal::Preparation)?;
        prepared.capability.realization_properties.push(property);
        prepared
            .capability
            .realization_properties
            .sort_by(|a, b| a.profile().as_str().cmp(b.profile().as_str()));
        Ok(prepared)
    }
}
