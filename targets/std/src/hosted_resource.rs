//! A bounded local Resource provider over the kernel's immutable value storage.
//! Platform dispatch must supply its exact admitted authority grant on every call.
use conduit_core::{
    AuthorityGrantId, BoundedResourceRef, ResourceAccessMode, ResourceBinding,
    ResourceContentOffer, ResourceDereferenceRequirement, ResourceReferenceBinding,
    ResourceRetention, ResourceSharing,
};
use conduit_kernel::{HostedValueStore, StorageError, ValueRef, ValueStorage};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourcePublicationRefusal {
    InvalidBinding,
    UnsupportedLifetime,
    AuthorityDenied,
    ReferenceRefused,
    CandidateOccupied,
    NotCandidate,
    NotPublished,
    PublishedImmutable,
    LeaseExhausted,
    StaleLease,
    ReadersPresent,
    Lost,
    Storage(StorageError),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResourceReadLease {
    scope: [u8; 32],
    identity: [u8; 32],
    version: [u8; 32],
    slot: usize,
    issuance: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Publication {
    Empty,
    Candidate(ValueRef),
    Published(ValueRef),
    Lost,
}

/// One exact admitted generation. Another generation requires its own sealed
/// binding and finite provider storage; an old Plan is never rewritten in place.
pub struct HostedResourceGeneration {
    offer: ResourceContentOffer,
    reference: BoundedResourceRef,
    access_requirement: ResourceDereferenceRequirement,
    access_binding: ResourceReferenceBinding,
    writer: Option<AuthorityGrantId>,
    storage: HostedValueStore,
    publication: Publication,
    leases: Vec<Option<u64>>,
    next_lease: u64,
    scope: [u8; 32],
    additional_readers: Vec<ResourceReferenceBinding>,
}

impl HostedResourceGeneration {
    /// Pre-Play construction is the only allocation boundary. The Host supplies
    /// already-admitted bindings; possessing a portable ResourceRef is insufficient.
    pub fn new(
        binding: &ResourceBinding,
        reference: BoundedResourceRef,
        access_requirement: ResourceDereferenceRequirement,
        access_binding: ResourceReferenceBinding,
        writer: Option<AuthorityGrantId>,
    ) -> Result<Self, ResourcePublicationRefusal> {
        let offer = binding
            .content
            .as_ref()
            .ok_or(ResourcePublicationRefusal::InvalidBinding)?;
        offer
            .validate()
            .map_err(|_| ResourcePublicationRefusal::InvalidBinding)?;
        offer
            .contract
            .accepts_reference(&reference)
            .map_err(|_| ResourcePublicationRefusal::ReferenceRefused)?;
        access_requirement
            .admit(&reference, &access_binding)
            .map_err(|_| ResourcePublicationRefusal::ReferenceRefused)?;
        if offer.contract.retention != ResourceRetention::Play {
            return Err(ResourcePublicationRefusal::UnsupportedLifetime);
        }
        if offer.contract.generation_slots != 1
            || binding.units == 0
            || reference.access_class != binding.class_id
            || writer.as_ref().is_some_and(|id| id.as_str().is_empty())
            || (offer.contract.access == ResourceAccessMode::WriteCandidatePublish)
                != writer.is_some()
        {
            return Err(ResourcePublicationRefusal::InvalidBinding);
        }
        let storage = HostedValueStore::new(
            1,
            offer.contract.maximum_bytes,
            offer.contract.maximum_bytes,
        )
        .map_err(ResourcePublicationRefusal::Storage)?;
        let scope = conduit_core::semantic_digest(
            "resource-lease-scope@1",
            format!(
                "{:?}",
                (&offer.owner_host, &offer.owner_boot, &access_binding.handle)
            )
            .as_bytes(),
        );
        Ok(Self {
            scope,
            additional_readers: Vec::new(),
            leases: vec![None; usize::from(offer.contract.reader_leases)],
            offer: offer.clone(),
            reference,
            access_requirement,
            access_binding,
            writer,
            storage,
            publication: Publication::Empty,
            next_lease: 1,
        })
    }

    /// Seed an immutable-read-many generation before Play. This consumes the
    /// provider, so callers cannot seed or replace live published bytes.
    pub fn initialize(mut self, bytes: &[u8]) -> Result<Self, ResourcePublicationRefusal> {
        if self.offer.contract.sharing != ResourceSharing::ImmutableReadMany
            || self.publication != Publication::Empty
        {
            return Err(ResourcePublicationRefusal::InvalidBinding);
        }
        self.check_extent(bytes)?;
        let value = self
            .storage
            .store(bytes)
            .map_err(ResourcePublicationRefusal::Storage)?;
        self.publication = Publication::Published(value);
        Ok(self)
    }

    pub fn write_candidate(
        &mut self,
        grant: &AuthorityGrantId,
        bytes: &[u8],
    ) -> Result<(), ResourcePublicationRefusal> {
        self.check_writer(grant)?;
        match self.publication {
            Publication::Empty => {}
            Publication::Candidate(_) => return Err(ResourcePublicationRefusal::CandidateOccupied),
            Publication::Published(_) => {
                return Err(ResourcePublicationRefusal::PublishedImmutable)
            }
            Publication::Lost => return Err(ResourcePublicationRefusal::Lost),
        }
        self.check_extent(bytes)?;
        let value = self
            .storage
            .store(bytes)
            .map_err(ResourcePublicationRefusal::Storage)?;
        self.publication = Publication::Candidate(value);
        Ok(())
    }

    pub fn publish(
        &mut self,
        grant: &AuthorityGrantId,
    ) -> Result<&BoundedResourceRef, ResourcePublicationRefusal> {
        self.check_writer(grant)?;
        let Publication::Candidate(value) = self.publication else {
            return Err(if self.publication == Publication::Lost {
                ResourcePublicationRefusal::Lost
            } else {
                ResourcePublicationRefusal::NotCandidate
            });
        };
        self.publication = Publication::Published(value);
        Ok(&self.reference)
    }

    /// Pre-Play installation of additional exact authorized readers.
    pub fn with_readers(
        mut self,
        readers: Vec<ResourceReferenceBinding>,
    ) -> Result<Self, ResourcePublicationRefusal> {
        if self.publication != Publication::Empty || readers.len() >= self.leases.len() {
            return Err(ResourcePublicationRefusal::InvalidBinding);
        }
        for (index, binding) in readers.iter().enumerate() {
            self.access_requirement
                .admit(&self.reference, binding)
                .map_err(|_| ResourcePublicationRefusal::ReferenceRefused)?;
            if binding.authority_grant == self.access_binding.authority_grant
                || readers[..index]
                    .iter()
                    .any(|other| other.authority_grant == binding.authority_grant)
            {
                return Err(ResourcePublicationRefusal::InvalidBinding);
            }
        }
        self.additional_readers = readers;
        Ok(self)
    }

    pub fn acquire(
        &mut self,
        grant: &AuthorityGrantId,
    ) -> Result<ResourceReadLease, ResourcePublicationRefusal> {
        // All references and grants were validated before Play and are private,
        // immutable provider state. Do not allocate another admitted-access value.
        core::iter::once(&self.access_binding)
            .chain(&self.additional_readers)
            .find(|binding| &binding.authority_grant == grant)
            .ok_or(ResourcePublicationRefusal::AuthorityDenied)?;
        let value = self.published()?;
        let slot = self
            .leases
            .iter()
            .position(Option::is_none)
            .ok_or(ResourcePublicationRefusal::LeaseExhausted)?;
        let next = self
            .next_lease
            .checked_add(1)
            .ok_or(ResourcePublicationRefusal::LeaseExhausted)?;
        self.storage
            .retain(value)
            .map_err(ResourcePublicationRefusal::Storage)?;
        let issuance = self.next_lease;
        self.next_lease = next;
        self.leases[slot] = Some(issuance);
        Ok(ResourceReadLease {
            scope: self.scope,
            identity: self.offer.contract.identity,
            version: self.offer.contract.version,
            slot,
            issuance,
        })
    }

    pub fn read(&self, lease: ResourceReadLease) -> Result<&[u8], ResourcePublicationRefusal> {
        self.check_lease(lease)?;
        self.storage
            .get(self.published()?)
            .map_err(ResourcePublicationRefusal::Storage)
    }

    pub fn release(&mut self, lease: ResourceReadLease) -> Result<(), ResourcePublicationRefusal> {
        self.check_lease(lease)?;
        self.storage
            .release(self.published()?)
            .map_err(ResourcePublicationRefusal::Storage)?;
        self.leases[lease.slot] = None;
        Ok(())
    }

    /// Cancellation/terminal disposal is bounded. Published storage cannot be
    /// reclaimed until every issued reader relinquishes its lease.
    pub fn retire(&mut self, grant: &AuthorityGrantId) -> Result<(), ResourcePublicationRefusal> {
        self.check_writer(grant)?;
        if self.leases.iter().any(Option::is_some) {
            return Err(ResourcePublicationRefusal::ReadersPresent);
        }
        self.storage.clear();
        self.publication = Publication::Lost;
        Ok(())
    }

    pub fn payload_residencies(&self) -> u16 {
        self.storage.used_items()
    }
    pub fn resident_bytes(&self) -> u32 {
        self.storage.used_bytes()
    }
    pub fn reference(&self) -> &BoundedResourceRef {
        &self.reference
    }

    fn check_extent(&self, bytes: &[u8]) -> Result<(), ResourcePublicationRefusal> {
        if bytes.len() as u64 != self.reference.extent.bytes {
            return Err(ResourcePublicationRefusal::ReferenceRefused);
        }
        Ok(())
    }
    fn check_writer(&self, grant: &AuthorityGrantId) -> Result<(), ResourcePublicationRefusal> {
        if self.writer.as_ref() != Some(grant) {
            return Err(ResourcePublicationRefusal::AuthorityDenied);
        }
        Ok(())
    }
    fn published(&self) -> Result<ValueRef, ResourcePublicationRefusal> {
        match self.publication {
            Publication::Published(value) => Ok(value),
            Publication::Lost => Err(ResourcePublicationRefusal::Lost),
            _ => Err(ResourcePublicationRefusal::NotPublished),
        }
    }
    fn check_lease(&self, lease: ResourceReadLease) -> Result<(), ResourcePublicationRefusal> {
        if lease.scope != self.scope
            || lease.identity != self.offer.contract.identity
            || lease.version != self.offer.contract.version
            || self.leases.get(lease.slot) != Some(&Some(lease.issuance))
        {
            return Err(ResourcePublicationRefusal::StaleLease);
        }
        Ok(())
    }
}
