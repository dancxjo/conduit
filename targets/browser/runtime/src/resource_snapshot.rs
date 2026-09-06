//! Exact bounded records at the browser durable-Resource boundary.
//!
//! Preparation consumes planned residence and authority. This codec neither
//! selects a generation nor schedules storage; the Host executor publishes the
//! resulting opaque record and completes the kernel request separately.
use conduit_core::{
    semantic_digest, AuthorityBinding, BoundedResourceRef, PlannedGear, ResourceAccessMode,
    ResourceContentRefusal, ResourceRetention, ResourceSharing,
};

pub const AUTHORITY_CONTRACT: &str = "authority/resource-snapshot@1";
pub const PUBLISH_OPERATION: &str = "conduit.host/resource-snapshot-publish@1";
pub const READ_OPERATION: &str = "conduit.host/resource-snapshot-read@1";
pub const MAXIMUM_SNAPSHOT_BYTES: usize = 4096;
pub const MAXIMUM_RECORD_BYTES: usize = 8 + 2 + 128 + 2 + 512 + 32 + MAXIMUM_SNAPSHOT_BYTES;
const MAGIC: &[u8; 8] = b"CDRSNP01";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotRefusal {
    InvalidBinding,
    ForeignHost,
    StaleBoot,
    UnsupportedLifetime,
    UnsupportedExpiry,
    AuthorityDenied,
    Reference(ResourceContentRefusal),
    WrongAccess,
    ContentExtent,
    CorruptRecord,
}

/// A single pre-admitted record, with all envelope storage allocated before Play.
/// Neither a portable ResourceRef nor a retained record confers access authority.
pub struct PreparedSnapshotRecord {
    authority: AuthorityBinding,
    access: ResourceAccessMode,
    key: String,
    prefix: Vec<u8>,
    extent: usize,
    buffer: [u8; MAXIMUM_RECORD_BYTES],
    buffer_len: usize,
}

impl PreparedSnapshotRecord {
    pub fn prepare(
        placement: &PlannedGear,
        reference: &BoundedResourceRef,
    ) -> Result<Self, SnapshotRefusal> {
        let [binding] = placement.resources.as_slice() else {
            return Err(SnapshotRefusal::InvalidBinding);
        };
        let offer = binding
            .content
            .as_ref()
            .ok_or(SnapshotRefusal::InvalidBinding)?;
        offer.validate().map_err(SnapshotRefusal::Reference)?;
        let contract = &offer.contract;
        contract
            .accepts_reference(reference)
            .map_err(SnapshotRefusal::Reference)?;
        if offer.owner_host != placement.host_id {
            return Err(SnapshotRefusal::ForeignHost);
        }
        if offer.owner_boot != placement.boot_id {
            return Err(SnapshotRefusal::StaleBoot);
        }
        if reference.lifetime.expires_at.is_some() {
            return Err(SnapshotRefusal::UnsupportedExpiry);
        }
        if contract.retention != ResourceRetention::ExternalDurable {
            return Err(SnapshotRefusal::UnsupportedLifetime);
        }
        if binding.units != 1
            || binding.class_id != reference.access_class
            || contract.generation_slots != 1
            || contract.maximum_bytes as usize > MAXIMUM_SNAPSHOT_BYTES
            || contract.sharing != ResourceSharing::SingleWriterPublished
        {
            return Err(SnapshotRefusal::InvalidBinding);
        }
        let operation = match contract.access {
            ResourceAccessMode::WriteCandidatePublish => PUBLISH_OPERATION,
            ResourceAccessMode::ReadPublished => READ_OPERATION,
        };
        let [host_operation] = placement.host_operations.as_slice() else {
            return Err(SnapshotRefusal::InvalidBinding);
        };
        let expected_bounds = if contract.access == ResourceAccessMode::WriteCandidatePublish {
            (
                MAXIMUM_SNAPSHOT_BYTES as u32,
                conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
            )
        } else {
            (
                conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                MAXIMUM_SNAPSHOT_BYTES as u32,
            )
        };
        if (
            host_operation.maximum_input_bytes,
            host_operation.maximum_output_bytes,
        ) != expected_bounds
        {
            return Err(SnapshotRefusal::InvalidBinding);
        }
        if host_operation.contract_id.as_str() != operation
            || host_operation.target_kind.as_ref() != Some(&placement.kind_id)
            || host_operation.maximum_in_flight != 1
        {
            return Err(SnapshotRefusal::InvalidBinding);
        }
        let [authority] = placement.authority.as_slice() else {
            return Err(SnapshotRefusal::AuthorityDenied);
        };
        if authority.grant_id.as_str().is_empty()
            || authority.contract_id.as_str() != AUTHORITY_CONTRACT
            || authority.host_operation_contract_id != host_operation.contract_id
            || authority.subject_kind != placement.kind_id
            || authority.capability_id != placement.capability_id
        {
            return Err(SnapshotRefusal::AuthorityDenied);
        }
        if authority.host_id != placement.host_id {
            return Err(SnapshotRefusal::ForeignHost);
        }
        if authority.boot_id != placement.boot_id {
            return Err(SnapshotRefusal::StaleBoot);
        }
        let encoded = reference
            .encode()
            .map_err(|_| SnapshotRefusal::InvalidBinding)?;
        let owner = placement.host_id.as_str().as_bytes();
        let mut prefix = Vec::with_capacity(8 + 2 + owner.len() + 2 + encoded.len());
        prefix.extend_from_slice(MAGIC);
        prefix.extend_from_slice(&(owner.len() as u16).to_le_bytes());
        prefix.extend_from_slice(owner);
        prefix.extend_from_slice(&(encoded.len() as u16).to_le_bytes());
        prefix.extend_from_slice(&encoded);
        // Boot and grant are deliberately absent: restoration requires a new
        // current-Boot binding, while the exact stored generation stays stable.
        let mut key_identity = Vec::with_capacity(owner.len() + 66);
        key_identity.extend_from_slice(&(owner.len() as u16).to_le_bytes());
        key_identity.extend_from_slice(owner);
        key_identity.extend_from_slice(&reference.identity.digest());
        key_identity.extend_from_slice(&reference.lifetime.version.digest());
        let digest = semantic_digest("conduit.resource/snapshot-key@1", &key_identity);
        let key = digest
            .iter()
            .fold(String::from("resource/"), |mut key, byte| {
                use std::fmt::Write;
                write!(&mut key, "{byte:02x}").expect("String write");
                key
            });
        Ok(Self {
            authority: authority.clone(),
            access: contract.access,
            key,
            prefix,
            extent: reference.extent.bytes as usize,
            buffer: [0; MAXIMUM_RECORD_BYTES],
            buffer_len: 0,
        })
    }

    pub fn candidate_record(&self) -> Option<&[u8]> {
        (self.buffer_len != 0).then_some(&self.buffer[..self.buffer_len])
    }

    pub fn storage_key(&self) -> &str {
        &self.key
    }

    pub fn publication<'a>(
        &'a mut self,
        authority: &AuthorityBinding,
        content: &[u8],
    ) -> Result<&'a [u8], SnapshotRefusal> {
        self.authorize(authority, ResourceAccessMode::WriteCandidatePublish)?;
        if content.len() != self.extent {
            return Err(SnapshotRefusal::ContentExtent);
        }
        let prefix_end = self.prefix.len();
        let content_start = prefix_end + 32;
        self.buffer[..prefix_end].copy_from_slice(&self.prefix);
        self.buffer[prefix_end..content_start].copy_from_slice(&semantic_digest(
            "conduit.resource/snapshot-content@1",
            content,
        ));
        self.buffer[content_start..content_start + content.len()].copy_from_slice(content);
        self.buffer_len = content_start + content.len();
        Ok(&self.buffer[..self.buffer_len])
    }

    pub fn restore<'a>(
        &self,
        authority: &AuthorityBinding,
        record: &'a [u8],
    ) -> Result<&'a [u8], SnapshotRefusal> {
        self.authorize(authority, ResourceAccessMode::ReadPublished)?;
        let content_start = self.prefix.len() + 32;
        if record.len() != content_start + self.extent || !record.starts_with(&self.prefix) {
            return Err(SnapshotRefusal::CorruptRecord);
        }
        let content = &record[content_start..];
        if record[self.prefix.len()..content_start]
            != semantic_digest("conduit.resource/snapshot-content@1", content)
        {
            return Err(SnapshotRefusal::CorruptRecord);
        }
        Ok(content)
    }

    fn authorize(
        &self,
        authority: &AuthorityBinding,
        access: ResourceAccessMode,
    ) -> Result<(), SnapshotRefusal> {
        if authority.host_id != self.authority.host_id {
            return Err(SnapshotRefusal::ForeignHost);
        }
        if authority.boot_id != self.authority.boot_id {
            return Err(SnapshotRefusal::StaleBoot);
        }
        if authority != &self.authority {
            return Err(SnapshotRefusal::AuthorityDenied);
        }
        if self.access != access {
            return Err(SnapshotRefusal::WrongAccess);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "resource_snapshot_tests.rs"]
pub(crate) mod tests;
