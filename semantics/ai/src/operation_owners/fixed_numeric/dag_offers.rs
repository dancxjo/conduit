//! Bounded physical alternatives from exact owned numeric offers, without costs.
use super::*;
use alloc::{collections::BTreeSet, vec};
use conduit_kernel::scheduler::CompactF32Layout;
/// Finite numeric alternatives; orchestration converts these into Planner offers.
pub const MAXIMUM_NUMERIC_DAG_OFFERS: usize = 64;
#[derive(Clone, Debug)]
pub struct NumericDagOffer {
    pub fusion_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub gear_ids: Vec<GearId>,
    pub internal_cords: Vec<(GearId, GearId)>,
}
/// Selected member identity, copied from the orchestration placement decision.
#[derive(Clone, Debug)]
pub struct NumericDagPlacement {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
}
use conduit_plot::ExpandedAuthoringPlot;

pub const COMPACT_MATRIX_DAG_PROFILE: &str = "conduit.numeric/finite-f32-matrix-dag@1";
pub const CANONICAL_MATRIX_DAG_PROFILE: &str = "conduit.numeric/canonical-f32-matrix-dag@1";
pub const COMPACT_MATRIX_DAG_IMPLEMENTATION: &str = "conduit.numeric/compact-matrix-dag@1";
pub const CANONICAL_MATRIX_DAG_IMPLEMENTATION: &str = "conduit.numeric/canonical-matrix-dag@1";
pub const MATRIX_DAG_ARTIFACT: &str = "conduit-kernel/logical-step-f32-matrix-dag@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericDagTransport {
    Canonical,
    Compact,
}
struct Ports {
    inputs: BTreeMap<conduit_core::PortId, CompactF32Layout>,
    output: (conduit_core::PortId, CompactF32Layout),
    canonical: Vec<conduit_core::PortId>,
}
fn ports(offer: &CapabilityOffer) -> Result<Ports, String> {
    if let Ok(layout) = compact_pointwise_offer_layout(offer) {
        return Ok(Ports {
            inputs: offer
                .inputs
                .iter()
                .map(|p| (p.port_id.clone(), layout))
                .collect(),
            output: (offer.outputs[0].port_id.clone(), layout),
            canonical: Vec::new(),
        });
    }
    let layout = compact_linear_offer_layout(offer).or_else(|_| {
        compact_slice_offer_layout(offer).map(|s| CompactLinearOfferLayout {
            numeric_input: s.numeric_input,
            output: s.output,
            canonical_inputs: s.canonical_inputs,
        })
    })?;
    Ok(Ports {
        inputs: BTreeMap::from([(
            offer.inputs[usize::from(layout.numeric_input.0 .0)]
                .port_id
                .clone(),
            layout.numeric_input.1,
        )]),
        output: (
            offer.outputs[usize::from(layout.output.0 .0)]
                .port_id
                .clone(),
            layout.output.1,
        ),
        canonical: layout
            .canonical_inputs
            .iter()
            .map(|p| offer.inputs[usize::from(p.0)].port_id.clone())
            .collect(),
    })
}
fn components(nodes: &BTreeSet<GearId>, edges: &[(GearId, GearId)]) -> Vec<BTreeSet<GearId>> {
    let mut adjacent: BTreeMap<GearId, BTreeSet<GearId>> = BTreeMap::new();
    for (a, b) in edges {
        if nodes.contains(a) && nodes.contains(b) {
            adjacent.entry(a.clone()).or_default().insert(b.clone());
            adjacent.entry(b.clone()).or_default().insert(a.clone());
        }
    }
    let mut visited = BTreeSet::new();
    let mut groups = Vec::new();
    for start in adjacent.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut group = BTreeSet::new();
        let mut stack = vec![start.clone()];
        while let Some(n) = stack.pop() {
            if group.insert(n.clone()) {
                stack.extend(adjacent[&n].iter().cloned());
            }
        }
        visited.extend(group.iter().cloned());
        groups.push(group);
    }
    groups
}
fn topological(nodes: &BTreeSet<GearId>, edges: &[(GearId, GearId)]) -> Option<Vec<GearId>> {
    let mut indegree: BTreeMap<_, usize> = nodes.iter().cloned().map(|n| (n, 0)).collect();
    for (a, b) in edges {
        if nodes.contains(a) && nodes.contains(b) {
            *indegree.get_mut(b)? += 1;
        }
    }
    let mut ready: BTreeSet<_> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    let mut order = Vec::new();
    while let Some(n) = ready.pop_first() {
        for (a, b) in edges {
            if a == &n && nodes.contains(b) {
                let d = indegree.get_mut(b)?;
                *d -= 1;
                if *d == 0 {
                    ready.insert(b.clone());
                }
            }
        }
        order.push(n);
    }
    (order.len() == nodes.len()).then_some(order)
}
/// Advertise explicit current-Host alternatives. Every member retains its exact
/// scalar Back and Source provenance. The ordinary planner must separately
/// admit the offer and all observation/authority boundaries. Resource adoption,
/// prepared owner validation and kernel binding remain required before Play.
/// No measured speedup or automatic optimal-choice claim follows from this API.
pub fn numeric_dag_offers(
    authoring: &ExpandedAuthoringPlot,
    host: &HostAdvertisement,
    placements: &BTreeMap<GearId, NumericDagPlacement>,
    transport: NumericDagTransport,
) -> Result<(Vec<NumericDagOffer>, Vec<Kind>), String> {
    authoring
        .expanded
        .validate_expansion()
        .map_err(|e| format!("numeric DAG Source: {e}"))?;
    let plot = &authoring.expanded;
    let mut members = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for gear in &plot.gears {
        let choice = placements
            .get(&gear.gear_id)
            .ok_or("numeric DAG placement absent")?;
        if choice.host_id != host.host_id {
            continue;
        }
        let mut offered = host
            .capabilities
            .iter()
            .filter(|o| o.capability_id == choice.capability_id);
        let offer = offered.next().ok_or("numeric DAG selected offer absent")?;
        if offered.next().is_some() {
            return Err("numeric DAG selected offer ambiguous".into());
        }
        let Ok(layout) = ports(offer) else {
            continue;
        };
        if !gear.accepts_realization(offer) {
            return Err("numeric DAG selected offer does not realize Gear".into());
        }
        // Canonical-only resource references must be local external inputs for
        // the current kernel binder. Metadata never grants their read authority.
        let local = layout.canonical.iter().all(|port| {
            let mut incoming = plot
                .connections
                .iter()
                .filter(|c| c.sink_gear_id == gear.gear_id && &c.sink_port_id == port);
            let Some(cord) = incoming.next() else {
                return false;
            };
            incoming.next().is_none()
                && placements
                    .get(&cord.source_gear_id)
                    .is_some_and(|p| p.host_id == host.host_id)
        });
        if !local {
            continue;
        }
        members.insert(gear.gear_id.clone(), layout);
        kinds.entry(offer.kind_id.clone()).or_insert_with(|| Kind {
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            startup_parameters: offer.startup_parameters.clone(),
            shorthand: offer.shorthand.clone(),
            configuration: offer.semantic_contract.configuration.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            semantic_laws: offer.semantic_contract.laws.clone(),
            limits: offer.limits.clone(),
        });
    }
    let mut outgoing: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for cord in &plot.connections {
        outgoing
            .entry((&cord.source_gear_id, &cord.source_port_id))
            .or_default()
            .push(cord);
    }
    let mut internal = Vec::new();
    for ((source, port), cords) in outgoing {
        let Some(layout) = members.get(source) else {
            continue;
        };
        if &layout.output.0 != port
            || authoring
                .output_bindings
                .iter()
                .any(|b| &b.gear_id == source && &b.gear_port_id == port)
        {
            continue;
        }
        if cords.iter().all(|c| {
            members
                .get(&c.sink_gear_id)
                .and_then(|p| p.inputs.get(&c.sink_port_id))
                == Some(&layout.output.1)
                && c.track == ConnectionTrack::Payload
                && c.temporal == PortTemporal::Value
        }) {
            internal.extend(cords);
        }
    }
    let edges: Vec<_> = internal
        .iter()
        .map(|c| (c.source_gear_id.clone(), c.sink_gear_id.clone()))
        .collect();
    let all: BTreeSet<_> = members.keys().cloned().collect();
    let mut result = Vec::new();
    for component in components(&all, &edges) {
        let Some(order) = topological(&component, &edges) else {
            continue;
        };
        for chunk in order.chunks(32) {
            let subset: BTreeSet<_> = chunk.iter().cloned().collect();
            // Cutting a component keeps a producer canonical whenever any of
            // its fanout escapes. Recompute connected raw groups after the cut.
            let closed: Vec<_> = internal
                .iter()
                .filter(|c| {
                    subset.contains(&c.source_gear_id)
                        && subset.contains(&c.sink_gear_id)
                        && plot
                            .connections
                            .iter()
                            .filter(|e| {
                                e.source_gear_id == c.source_gear_id
                                    && e.source_port_id == c.source_port_id
                            })
                            .all(|e| subset.contains(&e.sink_gear_id))
                })
                .map(|c| (c.source_gear_id.clone(), c.sink_gear_id.clone()))
                .collect();
            for group in components(&subset, &closed) {
                if group.len() < 2 {
                    continue;
                }
                let cords: Vec<_> = plot
                    .connections
                    .iter()
                    .filter(|c| {
                        group.contains(&c.source_gear_id) && group.contains(&c.sink_gear_id)
                    })
                    .collect();
                if cords.iter().any(|c| !internal.contains(c)) {
                    continue;
                }
                let pairs: BTreeSet<_> = cords
                    .iter()
                    .map(|c| (c.source_gear_id.clone(), c.sink_gear_id.clone()))
                    .collect();
                if pairs.len() != cords.len() {
                    continue;
                } // generic offer identifies Cords by pair
                if result.len() == MAXIMUM_NUMERIC_DAG_OFFERS {
                    return Err("numeric DAG offer bound exceeded".into());
                }
                let compact = transport == NumericDagTransport::Compact;
                result.push(NumericDagOffer {
                    fusion_id: format!(
                        "numeric-matrix-dag/{}/{}/{}",
                        if compact { "compact" } else { "canonical" },
                        host.host_id.as_str(),
                        group.first().ok_or("empty numeric DAG")?.as_str()
                    ),
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    execution_profile_id: if compact {
                        COMPACT_MATRIX_DAG_PROFILE
                    } else {
                        CANONICAL_MATRIX_DAG_PROFILE
                    }
                    .into(),
                    implementation_id: if compact {
                        COMPACT_MATRIX_DAG_IMPLEMENTATION
                    } else {
                        CANONICAL_MATRIX_DAG_IMPLEMENTATION
                    }
                    .into(),
                    artifact_id: MATRIX_DAG_ARTIFACT.into(),
                    gear_ids: group.into_iter().collect(),
                    internal_cords: pairs.into_iter().collect(),
                });
            }
        }
    }
    Ok((result, kinds.into_values().collect()))
}
