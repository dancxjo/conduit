//! Exact imported Back admission precedes provider load and selected inspection.
use super::*;
use conduit_ai::{
    imported_back_admission::{
        admit_imported_back, ExactImportedBackProfile, ImportedBackAdmissionReceipt,
        ImportedBackRefusal,
    },
    imported_model_descriptor::{ImportedInspectionRefusal, ImportedModelInspection},
    imported_onnx::ImportedOnnxResource,
    ModelComputeRefusal,
};
use std::sync::Arc;

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
        let (admission, property) =
            Self::admit_imported(&model, &adapter, &requirement, &adapter_artifact, profile)?;
        Self::finish_imported(
            ModelCustody::Imported { model, admission },
            adapter,
            requirement,
            adapter_artifact,
            property,
        )
    }

    /// Shares the exact already admitted model with a prepared native provider.
    /// Neither owner recreates model bytes, descriptors or read authority.
    pub fn prepare_imported_shared(
        model: Arc<ImportedOnnxResource>,
        adapter: A,
        requirement: ModelComputeRequirement,
        adapter_artifact: ArtifactId,
        profile: &ExactImportedBackProfile,
    ) -> Result<Self, ImportedModelPrepareRefusal> {
        let (admission, property) =
            Self::admit_imported(&model, &adapter, &requirement, &adapter_artifact, profile)?;
        Self::finish_imported(
            ModelCustody::SharedImported { model, admission },
            adapter,
            requirement,
            adapter_artifact,
            property,
        )
    }

    fn admit_imported(
        model: &ImportedOnnxResource,
        adapter: &A,
        requirement: &ModelComputeRequirement,
        adapter_artifact: &ArtifactId,
        profile: &ExactImportedBackProfile,
    ) -> Result<
        (ImportedBackAdmissionReceipt, StructuredConfigurationValue),
        ImportedModelPrepareRefusal,
    > {
        adapter
            .offer()
            .admits(requirement)
            .map_err(ImportedModelPrepareRefusal::Compute)?;
        let admission = admit_imported_back(
            model,
            profile,
            MODEL_INFER_IMPLEMENTATION,
            adapter_artifact.as_str(),
            adapter.runtime_identity(),
            MODEL_INFER_PROFILE,
        )
        .map_err(ImportedModelPrepareRefusal::Back)?;
        let property = ImportedModelInspection::from_resource(model, &admission)
            .and_then(|value| value.to_property())
            .map_err(ImportedModelPrepareRefusal::Inspection)?;
        Ok((admission, property))
    }

    fn finish_imported(
        model: ModelCustody,
        adapter: A,
        requirement: ModelComputeRequirement,
        adapter_artifact: ArtifactId,
        property: StructuredConfigurationValue,
    ) -> Result<Self, ImportedModelPrepareRefusal> {
        let mut prepared = Self::prepare_owned(model, adapter, requirement, adapter_artifact)
            .map_err(ImportedModelPrepareRefusal::Preparation)?;
        prepared.capability.realization_properties.push(property);
        prepared
            .capability
            .realization_properties
            .sort_by(|a, b| a.profile().as_str().cmp(b.profile().as_str()));
        Ok(prepared)
    }
}
