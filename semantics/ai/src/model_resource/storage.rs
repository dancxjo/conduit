//! Resource adoption preparation and retained requested payload capacities.
//! Native admission of the signature and complete source/model/plan association
//! belong to the caller. This preserves the original adoption guards; it does
//! not replace a Source admission receipt or grant model execution readiness.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct ModelResourceStorageReceipt {
    preparation_requested_bytes_bound: usize,
    retained_payload_bytes_bound: usize,
}
impl ModelResourceStorageReceipt {
    pub fn preparation_requested_bytes_bound(&self) -> usize {
        self.preparation_requested_bytes_bound
    }
    /// Full original descriptor/signature/backing payload, counted once here.
    /// Shared owner dedup, root/Arc headers/bookkeeping/stack are separate.
    pub fn retained_payload_bytes_bound(&self) -> usize {
        self.retained_payload_bytes_bound
    }
}
#[derive(Debug)]
pub enum ModelResourcePreparationRefusal {
    Capacity,
    Signature(crate::ModelSignatureRefusal),
    Adoption(ModelResourceRefusal),
}
impl AdmittedModelResource {
    /// Allocation-free before any original signature/descriptor digest or
    /// authority clone. Existing input construction/admission is upstream.
    pub fn storage_reservation(
        artifact: &ModelArtifact,
        signature: &ModelSignature,
        bytes: &[u8],
        binding: &ResourceReferenceBinding,
    ) -> Result<ModelResourceStorageReceipt, ModelResourcePreparationRefusal> {
        let signature_bytes = signature
            .digest_encoding_length()
            .map_err(ModelResourcePreparationRefusal::Signature)?;
        let descriptor_bytes = artifact
            .descriptor_encoding_length()
            .ok_or(ModelResourcePreparationRefusal::Capacity)?;
        let access_bytes = binding
            .handle
            .as_str()
            .len()
            .checked_add(binding.authority_grant.as_str().len())
            .ok_or(ModelResourcePreparationRefusal::Capacity)?;
        let preparation = signature_bytes
            .checked_add(descriptor_bytes)
            .and_then(|v| v.checked_add(artifact.content.content_profile.as_str().len()))
            .and_then(|v| v.checked_add(artifact.content.access_class.as_str().len()))
            .and_then(|v| v.checked_add(MODEL_READ_AUTHORITY.len()))
            .and_then(|v| v.checked_add(access_bytes))
            .ok_or(ModelResourcePreparationRefusal::Capacity)?;
        let retained = artifact
            .owned_heap_bytes()
            .checked_add(signature.owned_heap_bytes())
            .and_then(|v| v.checked_add(bytes.len()))
            .and_then(|v| v.checked_add(access_bytes))
            .ok_or(ModelResourcePreparationRefusal::Capacity)?;
        Ok(ModelResourceStorageReceipt {
            preparation_requested_bytes_bound: preparation,
            retained_payload_bytes_bound: retained,
        })
    }
    pub fn adopt_with_storage_limits(
        artifact: ModelArtifact,
        signature: ModelSignature,
        bytes: Arc<[u8]>,
        binding: &ResourceReferenceBinding,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_payload_bytes: usize,
    ) -> Result<(Self, ModelResourceStorageReceipt), ModelResourcePreparationRefusal> {
        let receipt = Self::storage_reservation(&artifact, &signature, &bytes, binding)?;
        if receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || receipt.retained_payload_bytes_bound > maximum_retained_payload_bytes
        {
            return Err(ModelResourcePreparationRefusal::Capacity);
        }
        let value = Self::adopt(artifact, signature, bytes, binding)
            .map_err(ModelResourcePreparationRefusal::Adoption)?;
        Ok((value, receipt))
    }
    pub fn owned_payload_bytes(&self) -> usize {
        self.artifact
            .owned_heap_bytes()
            .saturating_add(self.signature.owned_heap_bytes())
            .saturating_add(self.bytes.len())
            .saturating_add(self.access.handle.owned_heap_bytes())
            .saturating_add(self.access.authority_grant.owned_heap_bytes())
    }
    pub fn shares_storage_with(&self, bytes: &Arc<[u8]>) -> bool {
        Arc::ptr_eq(&self.bytes, bytes)
    }
}
