//! Named proof Host alternatives built only from exact owned pointwise offers.
//! This is explicit conformance selection, with no fabricated cost observations.
use conduit_ai::{
    fixed_numeric_compact::*, operation_owners::fixed_numeric::compact_pointwise_offer_layout,
};
use conduit_core::*;
use conduit_planner::{FusionRealizationOffer, PlacementChoices};
use conduit_plot::ExpandedAuthoringPlot;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericProfile {
    Unfused,
    CanonicalDag,
    CompactDag,
    LocalExpressions,
    CompactDagLocalExpressions,
}
impl NumericProfile {
    pub fn local_expressions(self) -> bool {
        matches!(
            self,
            Self::LocalExpressions | Self::CompactDagLocalExpressions
        )
    }
    pub fn dags(self) -> bool {
        matches!(
            self,
            Self::CanonicalDag | Self::CompactDag | Self::CompactDagLocalExpressions
        )
    }
    fn compact(self) -> bool {
        matches!(self, Self::CompactDag | Self::CompactDagLocalExpressions)
    }
}
pub fn offers(
    authoring: &ExpandedAuthoringPlot,
    host: &HostAdvertisement,
    placements: &PlacementChoices,
    profile: NumericProfile,
) -> (Vec<FusionRealizationOffer>, Vec<Kind>) {
    let plot = &authoring.expanded;
    let mut members = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for gear in &plot.gears {
        let choice = &placements.by_gear[&gear.gear_id];
        if choice.host_id != host.host_id {
            continue;
        }
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .unwrap();
        let Ok(layout) = compact_pointwise_offer_layout(offer) else {
            continue;
        };
        assert!(gear.accepts_realization(offer));
        members.insert(gear.gear_id.clone(), layout);
        kinds.entry(offer.kind_id.clone()).or_insert_with(|| Kind {
            startup_parameters: offer.startup_parameters.clone(),
            shorthand: offer.shorthand.clone(),
            kind_id: offer.kind_id.clone(),
            kind_contract_revision: offer.kind_contract_revision.clone(),
            inputs: offer.inputs.clone(),
            outputs: offer.outputs.clone(),
            configuration: offer.semantic_contract.configuration.clone(),
            semantic_laws: offer.semantic_contract.laws.clone(),
            limits: offer.limits.clone(),
        });
    }
    let mut outputs: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for cord in &plot.connections {
        outputs
            .entry((&cord.source_gear_id, &cord.source_port_id))
            .or_default()
            .push(cord);
    }
    let mut internal = Vec::new();
    for ((source, port), cords) in outputs {
        let Some(layout) = members.get(source) else {
            continue;
        };
        if authoring
            .output_bindings
            .iter()
            .any(|binding| &binding.gear_id == source && &binding.gear_port_id == port)
        {
            continue;
        }
        if cords.iter().all(|cord| {
            members.get(&cord.sink_gear_id) == Some(layout)
                && cord.track == ConnectionTrack::Payload
                && cord.temporal == PortTemporal::Value
        }) {
            internal.extend(cords);
        }
    }
    let mut adjacency: BTreeMap<GearId, BTreeSet<GearId>> = BTreeMap::new();
    for cord in &internal {
        adjacency
            .entry(cord.source_gear_id.clone())
            .or_default()
            .insert(cord.sink_gear_id.clone());
        adjacency
            .entry(cord.sink_gear_id.clone())
            .or_default()
            .insert(cord.source_gear_id.clone());
    }
    let mut visited = BTreeSet::new();
    let mut result = Vec::new();
    for start in adjacency.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut component = BTreeSet::new();
        let mut stack = vec![start.clone()];
        while let Some(node) = stack.pop() {
            if component.insert(node.clone()) {
                stack.extend(adjacency[&node].iter().cloned());
            }
        }
        visited.extend(component.iter().cloned());
        if !(2..=32).contains(&component.len()) {
            continue;
        }
        let cords: Vec<_> = plot
            .connections
            .iter()
            .filter(|cord| {
                component.contains(&cord.source_gear_id) && component.contains(&cord.sink_gear_id)
            })
            .collect();
        if cords.iter().any(|cord| !internal.contains(cord)) {
            continue;
        }
        let pairs: BTreeSet<_> = cords
            .iter()
            .map(|cord| (cord.source_gear_id.clone(), cord.sink_gear_id.clone()))
            .collect();
        if pairs.len() != cords.len() {
            continue;
        } // the current generic offer identifies each Cord by its pair
        let compact = profile.compact();
        result.push(FusionRealizationOffer {
            fusion_id: format!(
                "fargan-pointwise/{}/{}",
                if compact { "compact" } else { "canonical" },
                start.as_str()
            ),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            execution_profile_id: if compact {
                COMPACT_DAG_PROFILE
            } else {
                CANONICAL_DAG_PROFILE
            }
            .into(),
            implementation_id: if compact {
                COMPACT_DAG_IMPLEMENTATION
            } else {
                CANONICAL_DAG_IMPLEMENTATION
            }
            .into(),
            artifact_id: DAG_ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            gear_ids: component.into_iter().collect(),
            internal_cords: pairs.into_iter().collect(),
            preserves_typed_ports: true,
            preserves_atomic_pressure: true,
            preserves_cancellation: true,
            preserves_required_evidence: true,
        });
    }
    (result, kinds.into_values().collect())
}
