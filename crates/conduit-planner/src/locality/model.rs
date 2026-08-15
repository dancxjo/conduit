use crate::prelude::*;
use crate::PlacementChoices;
use alloc::collections::BTreeMap;
use conduit_core::{BootId, CapabilityId, GearId, HostId, LineId, ResourceObservation, SignId};

pub const MAXIMUM_LOCALITY_CANDIDATES: usize = 32;
pub const MAXIMUM_LOCALITY_OBSERVATIONS: usize = 256;
pub const MAXIMUM_LOCALITY_LINE_OFFERS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationProvenance {
    pub sign_id: SignId,
    pub source: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowObservation {
    pub source_gear_id: GearId,
    pub items_per_second: u64,
    pub bytes_per_item: u32,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReductionObservation {
    pub gear_id: GearId,
    pub output_items_numerator: u32,
    pub input_items_denominator: u32,
    pub output_bytes_numerator: u32,
    pub input_bytes_denominator: u32,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationWorkObservation {
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportObservation {
    pub line_id: LineId,
    pub source_host_id: HostId,
    pub sink_host_id: HostId,
    pub throughput_bytes_per_second: u64,
    pub setup_work_units: u64,
    pub bandwidth_work_units_per_kibibyte: u64,
    pub serialization_work_units_per_kibibyte: u64,
    pub framing_work_units: u64,
    pub queueing_work_units: u64,
    pub latency_work_units: u64,
    pub jitter_work_units: u64,
    pub pressure_work_units: u64,
    pub cancellation_work_units: u64,
    pub loss_work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCordObservation {
    pub source_gear_id: GearId,
    pub sink_gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalityPlanningBasis {
    pub now_ms: u64,
    pub horizon_seconds: u32,
    /// A realization-policy ceiling. `Some(0)` forbids remote transport; this
    /// remains outside authored Form meaning.
    pub remote_bytes_per_second_ceiling: Option<u64>,
    pub data_flow: DataFlowObservation,
    pub reductions: Vec<ReductionObservation>,
    pub realization_work: Vec<RealizationWorkObservation>,
    pub transports: Vec<TransportObservation>,
    pub local_cords: Vec<LocalCordObservation>,
    pub resources: Vec<ResourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalityCandidate {
    pub candidate_id: String,
    pub placements: PlacementChoices,
    pub lines: BTreeMap<(GearId, GearId), LineId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidatePlacementDisposition {
    Admitted,
    Rejected(String),
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCostEvidence {
    pub candidate_id: String,
    pub disposition: CandidatePlacementDisposition,
    pub compute_work_units: u64,
    pub transport_work_units: u64,
    pub transported_bytes: u64,
    pub total_work_units: u64,
    pub supporting_sign_ids: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalitySelection {
    pub checked_form_id: conduit_core::CheckedFormId,
    pub selected: CandidatePlacement,
    pub considered: Vec<CandidateCostEvidence>,
    pub planning_basis: LocalityPlanningBasis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePlacement {
    pub candidate_id: String,
    pub placements: PlacementChoices,
    pub lines: BTreeMap<(GearId, GearId), LineId>,
}

impl LocalitySelection {
    pub fn explain(&self) -> String {
        let winner = self
            .considered
            .iter()
            .find(|item| item.disposition == CandidatePlacementDisposition::Selected)
            .expect("a locality selection always has one winner");
        let mut explanation = format!(
            "candidate '{}' won with {} total work units: {} compute + {} transport, carrying {} bytes",
            winner.candidate_id, winner.total_work_units, winner.compute_work_units,
            winner.transport_work_units, winner.transported_bytes
        );
        for candidate in self
            .considered
            .iter()
            .filter(|item| item.candidate_id != winner.candidate_id)
        {
            match &candidate.disposition {
                CandidatePlacementDisposition::Rejected(reason) => explanation.push_str(&format!(
                    "; candidate '{}' was rejected: {reason}",
                    candidate.candidate_id
                )),
                _ => explanation.push_str(&format!(
                    "; candidate '{}' carried {} bytes and would need at least {} fewer work units to win",
                    candidate.candidate_id, candidate.transported_bytes,
                    candidate.total_work_units.saturating_sub(winner.total_work_units).saturating_add(1)
                )),
            }
        }
        explanation
    }
}
