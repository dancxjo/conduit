//! Admitted Host-local access to one portable bounded resource reference.

use crate::{
    AuthorityContractId, AuthorityGrantId, BoundedResourceRef, KindId, ResourceClassId,
    ResourceHandleId, ResourceSemanticIdentity, ResourceVersionIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDereferenceRequirement {
    pub content_profile: KindId,
    pub access_class: ResourceClassId,
    pub authority_contract: AuthorityContractId,
    pub maximum_bytes: u64,
    pub maximum_items: Option<u64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceReferenceAvailability {
    Available,
    Lost,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReferenceBinding {
    pub identity: ResourceSemanticIdentity,
    pub version: ResourceVersionIdentity,
    pub content_profile: KindId,
    pub access_class: ResourceClassId,
    pub handle: ResourceHandleId,
    pub authority_contract: AuthorityContractId,
    pub authority_grant: AuthorityGrantId,
    pub maximum_bytes: u64,
    pub maximum_items: Option<u64>,
    pub availability: ResourceReferenceAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedResourceAccess {
    pub handle: ResourceHandleId,
    pub authority_grant: AuthorityGrantId,
    pub maximum_bytes: u64,
    pub maximum_items: Option<u64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceReferenceAccessRefusal {
    InvalidReference,
    EmptyAuthorityContract,
    EmptyAuthorityGrant,
    EmptyHandle,
    SemanticIdentityMismatch,
    StaleVersion,
    ContentProfileMismatch,
    AccessClassMismatch,
    AuthorityContractMismatch,
    ResourceLost,
    ResourceStale,
    ByteBoundExceeded,
    ItemBoundExceeded,
}

impl ResourceDereferenceRequirement {
    pub fn admit(
        &self,
        reference: &BoundedResourceRef,
        binding: &ResourceReferenceBinding,
    ) -> Result<AdmittedResourceAccess, ResourceReferenceAccessRefusal> {
        reference
            .validate()
            .map_err(|_| ResourceReferenceAccessRefusal::InvalidReference)?;
        if self.authority_contract.as_str().is_empty() {
            return Err(ResourceReferenceAccessRefusal::EmptyAuthorityContract);
        }
        if binding.handle.as_str().is_empty() {
            return Err(ResourceReferenceAccessRefusal::EmptyHandle);
        }
        if binding.authority_grant.as_str().is_empty() {
            return Err(ResourceReferenceAccessRefusal::EmptyAuthorityGrant);
        }
        if reference.identity != binding.identity {
            return Err(ResourceReferenceAccessRefusal::SemanticIdentityMismatch);
        }
        if reference.lifetime.version != binding.version {
            return Err(ResourceReferenceAccessRefusal::StaleVersion);
        }
        if reference.content_profile != self.content_profile
            || reference.content_profile != binding.content_profile
        {
            return Err(ResourceReferenceAccessRefusal::ContentProfileMismatch);
        }
        if reference.access_class != self.access_class
            || reference.access_class != binding.access_class
        {
            return Err(ResourceReferenceAccessRefusal::AccessClassMismatch);
        }
        if self.authority_contract != binding.authority_contract {
            return Err(ResourceReferenceAccessRefusal::AuthorityContractMismatch);
        }
        match binding.availability {
            ResourceReferenceAvailability::Available => {}
            ResourceReferenceAvailability::Lost => {
                return Err(ResourceReferenceAccessRefusal::ResourceLost)
            }
            ResourceReferenceAvailability::Stale => {
                return Err(ResourceReferenceAccessRefusal::ResourceStale)
            }
        }
        if reference.extent.bytes > self.maximum_bytes
            || reference.extent.bytes > binding.maximum_bytes
        {
            return Err(ResourceReferenceAccessRefusal::ByteBoundExceeded);
        }
        if let Some(items) = reference.extent.items {
            let admitted = self.maximum_items.is_some_and(|maximum| items <= maximum)
                && binding
                    .maximum_items
                    .is_some_and(|maximum| items <= maximum);
            if !admitted {
                return Err(ResourceReferenceAccessRefusal::ItemBoundExceeded);
            }
        }
        Ok(AdmittedResourceAccess {
            handle: binding.handle.clone(),
            authority_grant: binding.authority_grant.clone(),
            maximum_bytes: reference.extent.bytes,
            maximum_items: reference.extent.items,
        })
    }
}
