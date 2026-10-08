use crate::checkpoint_store::{digest, BlobRef};
use crate::{AuthoringDescriptor, DirectoryCheckpointStore, Error, OptimizerRecipe};
use conduit_ai::ModelArtifact;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Descriptor {
    pub(crate) schema: u32,
    pub(crate) architecture: String,
    pub(crate) config_identity: [u8; 32],
    pub(crate) signature_identity: [u8; 32],
    pub(crate) checkpoint_schema: u32,
    pub(crate) generation: u64,
    pub(crate) base_artifact_identity: [u8; 32],
    pub(crate) group_identity: [u8; 32],
    pub(crate) inference: BlobRef,
    pub(crate) resume: Option<Resume>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Resume {
    pub(crate) context_identity: [u8; 32],
    pub(crate) recipe: OptimizerRecipe,
    pub(crate) model: BlobRef,
    pub(crate) optimizer: BlobRef,
    pub(crate) initial_generation: u64,
    pub(crate) generation: u64,
    pub(crate) steps: u64,
    pub(crate) work: u64,
    pub(crate) state_identity: String,
    pub(crate) runtime_version: String,
}
pub(crate) fn read_descriptor(
    authoring: &AuthoringDescriptor,
    artifact: &ModelArtifact,
    store: &DirectoryCheckpointStore,
    id: &[u8; 32],
) -> Result<Descriptor, Error> {
    let bytes = store.read_descriptor(id)?;
    let descriptor: Descriptor =
        serde_json::from_slice(&bytes).map_err(|_| Error::CorruptCheckpoint)?;
    if descriptor.schema != 1
        || descriptor.architecture != authoring.architecture
        || descriptor.config_identity != authoring.config_identity
        || descriptor.signature_identity
            != authoring
                .signature
                .semantic_digest()
                .map_err(|_| Error::InvalidDescriptor)?
        || descriptor.checkpoint_schema != authoring.checkpoint_schema
        || descriptor.base_artifact_identity != artifact.content_identity()
        || descriptor.group_identity != group_identity(authoring)?
    {
        return Err(Error::IncompatibleCheckpoint);
    }
    let total = descriptor
        .inference
        .bytes
        .checked_add(
            descriptor
                .resume
                .as_ref()
                .map_or(0, |r| r.model.bytes.saturating_add(r.optimizer.bytes)),
        )
        .and_then(|n| n.checked_add(bytes.len() as u64))
        .ok_or(Error::ResourceBound)?;
    if total > authoring.resources.checkpoint_bytes {
        return Err(Error::ResourceBound);
    }
    Ok(descriptor)
}
pub(crate) fn group_identity(authoring: &AuthoringDescriptor) -> Result<[u8; 32], Error> {
    Ok(digest(
        &serde_json::to_vec(&authoring.groups).map_err(|_| Error::InvalidDescriptor)?,
    ))
}
