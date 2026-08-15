use crate::observations::{observations_admit, validate_resource_observations};
use crate::prelude::*;
use crate::realization::consume_selected_capacity;
use crate::{PlacementChoices, PlannerError};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    BootId, CapabilityId, GearId, HostAdvertisement, HostId, LineAvailability, LineId, LineOffer,
    ResourceObservation, SignId,
};
use conduit_form::CheckedForm;

pub const MAXIMUM_LOCALITY_CANDIDATES: usize = 32;
pub const MAXIMUM_LOCALITY_OBSERVATIONS: usize = 256;

/// Current evidence used only to choose a new realization. It is neither Form
/// meaning nor a stable Host offer.
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
    /// Basis-specific admitted work for the comparison horizon. This is not a
    /// universal Host speed score.
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
    pub work_units_per_kibibyte: u64,
    pub latency_work_units: u64,
    pub pressure_work_units: u64,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalityPlanningBasis {
    pub now_ms: u64,
    pub horizon_seconds: u32,
    pub data_flow: DataFlowObservation,
    pub reductions: Vec<ReductionObservation>,
    pub realization_work: Vec<RealizationWorkObservation>,
    pub transports: Vec<TransportObservation>,
    pub resources: Vec<ResourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalityCandidate {
    pub candidate_id: String,
    pub placements: PlacementChoices,
    /// Exact Line selected for each authored cross-Host Cord.
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
    /// Exact bounded observations behind the explanation. These remain
    /// planning evidence and are not copied into Form meaning or the Plan.
    pub planning_basis: LocalityPlanningBasis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePlacement {
    pub candidate_id: String,
    pub placements: PlacementChoices,
    pub lines: BTreeMap<(GearId, GearId), LineId>,
}

pub fn select_data_locality_candidate(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidates: &[LocalityCandidate],
    basis: &LocalityPlanningBasis,
    line_offers: &[LineOffer],
) -> Result<LocalitySelection, PlannerError> {
    form.validate_identities()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    validate_resource_observations(hosts, &basis.resources)?;
    validate_inputs(form, hosts, candidates, basis)?;
    let mut considered = candidates
        .iter()
        .map(|candidate| evaluate(form, hosts, candidate, basis, line_offers))
        .collect::<Vec<_>>();
    let selected_index = considered
        .iter()
        .enumerate()
        .filter(|(_, evidence)| evidence.disposition == CandidatePlacementDisposition::Admitted)
        .min_by_key(|(_, evidence)| (evidence.total_work_units, evidence.candidate_id.as_str()))
        .map(|(index, _)| index)
        .ok_or_else(|| {
            PlannerError::CurrentResourceObservationUnavailable(
                "no locality candidate has complete current planning evidence".to_string(),
            )
        })?;
    considered[selected_index].disposition = CandidatePlacementDisposition::Selected;
    let candidate = &candidates[selected_index];
    Ok(LocalitySelection {
        checked_form_id: form.checked_form_id.clone(),
        selected: CandidatePlacement {
            candidate_id: candidate.candidate_id.clone(),
            placements: candidate.placements.clone(),
            lines: candidate.lines.clone(),
        },
        considered,
        planning_basis: basis.clone(),
    })
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
                CandidatePlacementDisposition::Rejected(reason) => {
                    explanation.push_str(&format!(
                        "; candidate '{}' was rejected: {reason}",
                        candidate.candidate_id
                    ));
                }
                _ => {
                    let improvement = candidate
                        .total_work_units
                        .saturating_sub(winner.total_work_units);
                    explanation.push_str(&format!(
                        "; candidate '{}' carried {} bytes and would need at least {} fewer work units to win",
                        candidate.candidate_id, candidate.transported_bytes, improvement.saturating_add(1)
                    ));
                }
            }
        }
        explanation
    }
}

fn validate_inputs(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidates: &[LocalityCandidate],
    basis: &LocalityPlanningBasis,
) -> Result<(), PlannerError> {
    if candidates.is_empty() || candidates.len() > MAXIMUM_LOCALITY_CANDIDATES {
        return invalid("candidate count is empty or exceeds the finite bound");
    }
    let observation_count = 1
        + basis.reductions.len()
        + basis.realization_work.len()
        + basis.transports.len()
        + basis.resources.len();
    if observation_count > MAXIMUM_LOCALITY_OBSERVATIONS || basis.horizon_seconds == 0 {
        return invalid("observation count or comparison horizon is invalid");
    }
    let mut ids = BTreeSet::new();
    for candidate in candidates {
        if candidate.candidate_id.is_empty() || !ids.insert(candidate.candidate_id.as_str()) {
            return invalid("candidate identities must be non-empty and unique");
        }
    }
    for provenance in core::iter::once(&basis.data_flow.provenance)
        .chain(basis.reductions.iter().map(|item| &item.provenance))
        .chain(basis.realization_work.iter().map(|item| &item.provenance))
        .chain(basis.transports.iter().map(|item| &item.provenance))
    {
        if provenance.sign_id.as_str().is_empty()
            || provenance.source.is_empty()
            || provenance.observed_at_ms > basis.now_ms
            || basis.now_ms > provenance.valid_until_ms
        {
            return invalid("planning observations must have non-empty, fresh provenance");
        }
    }
    if !form
        .gears
        .iter()
        .any(|gear| gear.gear_id == basis.data_flow.source_gear_id)
        || basis.data_flow.items_per_second == 0
        || basis.data_flow.bytes_per_item == 0
    {
        return invalid("data-flow observation does not name a positive authored source");
    }
    for reduction in &basis.reductions {
        if reduction.output_items_numerator == 0
            || reduction.input_items_denominator == 0
            || reduction.output_bytes_numerator == 0
            || reduction.input_bytes_denominator == 0
            || !form
                .gears
                .iter()
                .any(|gear| gear.gear_id == reduction.gear_id)
        {
            return invalid("reduction observation is zero or names an unknown Gear");
        }
    }
    if hosts.is_empty() {
        return invalid("planning Hosts are empty");
    }
    Ok(())
}

fn evaluate(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidate: &LocalityCandidate,
    basis: &LocalityPlanningBasis,
    line_offers: &[LineOffer],
) -> CandidateCostEvidence {
    let mut evidence = CandidateCostEvidence {
        candidate_id: candidate.candidate_id.clone(),
        disposition: CandidatePlacementDisposition::Admitted,
        compute_work_units: 0,
        transport_work_units: 0,
        transported_bytes: 0,
        total_work_units: 0,
        supporting_sign_ids: vec![basis.data_flow.provenance.sign_id.clone()],
    };
    let reject = |evidence: &mut CandidateCostEvidence, detail: &str| {
        evidence.disposition = CandidatePlacementDisposition::Rejected(detail.to_string());
    };
    let mut remaining_resources = basis.resources.clone();
    let mut rates = BTreeMap::from([(
        basis.data_flow.source_gear_id.clone(),
        (
            basis.data_flow.items_per_second,
            basis
                .data_flow
                .items_per_second
                .saturating_mul(u64::from(basis.data_flow.bytes_per_item)),
        ),
    )]);
    let mut ordered = Vec::new();
    let mut ready = vec![basis.data_flow.source_gear_id.clone()];
    while let Some(gear_id) = ready.pop() {
        if ordered
            .iter()
            .any(|gear: &&conduit_form::CheckedGear| gear.gear_id == gear_id)
        {
            continue;
        }
        if let Some(gear) = form.gears.iter().find(|gear| gear.gear_id == gear_id) {
            ordered.push(gear);
            for connection in form
                .connections
                .iter()
                .filter(|connection| connection.source_gear_id == gear_id)
            {
                ready.push(connection.sink_gear_id.clone());
            }
        }
    }
    let ordered_ids = ordered
        .iter()
        .map(|gear| gear.gear_id.clone())
        .collect::<BTreeSet<_>>();
    ordered.extend(
        form.gears
            .iter()
            .filter(|gear| !ordered_ids.contains(&gear.gear_id)),
    );
    for gear in ordered {
        let Some(choice) = candidate.placements.by_gear.get(&gear.gear_id) else {
            reject(&mut evidence, "missing Gear placement");
            return evidence;
        };
        let Some(host) = hosts.iter().find(|host| host.host_id == choice.host_id) else {
            reject(&mut evidence, "placement Host is absent");
            return evidence;
        };
        let Some(offer) = host.capabilities.iter().find(|offer| {
            offer.capability_id == choice.capability_id
                && offer.checked_face() == gear.checked_face()
        }) else {
            reject(
                &mut evidence,
                "placement capability does not offer the checked Face",
            );
            return evidence;
        };
        if !observations_admit(host, offer, &remaining_resources)
            || consume_selected_capacity(hosts, choice, &mut remaining_resources).is_err()
        {
            reject(
                &mut evidence,
                "current realization resources are insufficient",
            );
            return evidence;
        }
        evidence.supporting_sign_ids.extend(
            basis
                .resources
                .iter()
                .filter(|observation| observation.host_id == host.host_id)
                .map(|observation| observation.sign_id.clone()),
        );
        let Some(work) = basis.realization_work.iter().find(|item| {
            item.gear_id == gear.gear_id
                && item.host_id == host.host_id
                && item.boot_id == host.boot_id
                && item.capability_id == choice.capability_id
        }) else {
            reject(
                &mut evidence,
                "current realization-work observation is missing",
            );
            return evidence;
        };
        evidence.compute_work_units = match evidence.compute_work_units.checked_add(work.work_units)
        {
            Some(value) => value,
            None => {
                reject(&mut evidence, "compute cost overflow");
                return evidence;
            }
        };
        evidence
            .supporting_sign_ids
            .push(work.provenance.sign_id.clone());
        if let Some(reduction) = basis
            .reductions
            .iter()
            .find(|item| item.gear_id == gear.gear_id)
        {
            if let Some((items, bytes)) = rates.get(&gear.gear_id).copied() {
                rates.insert(
                    gear.gear_id.clone(),
                    (
                        items.saturating_mul(u64::from(reduction.output_items_numerator))
                            / u64::from(reduction.input_items_denominator),
                        bytes.saturating_mul(u64::from(reduction.output_bytes_numerator))
                            / u64::from(reduction.input_bytes_denominator),
                    ),
                );
                evidence
                    .supporting_sign_ids
                    .push(reduction.provenance.sign_id.clone());
            }
        }
        for connection in form
            .connections
            .iter()
            .filter(|connection| connection.source_gear_id == gear.gear_id)
        {
            let rate = rates.get(&gear.gear_id).copied().unwrap_or((
                basis.data_flow.items_per_second,
                basis
                    .data_flow
                    .items_per_second
                    .saturating_mul(u64::from(basis.data_flow.bytes_per_item)),
            ));
            rates.insert(connection.sink_gear_id.clone(), rate);
            let source = choice;
            let Some(sink) = candidate.placements.by_gear.get(&connection.sink_gear_id) else {
                reject(&mut evidence, "missing sink placement");
                return evidence;
            };
            if source.host_id != sink.host_id {
                let Some(line_id) = candidate.lines.get(&(
                    connection.source_gear_id.clone(),
                    connection.sink_gear_id.clone(),
                )) else {
                    reject(&mut evidence, "cross-Host Cord has no exact Line");
                    return evidence;
                };
                let Some(line) = basis.transports.iter().find(|line| {
                    &line.line_id == line_id
                        && line.source_host_id == source.host_id
                        && line.sink_host_id == sink.host_id
                }) else {
                    reject(&mut evidence, "current transport observation is missing");
                    return evidence;
                };
                let Some(offer) = line_offers.iter().find(|offer| &offer.line_id == line_id) else {
                    reject(&mut evidence, "exact Line offer is missing");
                    return evidence;
                };
                let source_boot = hosts
                    .iter()
                    .find(|host| host.host_id == source.host_id)
                    .map(|host| &host.boot_id);
                let sink_boot = hosts
                    .iter()
                    .find(|host| host.host_id == sink.host_id)
                    .map(|host| &host.boot_id);
                if offer.availability.availability != LineAvailability::Ready
                    || !offer.validate_sign_identity()
                    || Some(&offer.binding.source.boot_id) != source_boot
                    || Some(&offer.binding.sink.boot_id) != sink_boot
                    || offer.binding.source.host_id != source.host_id
                    || offer.binding.sink.host_id != sink.host_id
                {
                    reject(
                        &mut evidence,
                        "Line offer is unavailable or does not bind the selected Boots",
                    );
                    return evidence;
                }
                let bytes_per_second = rate.1;
                if bytes_per_second > line.throughput_bytes_per_second {
                    reject(&mut evidence, "observed Line throughput is insufficient");
                    return evidence;
                }
                let bytes = bytes_per_second.saturating_mul(u64::from(basis.horizon_seconds));
                let transfer = bytes.saturating_add(1023) / 1024 * line.work_units_per_kibibyte;
                let cost = line
                    .setup_work_units
                    .saturating_add(transfer)
                    .saturating_add(line.latency_work_units)
                    .saturating_add(line.pressure_work_units);
                evidence.transported_bytes = evidence.transported_bytes.saturating_add(bytes);
                evidence.transport_work_units = evidence.transport_work_units.saturating_add(cost);
                evidence
                    .supporting_sign_ids
                    .push(line.provenance.sign_id.clone());
            }
        }
    }
    evidence.total_work_units = evidence
        .compute_work_units
        .saturating_add(evidence.transport_work_units);
    evidence
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidPlanningObservation(detail.to_string()))
}
