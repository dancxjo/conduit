//! Finite pre-Play search for compatible optional Quantity wire placements.
//! Ordinary advertisements without relevant optional codecs keep the greedy path.
use super::*;
use conduit_core::{CapabilityOffer, ConnectionTrack, QuantityWireContract};

const MAXIMUM_GEARS: usize = 256;
const MAXIMUM_CONNECTIONS: usize = 4096;
const MAXIMUM_CANDIDATES: usize = 4096;
const MAXIMUM_ATTEMPTS: usize = 65536;

pub(super) fn needed(
    gears: &PlanningGears<'_>,
    hosts: &[HostAdvertisement],
) -> Result<bool, PlannerError> {
    needed_access(gears, &crate::offer_access::OfferAccess::Ordinary(hosts))
}
pub(super) fn select(
    gears: &PlanningGears<'_>,
    connections: &[CheckedConnection],
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    select_access(
        gears,
        connections,
        &crate::offer_access::OfferAccess::Ordinary(hosts),
    )
}
#[cfg(test)]
fn rows(
    gears: &PlanningGears<'_>,
    hosts: &[HostAdvertisement],
) -> Result<SelectionRows, PlannerError> {
    rows_access(gears, &crate::offer_access::OfferAccess::Ordinary(hosts))
}
pub(super) fn needed_access(
    gears: &PlanningGears<'_>,
    access: &crate::offer_access::OfferAccess<'_>,
) -> Result<bool, PlannerError> {
    let profile = QuantityWireContract::property_kind();
    let mut any = false;
    for host in 0..access.len() {
        for index in 0..access.count(host)? {
            let offer = access.offer(host, index)?;
            if offer
                .realization_properties
                .iter()
                .any(|p| p.profile() == &profile)
            {
                any = true;
                break;
            }
        }
        if any {
            break;
        }
    }
    if !any {
        return Ok(false);
    }
    for gear in gears.iter() {
        let gear = gear?;
        for host in 0..access.len() {
            for index in 0..access.count(host)? {
                let offer = access.offer(host, index)?;
                if offer
                    .realization_properties
                    .iter()
                    .any(|p| p.profile() == &profile)
                    && gear.accepts_realization(&offer)
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

struct SelectionGear {
    gear_id: conduit_core::GearId,
    kind_id: conduit_core::KindId,
}

struct Candidate {
    host_id: conduit_core::HostId,
    capability_id: conduit_core::CapabilityId,
    maximum_instances: u16,
    wire: QuantityWireContract,
}
struct SelectionRows {
    gears: Vec<SelectionGear>,
    candidates: Vec<Vec<Candidate>>,
}
struct Edge<'a> {
    source: usize,
    sink: usize,
    connection: &'a CheckedConnection,
}
fn limit(name: &str) -> PlannerError {
    PlannerError::PlannerLimitExceeded(alloc::format!("quantity/wire-selection-{name}-bound@1"))
}
fn contract(offer: &CapabilityOffer) -> Result<QuantityWireContract, PlannerError> {
    let invalid = || PlannerError::InvalidRealizationProperties(offer.capability_id.clone());
    if !conduit_core::valid_realization_properties(&offer.realization_properties) {
        return Err(invalid());
    }
    let Some(property) = offer
        .realization_properties
        .iter()
        .find(|p| p.profile() == &QuantityWireContract::property_kind())
    else {
        return Ok(QuantityWireContract::default());
    };
    let wire = QuantityWireContract::from_property(property).map_err(|_| invalid())?;
    if wire
        .input_ports()
        .iter()
        .any(|(id, _)| !offer.inputs.iter().any(|p| &p.port_id == id))
        || wire
            .output_ports()
            .iter()
            .any(|(id, _)| !offer.outputs.iter().any(|p| &p.port_id == id))
    {
        return Err(invalid());
    }
    Ok(wire)
}
fn rows_access(
    gears: &PlanningGears<'_>,
    access: &crate::offer_access::OfferAccess<'_>,
) -> Result<SelectionRows, PlannerError> {
    let mut identities = Vec::new();
    let mut rows = Vec::new();
    let mut total = 0;
    for gear in gears.iter() {
        let gear = gear?;
        let mut candidates = Vec::new();
        for host in 0..access.len() {
            for index in 0..access.count(host)? {
                let offer = access.offer(host, index)?;
                if gear.accepts_realization(&offer) {
                    total += 1;
                    if total > MAXIMUM_CANDIDATES {
                        return Err(limit("candidates"));
                    }
                    let wire = contract(&offer);
                    candidates.push((
                        crate::offer_access::CandidateEvidence::capture(
                            host,
                            access.host(host),
                            &gear.kind_id,
                            offer,
                        ),
                        wire,
                    ));
                }
            }
        }
        let mut candidates =
            crate::language_coverage::filter_candidate_evidence(&gear, candidates, |c| {
                c.0.fields()
            })?;
        candidates.sort_by(|a, b| {
            (a.0.host, a.0.different_kind, &a.0.capability_id).cmp(&(
                b.0.host,
                b.0.different_kind,
                &b.0.capability_id,
            ))
        });
        let row = candidates
            .into_iter()
            .map(|(c, wire)| {
                Ok(Candidate {
                    host_id: access.host(c.host).host_id.clone(),
                    capability_id: c.capability_id,
                    maximum_instances: c.maximum_instances,
                    wire: wire?,
                })
            })
            .collect::<Result<Vec<_>, PlannerError>>()?;
        if row.is_empty() {
            return Err(PlannerError::UnknownCapability(
                gear.kind_id.as_str().into(),
            ));
        }
        identities.push(SelectionGear {
            gear_id: gear.gear_id.clone(),
            kind_id: gear.kind_id.clone(),
        });
        rows.push(row);
    }
    Ok(SelectionRows {
        gears: identities,
        candidates: rows,
    })
}

fn edges<'a>(
    gears: &[SelectionGear],
    connections: &'a [CheckedConnection],
) -> Result<Vec<Edge<'a>>, PlannerError> {
    let mut edges = Vec::new();
    for connection in connections {
        if connection.track != ConnectionTrack::Payload {
            continue;
        }
        let position = |id: &conduit_core::GearId| {
            gears
                .iter()
                .position(|g| &g.gear_id == id)
                .ok_or_else(|| PlannerError::MissingPlacement(id.as_str().into()))
        };
        edges.push(Edge {
            source: position(&connection.source_gear_id)?,
            sink: position(&connection.sink_gear_id)?,
            connection,
        });
    }
    Ok(edges)
}
fn mismatch(edge: &Edge<'_>) -> PlannerError {
    let c = edge.connection;
    PlannerError::QuantityWireProfileMismatch(alloc::boxed::Box::new((
        c.source_gear_id.clone(),
        c.source_port_id.clone(),
        c.sink_gear_id.clone(),
        c.sink_port_id.clone(),
    )))
}

pub(super) fn select_access(
    gears: &PlanningGears<'_>,
    connections: &[CheckedConnection],
    access: &crate::offer_access::OfferAccess<'_>,
) -> Result<PlacementChoices, PlannerError> {
    if gears.len() > MAXIMUM_GEARS {
        return Err(limit("gears"));
    }
    if connections.len() > MAXIMUM_CONNECTIONS {
        return Err(limit("connections"));
    }
    let SelectionRows {
        gears: identities,
        candidates: rows,
    } = rows_access(gears, access)?;
    let edges = edges(&identities, connections)?;
    search(&identities, &rows, &edges, MAXIMUM_ATTEMPTS)
}
// Iterative backtracking avoids a preparation stack proportional to Plot size.
// Ranking remains deterministic; only incompatible/instance-exhausted tuples
// are skipped. Search exhaustion refuses instead of installing a weaker codec.
fn search(
    gears: &[SelectionGear],
    rows: &[Vec<Candidate>],
    edges: &[Edge<'_>],
    maximum_attempts: usize,
) -> Result<PlacementChoices, PlannerError> {
    let mut selected: Vec<Option<usize>> = vec![None; gears.len()];
    let mut next = vec![0usize; gears.len()];
    let mut depth = 0;
    let mut attempts = 0;
    let mut last_mismatch = None;
    loop {
        if depth == gears.len() {
            let by_gear = gears
                .iter()
                .enumerate()
                .map(|(index, gear)| {
                    let candidate = &rows[index][selected[index].unwrap()];
                    (
                        gear.gear_id.clone(),
                        PlacementChoice {
                            host_id: candidate.host_id.clone(),
                            capability_id: candidate.capability_id.clone(),
                        },
                    )
                })
                .collect();
            return Ok(PlacementChoices { by_gear });
        }
        if next[depth] == rows[depth].len() {
            next[depth] = 0;
            selected[depth] = None;
            if depth == 0 {
                return Err(last_mismatch.unwrap_or_else(|| {
                    PlannerError::UnknownCapability(gears[0].kind_id.as_str().into())
                }));
            }
            depth -= 1;
            selected[depth] = None;
            continue;
        }
        if attempts == maximum_attempts {
            return Err(limit("attempts"));
        }
        attempts += 1;
        let index = next[depth];
        next[depth] += 1;
        let candidate = &rows[depth][index];
        let instances = selected[..depth]
            .iter()
            .enumerate()
            .filter(|(row, selection)| {
                selection.is_some_and(|choice| {
                    let other = &rows[*row][choice];
                    other.host_id == candidate.host_id
                        && other.capability_id == candidate.capability_id
                })
            })
            .count();
        if instances >= usize::from(candidate.maximum_instances) {
            continue;
        }
        selected[depth] = Some(index);
        let incompatible = edges.iter().find(|edge| {
            let (Some(source), Some(sink)) = (selected[edge.source], selected[edge.sink]) else {
                return false;
            };
            !rows[edge.sink][sink]
                .wire
                .input(&edge.connection.sink_port_id)
                .accepts(
                    rows[edge.source][source]
                        .wire
                        .output(&edge.connection.source_port_id),
                )
        });
        if let Some(edge) = incompatible {
            last_mismatch = Some(mismatch(edge));
            selected[depth] = None;
            continue;
        }
        depth += 1;
    }
}
#[cfg(test)]
mod tests;
