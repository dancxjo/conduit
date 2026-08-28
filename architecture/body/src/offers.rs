//! Finite resource allowances offered to one Body.
//!
//! A `ResourceOffer` remains the Host's unchanged description of a real pool.
//! This module adds only a Body policy fence over that pool. Machine capacity
//! stays adapter context, current availability stays a `ResourceObservation`,
//! a Plan selection stays a `ResourceBinding`, and Play utilization stays the
//! observation's `utilized_units`; none is recast as another resource pool.

use alloc::{string::String, vec::Vec};
use conduit_core::{
    resource_binding_satisfies, BootId, HostAdvertisement, HostId, OfferGeneration,
    ResourceAllowance, ResourceAllowanceSet, ResourceAllowanceSourceId, ResourceBinding,
    ResourceObservation, ResourceOffer, ResourceRequirement,
};
use core::cmp::Ordering;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{BodyId, PartId};

/// Maximum number of Host pools fenced by one Body envelope.
pub const MAX_BODY_RESOURCE_ALLOWANCES: usize = 64;

/// Stable identity of one exact Body resource-policy revision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct BodyResourceEnvelopeId(String);

impl BodyResourceEnvelopeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Body policy over an existing Host pool, not a smaller `ResourceOffer`.
pub type BodyResourceAllowance = ResourceAllowance;

/// Exact Body/Part fence over one current Host advertisement.
///
/// It intentionally has no `WakeId`: the Body/Part relationship survives a
/// Lull. A later preparation slice can snapshot this `envelope_id` for a Wake
/// or Plan without mutating the durable policy revision.
///
/// The caller establishes that `part_id` is the membership relation currently
/// attached to this Host/Boot. This constructor validates resource truth, not
/// membership. The envelope deliberately cannot bypass validation through
/// derived deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyResourceEnvelope {
    envelope_id: BodyResourceEnvelopeId,
    body_id: BodyId,
    part_id: PartId,
    host_id: HostId,
    boot_id: BootId,
    host_offer_generation: OfferGeneration,
    allowances: Vec<BodyResourceAllowance>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyResourceEnvelopeError {
    Empty,
    CapacityExceeded,
    InvalidAllowance,
    DuplicatePool,
    NotHostOffered,
    EnlargesHostOffer,
    StaleObservation,
    ResourceUnavailable,
    InvalidReservation,
    ReservationExceedsAllowance,
    ReservationUnavailable,
}

impl core::fmt::Display for BodyResourceEnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid Body resource envelope: {self:?}")
    }
}

impl BodyResourceEnvelope {
    /// Creates a deterministic Body allowance over unchanged Host offers.
    ///
    /// Entries must be in canonical pool/class order. Requiring canonical
    /// input makes the identity independent of allocator or map behavior.
    pub fn new(
        body_id: BodyId,
        part_id: PartId,
        host: &HostAdvertisement,
        allowances: Vec<BodyResourceAllowance>,
    ) -> Result<Self, BodyResourceEnvelopeError> {
        if allowances.is_empty() {
            return Err(BodyResourceEnvelopeError::Empty);
        }
        if allowances.len() > MAX_BODY_RESOURCE_ALLOWANCES {
            return Err(BodyResourceEnvelopeError::CapacityExceeded);
        }
        for (index, allowance) in allowances.iter().enumerate() {
            if allowance.pool_id.as_str().is_empty()
                || allowance.class_id.as_str().is_empty()
                || allowance.maximum_units == 0
            {
                return Err(BodyResourceEnvelopeError::InvalidAllowance);
            }
            if index > 0 {
                let previous = &allowances[index - 1];
                match (&previous.pool_id, &previous.class_id)
                    .cmp(&(&allowance.pool_id, &allowance.class_id))
                {
                    Ordering::Less => {}
                    Ordering::Equal => return Err(BodyResourceEnvelopeError::DuplicatePool),
                    Ordering::Greater => return Err(BodyResourceEnvelopeError::InvalidAllowance),
                }
            }
            let Some(host_offer) = find_host_offer(host, allowance) else {
                return Err(BodyResourceEnvelopeError::NotHostOffered);
            };
            if allowance.maximum_units > host_offer.capacity_units {
                return Err(BodyResourceEnvelopeError::EnlargesHostOffer);
            }
        }
        let envelope_id = bind_envelope_id(&body_id, &part_id, host, &allowances);
        Ok(Self {
            envelope_id,
            body_id,
            part_id,
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            host_offer_generation: host.offer_generation,
            allowances,
        })
    }

    pub fn envelope_id(&self) -> &BodyResourceEnvelopeId {
        &self.envelope_id
    }
    pub fn body_id(&self) -> &BodyId {
        &self.body_id
    }
    pub fn part_id(&self) -> &PartId {
        &self.part_id
    }
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub fn host_offer_generation(&self) -> OfferGeneration {
        self.host_offer_generation
    }
    pub fn allowances(&self) -> &[BodyResourceAllowance] {
        &self.allowances
    }

    /// Projects Body/Part policy into Body-neutral planner facts.
    pub fn planning_allowances(&self) -> ResourceAllowanceSet {
        ResourceAllowanceSet {
            source_id: ResourceAllowanceSourceId::from(self.envelope_id.as_str()),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: self.host_offer_generation,
            allowances: self.allowances.clone(),
        }
    }

    /// Validates one candidate reservation without keeping a reservation ledger.
    ///
    /// Cumulative prepared-reservation accounting belongs to the downstream
    /// preparation slice. The caller supplies the original semantic
    /// requirement: selected binding facts are never used to reconstruct it.
    pub fn validates_reservation(
        &self,
        requirement: &ResourceRequirement,
        binding: &ResourceBinding,
        host: &HostAdvertisement,
        observation: &ResourceObservation,
    ) -> Result<(), BodyResourceEnvelopeError> {
        if host.host_id != self.host_id
            || host.boot_id != self.boot_id
            || host.offer_generation != self.host_offer_generation
            || observation.host_id != self.host_id
            || observation.boot_id != self.boot_id
            || observation.offer_generation != self.host_offer_generation
        {
            return Err(BodyResourceEnvelopeError::StaleObservation);
        }
        let Some(allowance) = self.allowances.iter().find(|allowance| {
            allowance.pool_id == binding.pool_id && allowance.class_id == binding.class_id
        }) else {
            return Err(BodyResourceEnvelopeError::InvalidReservation);
        };
        let Some(host_offer) = find_host_offer(host, allowance) else {
            return Err(BodyResourceEnvelopeError::InvalidReservation);
        };
        if observation.pool_id != host_offer.pool_id || observation.class_id != host_offer.class_id
        {
            return Err(BodyResourceEnvelopeError::StaleObservation);
        }
        if observation.health != conduit_core::ResourceHealth::Ready {
            return Err(BodyResourceEnvelopeError::ResourceUnavailable);
        }
        if binding.units > allowance.maximum_units {
            return Err(BodyResourceEnvelopeError::ReservationExceedsAllowance);
        }
        if !resource_binding_satisfies(binding, requirement, host_offer) {
            return Err(BodyResourceEnvelopeError::InvalidReservation);
        }
        if binding.units > observation.unreserved_units {
            return Err(BodyResourceEnvelopeError::ReservationUnavailable);
        }
        Ok(())
    }
}

fn find_host_offer<'a>(
    host: &'a HostAdvertisement,
    allowance: &BodyResourceAllowance,
) -> Option<&'a ResourceOffer> {
    host.resources
        .iter()
        .find(|offer| offer.pool_id == allowance.pool_id && offer.class_id == allowance.class_id)
}

fn bind_envelope_id(
    body_id: &BodyId,
    part_id: &PartId,
    host: &HostAdvertisement,
    allowances: &[BodyResourceAllowance],
) -> BodyResourceEnvelopeId {
    let mut digest = Sha256::new();
    for value in [
        "body-resource-envelope@1",
        body_id.as_str(),
        part_id.as_str(),
        host.host_id.as_str(),
        host.boot_id.as_str(),
    ] {
        digest.update((value.len() as u32).to_le_bytes());
        digest.update(value.as_bytes());
    }
    digest.update(host.offer_generation.0.to_le_bytes());
    digest.update((allowances.len() as u32).to_le_bytes());
    for allowance in allowances {
        for value in [allowance.pool_id.as_str(), allowance.class_id.as_str()] {
            digest.update((value.len() as u32).to_le_bytes());
            digest.update(value.as_bytes());
        }
        digest.update(allowance.maximum_units.to_le_bytes());
    }
    let mut output = String::with_capacity(64);
    for byte in <[u8; 32]>::from(digest.finalize()) {
        use core::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    BodyResourceEnvelopeId(output)
}
