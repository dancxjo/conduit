//! Bounded current Host/Line truth for one front-door Body session.

use conduit_body::{BodyMembership, CandidateInventory, CandidateState};
use conduit_core::{HostAdvertisement, LineAvailability, LineId, LineOffer, SignId};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport, LineReport,
    ObservatorySnapshot, OfferFreshness, OperationalState,
};

pub const MAX_FRONT_DOOR_LINES: usize = 16;

#[derive(Clone, Default)]
pub(super) struct FrontDoorTopology {
    lines: Vec<ObservedFrontDoorLine>,
}

#[derive(Clone)]
struct ObservedFrontDoorLine {
    report: LineReport,
    source: HostAdvertisement,
    sink: HostAdvertisement,
}

impl FrontDoorTopology {
    pub(super) fn observe_line(
        &mut self,
        offer: LineOffer,
        local: &HostAdvertisement,
        candidates: &CandidateInventory,
    ) -> Result<(), String> {
        if self.lines.len() == MAX_FRONT_DOOR_LINES {
            return Err("front-door Line capacity exhausted".into());
        }
        if self
            .lines
            .iter()
            .any(|line| line.report.offer.line_id == offer.line_id)
        {
            return Err(format!(
                "front-door Line {} is already observed",
                offer.line_id.as_str()
            ));
        }
        let source = endpoint_advertisement(&offer.binding.source, local, candidates)
            .ok_or_else(|| unknown_endpoint(&offer, &offer.binding.source))?;
        let sink = endpoint_advertisement(&offer.binding.sink, local, candidates)
            .ok_or_else(|| unknown_endpoint(&offer, &offer.binding.sink))?;
        let state = operational_line_state(offer.availability.availability);
        self.lines.push(ObservedFrontDoorLine {
            report: LineReport { offer, state },
            source,
            sink,
        });
        Ok(())
    }

    pub(super) fn observe_availability(
        &mut self,
        line_id: &LineId,
        availability: LineAvailability,
        sign_id: SignId,
    ) -> Result<(), String> {
        let line = self
            .lines
            .iter_mut()
            .find(|line| &line.report.offer.line_id == line_id)
            .ok_or_else(|| format!("unknown front-door Line {}", line_id.as_str()))?;
        if line.report.offer.availability.availability == availability {
            return Err(format!(
                "front-door Line {} is already {availability:?}",
                line_id.as_str()
            ));
        }
        line.report.offer.availability.availability = availability;
        line.report.offer.availability.sign_id = sign_id;
        line.report.state = operational_line_state(availability);
        Ok(())
    }

    pub(super) fn snapshot(
        &self,
        mut snapshot: ObservatorySnapshot,
        candidates: &CandidateInventory,
        membership: &BodyMembership,
    ) -> ObservatorySnapshot {
        for candidate in &candidates.candidates {
            let advertisement = &candidate.observation.advertisement;
            if snapshot.hosts.iter().any(|host| {
                host.advertisement.host_id == advertisement.host_id
                    && host.advertisement.boot_id == advertisement.boot_id
            }) {
                continue;
            }
            let present = membership.parts.iter().any(|part| {
                part.current.as_ref().is_some_and(|current| {
                    current.host_id == advertisement.host_id
                        && current.boot_id == advertisement.boot_id
                })
            });
            let state = if present || candidate.state != CandidateState::Admitted {
                OperationalState::Available
            } else {
                OperationalState::Unreachable
            };
            snapshot.hosts.push(host_report(advertisement, state));
        }
        for advertisement in self
            .lines
            .iter()
            .flat_map(|line| [&line.source, &line.sink])
        {
            if !snapshot.hosts.iter().any(|host| {
                host.advertisement.host_id == advertisement.host_id
                    && host.advertisement.boot_id == advertisement.boot_id
            }) {
                snapshot
                    .hosts
                    .push(host_report(advertisement, OperationalState::Unreachable));
            }
        }
        snapshot.lines = self.lines.iter().map(|line| line.report.clone()).collect();
        snapshot
    }
}

fn endpoint_advertisement(
    endpoint: &conduit_core::LinkEndpoint,
    local: &HostAdvertisement,
    candidates: &CandidateInventory,
) -> Option<HostAdvertisement> {
    if endpoint.host_id == local.host_id && endpoint.boot_id == local.boot_id {
        return Some(local.clone());
    }
    candidates
        .candidates
        .iter()
        .find(|candidate| {
            candidate.observation.advertisement.host_id == endpoint.host_id
                && candidate.observation.advertisement.boot_id == endpoint.boot_id
        })
        .map(|candidate| candidate.observation.advertisement.clone())
}

fn unknown_endpoint(offer: &LineOffer, endpoint: &conduit_core::LinkEndpoint) -> String {
    format!(
        "front-door Line {} names unobserved Host/Boot endpoint {}/{}",
        offer.line_id.as_str(),
        endpoint.host_id.as_str(),
        endpoint.boot_id.as_str()
    )
}

fn host_report(advertisement: &HostAdvertisement, state: OperationalState) -> HostReport {
    HostReport {
        advertisement: advertisement.clone(),
        state,
        capabilities: advertisement
            .capabilities
            .iter()
            .map(|offer| CapabilityStatusReport {
                capability_id: offer.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: if state == OperationalState::Available {
                    CapabilityAvailability::Available
                } else {
                    CapabilityAvailability::Unavailable
                },
            })
            .collect(),
    }
}

fn operational_line_state(availability: LineAvailability) -> OperationalState {
    match availability {
        LineAvailability::Ready => OperationalState::Available,
        LineAvailability::Unavailable => OperationalState::Unreachable,
    }
}
