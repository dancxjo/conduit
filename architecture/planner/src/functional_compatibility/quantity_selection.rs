//! Finite pre-Play search for compatible optional Quantity wire placements.
//! Ordinary advertisements without relevant optional codecs keep the greedy path.
use super::*;
use conduit_core::{CapabilityOffer, ConnectionTrack, QuantityWireContract};

const MAXIMUM_GEARS: usize = 256;
const MAXIMUM_CONNECTIONS: usize = 4096;
const MAXIMUM_CANDIDATES: usize = 4096;
const MAXIMUM_ATTEMPTS: usize = 65536;

pub(super) fn needed(gears: &[CheckedGear], hosts: &[HostAdvertisement]) -> bool {
    let profile = QuantityWireContract::property_kind();
    hosts.iter().flat_map(|h| &h.capabilities).any(|offer| {
        offer
            .realization_properties
            .iter()
            .any(|p| p.profile() == &profile)
            && gears.iter().any(|gear| gear.accepts_realization(offer))
    })
}
struct Candidate<'a> {
    host: &'a HostAdvertisement,
    offer: &'a CapabilityOffer,
    wire: QuantityWireContract,
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
fn rows<'a>(
    gears: &[CheckedGear],
    hosts: &'a [HostAdvertisement],
) -> Result<Vec<Vec<Candidate<'a>>>, PlannerError> {
    let mut rows = Vec::with_capacity(gears.len());
    let mut count = 0usize;
    for gear in gears {
        let mut candidates = Vec::new();
        for (host_index, host) in hosts.iter().enumerate() {
            for offer in &host.capabilities {
                if gear.accepts_realization(offer) {
                    count += 1;
                    if count > MAXIMUM_CANDIDATES {
                        return Err(limit("candidates"));
                    }
                    candidates.push((host_index, host, offer));
                }
            }
        }
        let mut candidates =
            crate::language_coverage::filter_candidates(gear, candidates, |c| (c.1, c.2))?;
        candidates.sort_by_key(|(host_index, _, offer)| {
            (
                *host_index,
                offer.kind_id != gear.kind_id,
                offer.capability_id.clone(),
            )
        });
        let row = candidates
            .into_iter()
            .map(|(_, host, offer)| {
                Ok(Candidate {
                    host,
                    offer,
                    wire: contract(offer)?,
                })
            })
            .collect::<Result<Vec<_>, PlannerError>>()?;
        if row.is_empty() {
            return Err(PlannerError::UnknownCapability(
                gear.kind_id.as_str().into(),
            ));
        }
        rows.push(row);
    }
    Ok(rows)
}
fn edges<'a>(
    gears: &[CheckedGear],
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

pub(super) fn select(
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    if gears.len() > MAXIMUM_GEARS {
        return Err(limit("gears"));
    }
    if connections.len() > MAXIMUM_CONNECTIONS {
        return Err(limit("connections"));
    }
    let rows = rows(gears, hosts)?;
    let edges = edges(gears, connections)?;
    search(gears, &rows, &edges, MAXIMUM_ATTEMPTS)
}
// Iterative backtracking avoids a preparation stack proportional to Plot size.
// Ranking remains deterministic; only incompatible/instance-exhausted tuples
// are skipped. Search exhaustion refuses instead of installing a weaker codec.
fn search(
    gears: &[CheckedGear],
    rows: &[Vec<Candidate<'_>>],
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
                            host_id: candidate.host.host_id.clone(),
                            capability_id: candidate.offer.capability_id.clone(),
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
                    other.host.host_id == candidate.host.host_id
                        && other.offer.capability_id == candidate.offer.capability_id
                })
            })
            .count();
        if instances >= usize::from(candidate.offer.limits.max_active_instances) {
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
