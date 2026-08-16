use crate::prelude::*;
use crate::{
    CandidatePlacementDisposition, LocalityCandidate, LocalityPlanningBasis, ObservationProvenance,
};
use conduit_core::{
    verify_plan, ArtifactId, BootId, ExecutionProfileId, GearId, HostId, ImplementationId,
    LineOffer, OfferGeneration, Plan,
};

pub const MAXIMUM_FUSION_CANDIDATES: usize = 32;
pub const MAXIMUM_FUSION_GROUPS: usize = 16;
pub const MAXIMUM_FUSION_OFFERS: usize = 64;
pub const MAXIMUM_FUSION_MEMBERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A composite implementation advertised by one exact Host incarnation.
///
/// This is an additional realization offer, not a replacement for the member
/// capability offers: every preserved Gear retains its own selected
/// implementation in the Plan.
pub struct FusionRealizationOffer {
    pub fusion_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub gear_ids: Vec<GearId>,
    pub internal_cords: Vec<(GearId, GearId)>,
    pub preserves_typed_ports: bool,
    pub preserves_atomic_pressure: bool,
    pub preserves_cancellation: bool,
    pub preserves_required_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionBoundary {
    pub source_gear_id: GearId,
    pub sink_gear_id: GearId,
    pub requires_observation: bool,
    pub requires_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionPlanningObservation {
    pub fusion_id: String,
    pub fused_work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionCandidate {
    pub candidate_id: String,
    pub realization: LocalityCandidate,
    pub fusion_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct FusionPlanningInputs<'a> {
    pub offers: &'a [FusionRealizationOffer],
    pub observations: &'a [FusionPlanningObservation],
    pub boundaries: &'a [FusionBoundary],
    pub line_offers: &'a [LineOffer],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionDecisionGroup {
    pub fusion_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub preserved_gear_ids: Vec<GearId>,
    pub preserved_cords: Vec<(GearId, GearId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionCandidateEvidence {
    pub candidate_id: String,
    pub disposition: CandidatePlacementDisposition,
    pub compute_work_units: u64,
    pub transport_work_units: u64,
    pub transported_bytes: u64,
    pub total_work_units: u64,
    pub fusion_groups: Vec<FusionDecisionGroup>,
    pub supporting_sign_ids: Vec<conduit_core::SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionSelection {
    pub checked_form_id: conduit_core::CheckedFormId,
    pub selected_candidate_id: String,
    pub selected_realization: LocalityCandidate,
    pub selected_fusion_groups: Vec<FusionDecisionGroup>,
    pub considered: Vec<FusionCandidateEvidence>,
    pub locality_basis: LocalityPlanningBasis,
    pub fusion_observations: Vec<FusionPlanningObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedPlan {
    pub plan: Plan,
    pub fusion_groups: Vec<FusionDecisionGroup>,
}

impl OptimizedPlan {
    pub fn verify(&self) -> bool {
        if !verify_plan(&self.plan) {
            return false;
        }
        let placements = self
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .collect::<Vec<_>>();
        let connections = self
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .collect::<Vec<_>>();
        self.fusion_groups.iter().all(|group| {
            let exact_plan_group = self.plan.fragments.iter().find_map(|fragment| {
                fragment.execution_fusions.iter().find(|fusion| {
                    fusion.fusion_id.as_str() == group.fusion_id
                        && fusion.execution_profile_id == group.execution_profile_id
                        && fusion.implementation_id == group.implementation_id
                        && fusion.artifact_id == group.artifact_id
                })
            });
            exact_plan_group.is_some()
                && group.preserved_gear_ids.iter().all(|gear_id| {
                    placements.iter().any(|placement| {
                        placement.gear_id == *gear_id
                            && placement.host_id == group.host_id
                            && placement.boot_id == group.boot_id
                    })
                })
                && group.preserved_cords.iter().all(|(source, sink)| {
                    let source = placements.iter().find(|item| item.gear_id == *source);
                    let sink = placements.iter().find(|item| item.gear_id == *sink);
                    source.zip(sink).is_some_and(|(source, sink)| {
                        connections.iter().any(|connection| {
                            connection.source_placement_id == source.placement_id
                                && connection.sink_placement_id == sink.placement_id
                                && connection.selected_line.is_none()
                        })
                    })
                })
        })
    }
}

impl FusionSelection {
    pub fn explain(&self) -> String {
        let winner = self
            .considered
            .iter()
            .find(|item| item.disposition == CandidatePlacementDisposition::Selected)
            .expect("selection has a winner");
        let mode = if winner.fusion_groups.is_empty() {
            "unfused"
        } else {
            "safely fused"
        };
        let mut text = format!(
            "candidate '{}' won {} with {} total work units: {} compute + {} transport across {} bytes",
            winner.candidate_id, mode, winner.total_work_units, winner.compute_work_units,
            winner.transport_work_units, winner.transported_bytes
        );
        for other in self
            .considered
            .iter()
            .filter(|item| item.candidate_id != winner.candidate_id)
        {
            match &other.disposition {
                CandidatePlacementDisposition::Rejected(reason) => text.push_str(&format!(
                    "; candidate '{}' was rejected: {reason}",
                    other.candidate_id
                )),
                _ => text.push_str(&format!(
                    "; candidate '{}' would need at least {} fewer work units to win",
                    other.candidate_id,
                    other
                        .total_work_units
                        .saturating_sub(winner.total_work_units)
                        .saturating_add(1)
                )),
            }
        }
        text
    }
}
