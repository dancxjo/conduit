//! Bounded Body addressability projected over truthful current Lines.

use alloc::vec::Vec;
use conduit_core::{BootId, HostId, LineAvailability, LineId, LineOffer, OfferGeneration};
use serde::{Deserialize, Serialize};

use crate::{BodyId, BodyMembership, MembershipState, PartId, MAX_BODY_PARTS};

pub const MAX_BODY_LINES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPartAddress {
    pub body_id: BodyId,
    pub part_id: PartId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodySpace {
    pub body_id: BodyId,
    pub membership_revision: crate::BodyMembershipRevision,
    pub addresses: Vec<BodyPartAddress>,
    pub lines: Vec<LineOffer>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodySpaceRefusal {
    WrongBody,
    MalformedMembership,
    AddressCapacityExhausted,
    LineCapacityExhausted,
    DuplicatePart,
    DuplicateLine,
    InvalidAvailability,
    StaleEndpoint,
    MissingEndpoint,
}

impl BodySpace {
    pub fn project(
        body_id: &BodyId,
        membership: &BodyMembership,
        offered_lines: &[LineOffer],
    ) -> Result<Self, BodySpaceRefusal> {
        if body_id != &membership.body_id {
            return Err(BodySpaceRefusal::WrongBody);
        }
        membership
            .validate()
            .map_err(|_| BodySpaceRefusal::MalformedMembership)?;
        if membership.parts.len() > MAX_BODY_PARTS {
            return Err(BodySpaceRefusal::AddressCapacityExhausted);
        }
        if offered_lines.len() > MAX_BODY_LINES {
            return Err(BodySpaceRefusal::LineCapacityExhausted);
        }

        let mut addresses = Vec::with_capacity(MAX_BODY_PARTS);
        for part in &membership.parts {
            if part.state != MembershipState::Admitted {
                continue;
            }
            let Some(current) = &part.current else {
                continue;
            };
            if addresses
                .iter()
                .any(|address: &BodyPartAddress| address.part_id == part.part_id)
            {
                return Err(BodySpaceRefusal::DuplicatePart);
            }
            addresses.push(BodyPartAddress {
                body_id: body_id.clone(),
                part_id: part.part_id.clone(),
                host_id: current.host_id.clone(),
                boot_id: current.boot_id.clone(),
                offer_generation: current.offer_generation,
            });
        }

        let mut lines = Vec::with_capacity(MAX_BODY_LINES);
        for line in offered_lines {
            if line.availability.line_id != line.line_id
                || line.availability.binding_id != line.binding.binding_id
            {
                return Err(BodySpaceRefusal::InvalidAvailability);
            }
            if lines
                .iter()
                .any(|existing: &LineOffer| existing.line_id == line.line_id)
            {
                return Err(BodySpaceRefusal::DuplicateLine);
            }
            validate_endpoint(
                &addresses,
                &line.binding.source.host_id,
                &line.binding.source.boot_id,
            )?;
            validate_endpoint(
                &addresses,
                &line.binding.sink.host_id,
                &line.binding.sink.boot_id,
            )?;
            lines.push(line.clone());
        }
        Ok(Self {
            body_id: body_id.clone(),
            membership_revision: membership.revision,
            addresses,
            lines,
        })
    }

    pub fn address(&self, part_id: &PartId) -> Option<&BodyPartAddress> {
        self.addresses
            .iter()
            .find(|address| &address.part_id == part_id)
    }

    pub fn ready_lines(&self) -> impl Iterator<Item = &LineOffer> {
        self.lines
            .iter()
            .filter(|line| line.availability.availability == LineAvailability::Ready)
    }

    pub fn ready_line_between(&self, source: &PartId, sink: &PartId) -> Option<&LineOffer> {
        let source = self.address(source)?;
        let sink = self.address(sink)?;
        self.ready_lines().find(|line| {
            line.binding.source.host_id == source.host_id
                && line.binding.source.boot_id == source.boot_id
                && line.binding.sink.host_id == sink.host_id
                && line.binding.sink.boot_id == sink.boot_id
        })
    }

    pub fn planner_line_candidates(&self) -> Vec<LineOffer> {
        self.ready_lines().cloned().collect()
    }

    pub fn contains_line(&self, line_id: &LineId) -> bool {
        self.lines.iter().any(|line| &line.line_id == line_id)
    }
}

fn validate_endpoint(
    addresses: &[BodyPartAddress],
    host_id: &HostId,
    boot_id: &BootId,
) -> Result<(), BodySpaceRefusal> {
    let matching_host = addresses
        .iter()
        .find(|address| &address.host_id == host_id)
        .ok_or(BodySpaceRefusal::MissingEndpoint)?;
    if &matching_host.boot_id != boot_id {
        return Err(BodySpaceRefusal::StaleEndpoint);
    }
    Ok(())
}
