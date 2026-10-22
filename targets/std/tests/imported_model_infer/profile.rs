//! Reviewed fixture Back declarations; never supplied by authored Source.
use conduit_ai::{
    imported_back_admission::{
        ExactImportedBackProfile, ImportedCheckpointIdentity, ImportedOpsetRange,
    },
    imported_onnx::ImportedOnnxResource,
    onnx_metadata::OnnxOperator,
    ModelComputeRuntimeIdentity,
};
use conduit_std_host::hosted_model_infer::{MODEL_INFER_IMPLEMENTATION, MODEL_INFER_PROFILE};

pub fn exact(
    model: &ImportedOnnxResource,
    runtime: &ModelComputeRuntimeIdentity,
    artifact: &str,
    operators: &[&str],
) -> ExactImportedBackProfile {
    ExactImportedBackProfile {
        profile_identity: "fixture/exact-onnx@1".into(),
        implementation_identity: MODEL_INFER_IMPLEMENTATION.into(),
        adapter_artifact_identity: artifact.into(),
        runtime: runtime.clone(),
        execution_profile: MODEL_INFER_PROFILE.into(),
        checkpoint: ImportedCheckpointIdentity::from_resource(model),
        minimum_ir: 9,
        maximum_ir: 9,
        opsets: vec![ImportedOpsetRange {
            domain: "".into(),
            minimum: 18,
            maximum: 18,
        }],
        operators: operators
            .iter()
            .map(|name| OnnxOperator {
                domain: "".into(),
                name: (*name).into(),
            })
            .collect(),
    }
}
