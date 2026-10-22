//! Preparation evidence for one reviewed imported checkpoint and exact Back.
//! A receipt does not grant execution authority or certify unreviewed graphs.
use crate::{
    imported_onnx::ImportedOnnxResource,
    onnx_metadata::{inspect_onnx_operators, OnnxMetadataRefusal, OnnxOperator},
    ModelComputeRuntimeIdentity, MAXIMUM_MODEL_IDENTITY_BYTES,
};
use alloc::{string::String, vec::Vec};
use conduit_core::semantic_digest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedCheckpointIdentity {
    pub descriptor: [u8; 32],
    pub graph: [u8; 32],
    pub signature: [u8; 32],
    pub sidecar: [u8; 32],
}
impl ImportedCheckpointIdentity {
    pub fn from_resource(resource: &ImportedOnnxResource) -> Self {
        Self {
            descriptor: resource.resource().descriptor_identity(),
            graph: resource.metadata().sha256,
            signature: resource.sidecar().signature_identity,
            sidecar: resource.sidecar_identity(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedOpsetRange {
    pub domain: String,
    pub minimum: u64,
    pub maximum: u64,
}
/// A reviewed profile of an actual adapter, including its exact checkpoint.
/// Supporting an operator name is never permission to substitute another graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactImportedBackProfile {
    pub profile_identity: String,
    pub implementation_identity: String,
    pub adapter_artifact_identity: String,
    pub runtime: ModelComputeRuntimeIdentity,
    pub execution_profile: String,
    pub checkpoint: ImportedCheckpointIdentity,
    pub minimum_ir: u64,
    pub maximum_ir: u64,
    pub opsets: Vec<ImportedOpsetRange>,
    pub operators: Vec<OnnxOperator>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedBackRefusal {
    InvalidProfile,
    BackIdentity,
    AdapterArtifact,
    RuntimeIdentity,
    ExecutionProfile,
    Checkpoint,
    Graph,
    Signature,
    Sidecar,
    UnsupportedIr,
    UnsupportedOpset { domain: String, version: u64 },
    UnsupportedOperator(OnnxOperator),
    MissingOperatorOpset { domain: String },
    Metadata(OnnxMetadataRefusal),
}
/// Constructed only after explicit profile checks. The selected adapter must
/// still compare this receipt with its actual identity before opening a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBackAdmissionReceipt {
    checkpoint: ImportedCheckpointIdentity,
    profile: ExactImportedBackProfile,
    identity: [u8; 32],
}
impl ImportedBackAdmissionReceipt {
    pub fn checkpoint(&self) -> &ImportedCheckpointIdentity {
        &self.checkpoint
    }
    pub fn profile(&self) -> &ExactImportedBackProfile {
        &self.profile
    }
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub fn verify_selected(
        &self,
        implementation: &str,
        artifact: &str,
        runtime: &ModelComputeRuntimeIdentity,
        execution_profile: &str,
    ) -> Result<(), ImportedBackRefusal> {
        self.profile
            .verify_selected(implementation, artifact, runtime, execution_profile)
    }
}
fn name(value: &str, empty: bool) -> bool {
    (empty || !value.is_empty())
        && value.len() <= MAXIMUM_MODEL_IDENTITY_BYTES
        && !value.as_bytes().contains(&0)
}
impl ExactImportedBackProfile {
    fn validate(&self) -> Result<(), ImportedBackRefusal> {
        let r = &self.runtime;
        let names = [
            &self.profile_identity,
            &self.implementation_identity,
            &self.adapter_artifact_identity,
            &self.execution_profile,
            &r.provider_name,
            &r.runtime_name,
            &r.runtime_version,
            &r.runtime_build_identity,
            &r.adapter_artifact_identity,
            &r.device_evidence,
            &r.precision_profile,
        ];
        // The same reviewed metadata profile admits at most eight opset domains.
        if names.iter().any(|s| !name(s, false))
            || self.adapter_artifact_identity != r.adapter_artifact_identity
            || self.minimum_ir == 0
            || self.minimum_ir > self.maximum_ir
            || self.maximum_ir > i64::MAX as u64
            || self.opsets.is_empty()
            || self.opsets.len() > 8
            || self.operators.len() > crate::onnx_metadata::MAXIMUM_ONNX_FIELDS
        {
            return Err(ImportedBackRefusal::InvalidProfile);
        }
        for (index, opset) in self.opsets.iter().enumerate() {
            if !name(&opset.domain, true)
                || opset.domain.len() > 128
                || opset.minimum == 0
                || opset.minimum > opset.maximum
                || opset.maximum > i64::MAX as u64
                || self.opsets[..index]
                    .iter()
                    .any(|p| p.domain == opset.domain)
            {
                return Err(ImportedBackRefusal::InvalidProfile);
            }
        }
        for (index, op) in self.operators.iter().enumerate() {
            if !name(&op.domain, true)
                || op.domain.len() > 128
                || !name(&op.name, false)
                || op.name.len() > 128
                || !self.opsets.iter().any(|p| p.domain == op.domain)
                || self.operators[..index].contains(op)
            {
                return Err(ImportedBackRefusal::InvalidProfile);
            }
        }
        Ok(())
    }
    fn verify_selected(
        &self,
        implementation: &str,
        artifact: &str,
        runtime: &ModelComputeRuntimeIdentity,
        execution_profile: &str,
    ) -> Result<(), ImportedBackRefusal> {
        use ImportedBackRefusal as R;
        if implementation != self.implementation_identity {
            return Err(R::BackIdentity);
        }
        if artifact != self.adapter_artifact_identity {
            return Err(R::AdapterArtifact);
        }
        if runtime != &self.runtime {
            return Err(R::RuntimeIdentity);
        }
        if execution_profile != self.execution_profile {
            return Err(R::ExecutionProfile);
        }
        Ok(())
    }
}

pub fn admit_imported_back(
    resource: &ImportedOnnxResource,
    profile: &ExactImportedBackProfile,
    implementation: &str,
    artifact: &str,
    runtime: &ModelComputeRuntimeIdentity,
    execution_profile: &str,
) -> Result<ImportedBackAdmissionReceipt, ImportedBackRefusal> {
    use ImportedBackRefusal as R;
    profile.validate()?;
    profile.verify_selected(implementation, artifact, runtime, execution_profile)?;
    let checkpoint = ImportedCheckpointIdentity::from_resource(resource);
    if checkpoint.descriptor != profile.checkpoint.descriptor {
        return Err(R::Checkpoint);
    }
    if checkpoint.graph != profile.checkpoint.graph {
        return Err(R::Graph);
    }
    if checkpoint.signature != profile.checkpoint.signature {
        return Err(R::Signature);
    }
    if checkpoint.sidecar != profile.checkpoint.sidecar {
        return Err(R::Sidecar);
    }
    let metadata = resource.metadata();
    if metadata.ir_version < profile.minimum_ir || metadata.ir_version > profile.maximum_ir {
        return Err(R::UnsupportedIr);
    }
    for (domain, version) in &metadata.opsets {
        if !profile
            .opsets
            .iter()
            .any(|p| &p.domain == domain && p.minimum <= *version && *version <= p.maximum)
        {
            return Err(R::UnsupportedOpset {
                domain: domain.clone(),
                version: *version,
            });
        }
    }
    let operators = inspect_onnx_operators(resource.resource().bytes(), metadata.sha256)
        .map_err(R::Metadata)?;
    for operator in operators {
        if !metadata
            .opsets
            .iter()
            .any(|(domain, _)| domain == &operator.domain)
        {
            return Err(R::MissingOperatorOpset {
                domain: operator.domain,
            });
        }
        if !profile.operators.contains(&operator) {
            return Err(R::UnsupportedOperator(operator));
        }
    }
    let identity = profile_digest(profile);
    Ok(ImportedBackAdmissionReceipt {
        checkpoint,
        profile: profile.clone(),
        identity,
    })
}
fn text(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}
fn profile_digest(profile: &ExactImportedBackProfile) -> [u8; 32] {
    let mut out = Vec::new();
    for id in [
        &profile.checkpoint.descriptor,
        &profile.checkpoint.graph,
        &profile.checkpoint.signature,
        &profile.checkpoint.sidecar,
    ] {
        out.extend_from_slice(id);
    }
    let r = &profile.runtime;
    for value in [
        &profile.profile_identity,
        &profile.implementation_identity,
        &profile.adapter_artifact_identity,
        &profile.execution_profile,
        &r.provider_name,
        &r.runtime_name,
        &r.runtime_version,
        &r.runtime_build_identity,
        &r.adapter_artifact_identity,
        &r.device_evidence,
        &r.precision_profile,
    ] {
        text(&mut out, value);
    }
    out.extend_from_slice(&profile.minimum_ir.to_le_bytes());
    out.extend_from_slice(&profile.maximum_ir.to_le_bytes());
    out.extend_from_slice(&(profile.opsets.len() as u32).to_le_bytes());
    for op in &profile.opsets {
        text(&mut out, &op.domain);
        out.extend_from_slice(&op.minimum.to_le_bytes());
        out.extend_from_slice(&op.maximum.to_le_bytes());
    }
    out.extend_from_slice(&(profile.operators.len() as u32).to_le_bytes());
    for op in &profile.operators {
        text(&mut out, &op.domain);
        text(&mut out, &op.name);
    }
    semantic_digest("conduit.ai/imported-back-admission@1", &out)
}
