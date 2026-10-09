//! Preparation-time custody of one exact immutable model artifact.
//! This admits bytes and read authority; topology and session policy remain Source-owned.

use alloc::sync::Arc;
use conduit_core::{
    AdmittedResourceAccess, AuthorityContractId, ResourceDereferenceRequirement,
    ResourceReferenceAccessRefusal, ResourceReferenceBinding,
};

use crate::{model_content_digest, ModelArtifact, ModelCompatibilityRefusal, ModelSignature};

pub const MODEL_READ_AUTHORITY: &str = "conduit.model/read-immutable-artifact@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResourceRefusal {
    Compatibility(ModelCompatibilityRefusal),
    Content,
    Authority(ResourceReferenceAccessRefusal),
}

/// Retains the full artifact and signature alongside immutable content and its
/// admitted access receipt. Descriptor identity alone does not replace these.
pub struct AdmittedModelResource {
    artifact: ModelArtifact,
    signature: ModelSignature,
    bytes: Arc<[u8]>,
    access: AdmittedResourceAccess,
    descriptor_identity: [u8; 32],
}

impl AdmittedModelResource {
    pub fn adopt(
        artifact: ModelArtifact,
        signature: ModelSignature,
        bytes: Arc<[u8]>,
        binding: &ResourceReferenceBinding,
    ) -> Result<Self, ModelResourceRefusal> {
        let descriptor_identity = artifact
            .descriptor_digest(&signature)
            .map_err(ModelResourceRefusal::Compatibility)?;
        if artifact.content.extent.bytes != bytes.len() as u64
            || artifact.content_identity() != model_content_digest(&bytes)
        {
            return Err(ModelResourceRefusal::Content);
        }
        let access = ResourceDereferenceRequirement {
            content_profile: artifact.content.content_profile.clone(),
            access_class: artifact.content.access_class.clone(),
            authority_contract: AuthorityContractId::from(MODEL_READ_AUTHORITY),
            maximum_bytes: artifact.content.extent.bytes,
            maximum_items: artifact.content.extent.items,
        }
        .admit(&artifact.content, binding)
        .map_err(ModelResourceRefusal::Authority)?;
        Ok(Self {
            artifact,
            signature,
            bytes,
            access,
            descriptor_identity,
        })
    }

    pub fn artifact(&self) -> &ModelArtifact {
        &self.artifact
    }

    pub fn signature(&self) -> &ModelSignature {
        &self.signature
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Shared custody for separately admitted exact tensor slices. Each slice
    /// still needs its own tensor descriptor and read grant.
    pub fn shared_storage(&self) -> Arc<[u8]> {
        self.bytes.clone()
    }

    pub fn access(&self) -> &AdmittedResourceAccess {
        &self.access
    }

    pub fn descriptor_identity(&self) -> [u8; 32] {
        self.descriptor_identity
    }
}

mod storage;
pub use storage::{ModelResourcePreparationRefusal, ModelResourceStorageReceipt};
