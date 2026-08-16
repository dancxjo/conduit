use crate::prelude::*;
use crate::{ObservationProvenance, PlannerError};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    BootId, CapabilityId, GearId, HostId, ImplementationId, OfferGeneration, ResourcePoolId, SignId,
};

pub const MAXIMUM_ACCELERATOR_CANDIDATES: usize = 32;
pub const MAXIMUM_ACCELERATOR_OFFERS: usize = 32;
pub const MAXIMUM_ACCELERATOR_DEMANDS: usize = 64;
pub const MAXIMUM_ACCELERATOR_DIMENSIONS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AcceleratorDimension(pub String);

impl From<&str> for AcceleratorDimension {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorOffer {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub pool_id: ResourcePoolId,
    /// Provider-specific finite dimensions. The portable planner compares
    /// exact names but does not pretend that every accelerator has one unit.
    pub capacities: BTreeMap<AcceleratorDimension, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub pool_id: ResourcePoolId,
    pub resource_generation: u64,
    pub runtime_usable: bool,
    pub unreserved: BTreeMap<AcceleratorDimension, u64>,
    pub resident_artifacts: Vec<String>,
    pub provenance: ObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMechanism {
    Cpu,
    Accelerator {
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        capability_id: CapabilityId,
        implementation_id: ImplementationId,
        pool_id: ResourcePoolId,
        resource_generation: u64,
        residency_artifact: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorDemand {
    pub gear_id: GearId,
    pub mechanism: ExecutionMechanism,
    pub dimensions: BTreeMap<AcceleratorDimension, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorCandidate {
    pub candidate_id: String,
    pub demands: Vec<AcceleratorDemand>,
    pub compute_work_units: u64,
    pub transfer_work_units: u64,
    pub setup_work_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorPlanningBasis {
    pub now_ms: u64,
    pub residency_credit_work_units: u64,
    pub offers: Vec<AcceleratorOffer>,
    pub observations: Vec<AcceleratorObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorReservation {
    pub gear_id: GearId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub pool_id: ResourcePoolId,
    pub resource_generation: u64,
    pub dimensions: BTreeMap<AcceleratorDimension, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceleratorCandidateDisposition {
    Admitted,
    Rejected(String),
    Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorCandidateEvidence {
    pub candidate_id: String,
    pub disposition: AcceleratorCandidateDisposition,
    pub compute_work_units: u64,
    pub transfer_work_units: u64,
    pub setup_work_units: u64,
    pub residency_credit_work_units: u64,
    pub total_work_units: u64,
    pub reservations: Vec<AcceleratorReservation>,
    pub supporting_sign_ids: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceleratorSelection {
    pub selected_candidate_id: String,
    pub considered: Vec<AcceleratorCandidateEvidence>,
    pub planning_basis: AcceleratorPlanningBasis,
}

impl AcceleratorSelection {
    pub fn explain(&self) -> String {
        let selected = self
            .considered
            .iter()
            .find(|item| item.disposition == AcceleratorCandidateDisposition::Selected)
            .expect("accelerator selection has one winner");
        format!(
            "candidate '{}' won with {} work units ({} compute + {} transfer + {} setup - {} current-residency credit) and {} exact accelerator reservations",
            selected.candidate_id, selected.total_work_units, selected.compute_work_units,
            selected.transfer_work_units, selected.setup_work_units,
            selected.residency_credit_work_units, selected.reservations.len()
        )
    }
}

pub fn select_accelerator_candidate(
    candidates: &[AcceleratorCandidate],
    basis: &AcceleratorPlanningBasis,
) -> Result<AcceleratorSelection, PlannerError> {
    validate_inputs(candidates, basis)?;
    let mut considered = candidates
        .iter()
        .map(|candidate| evaluate(candidate, basis))
        .collect::<Vec<_>>();
    let selected = considered
        .iter()
        .enumerate()
        .filter(|(_, item)| item.disposition == AcceleratorCandidateDisposition::Admitted)
        .min_by_key(|(_, item)| (item.total_work_units, item.candidate_id.as_str()))
        .map(|(index, _)| index)
        .ok_or_else(|| {
            PlannerError::CurrentResourceObservationUnavailable(
                "no CPU or accelerator candidate has complete current admission truth".to_string(),
            )
        })?;
    considered[selected].disposition = AcceleratorCandidateDisposition::Selected;
    Ok(AcceleratorSelection {
        selected_candidate_id: considered[selected].candidate_id.clone(),
        considered,
        planning_basis: basis.clone(),
    })
}

fn validate_inputs(
    candidates: &[AcceleratorCandidate],
    basis: &AcceleratorPlanningBasis,
) -> Result<(), PlannerError> {
    if candidates.is_empty()
        || candidates.len() > MAXIMUM_ACCELERATOR_CANDIDATES
        || basis.offers.len() > MAXIMUM_ACCELERATOR_OFFERS
        || basis.observations.len() > MAXIMUM_ACCELERATOR_OFFERS
    {
        return invalid("accelerator candidate or offer count exceeds its finite bound");
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        if candidate.candidate_id.is_empty()
            || !candidate_ids.insert(candidate.candidate_id.as_str())
            || candidate.demands.len() > MAXIMUM_ACCELERATOR_DEMANDS
        {
            return invalid("accelerator candidate identity or demand count is invalid");
        }
    }
    let mut offer_keys = BTreeSet::new();
    for offer in &basis.offers {
        if offer.capacities.is_empty()
            || offer.capacities.len() > MAXIMUM_ACCELERATOR_DIMENSIONS
            || offer
                .capacities
                .iter()
                .any(|(dimension, value)| dimension.0.is_empty() || *value == 0)
            || !offer_keys.insert((&offer.host_id, &offer.boot_id, &offer.pool_id))
        {
            return invalid("accelerator offer dimensions or exact identity are invalid");
        }
    }
    let mut signs = BTreeSet::new();
    for observation in &basis.observations {
        let provenance = &observation.provenance;
        if observation.resource_generation == 0
            || observation.unreserved.len() > MAXIMUM_ACCELERATOR_DIMENSIONS
            || observation
                .unreserved
                .keys()
                .any(|dimension| dimension.0.is_empty())
            || provenance.sign_id.as_str().is_empty()
            || !signs.insert(&provenance.sign_id)
            || provenance.source.is_empty()
            || provenance.observed_at_ms > basis.now_ms
            || basis.now_ms > provenance.valid_until_ms
        {
            return invalid("accelerator observation is malformed, stale, or unbounded");
        }
    }
    Ok(())
}

fn evaluate(
    candidate: &AcceleratorCandidate,
    basis: &AcceleratorPlanningBasis,
) -> AcceleratorCandidateEvidence {
    let mut evidence = AcceleratorCandidateEvidence {
        candidate_id: candidate.candidate_id.clone(),
        disposition: AcceleratorCandidateDisposition::Admitted,
        compute_work_units: candidate.compute_work_units,
        transfer_work_units: candidate.transfer_work_units,
        setup_work_units: candidate.setup_work_units,
        residency_credit_work_units: 0,
        total_work_units: 0,
        reservations: Vec::new(),
        supporting_sign_ids: Vec::new(),
    };
    let mut totals =
        BTreeMap::<(HostId, BootId, ResourcePoolId, u64, AcceleratorDimension), u64>::new();
    for demand in &candidate.demands {
        match &demand.mechanism {
            ExecutionMechanism::Cpu => {
                if !demand.dimensions.is_empty() {
                    return reject(evidence, "CPU demand may not invent accelerator dimensions");
                }
            }
            ExecutionMechanism::Accelerator {
                host_id,
                boot_id,
                offer_generation,
                capability_id,
                implementation_id,
                pool_id,
                resource_generation,
                residency_artifact,
            } => {
                let Some(offer) = basis.offers.iter().find(|offer| {
                    &offer.host_id == host_id
                        && &offer.boot_id == boot_id
                        && offer.offer_generation == *offer_generation
                        && &offer.capability_id == capability_id
                        && &offer.implementation_id == implementation_id
                        && &offer.pool_id == pool_id
                }) else {
                    return reject(
                        evidence,
                        "accelerator presence is not an exact usable implementation offer",
                    );
                };
                let Some(observation) = basis.observations.iter().find(|observation| {
                    &observation.host_id == host_id
                        && &observation.boot_id == boot_id
                        && observation.offer_generation == *offer_generation
                        && &observation.pool_id == pool_id
                        && observation.resource_generation == *resource_generation
                }) else {
                    return reject(
                        evidence,
                        "accelerator resource generation lacks current observation truth",
                    );
                };
                if !observation.runtime_usable {
                    return reject(evidence, "accelerator runtime or provider is unavailable");
                }
                if demand.dimensions.is_empty()
                    || demand.dimensions.len() > MAXIMUM_ACCELERATOR_DIMENSIONS
                {
                    return reject(
                        evidence,
                        "accelerator demand has no finite multidimensional requirement",
                    );
                }
                for (dimension, units) in &demand.dimensions {
                    if *units == 0 || !offer.capacities.contains_key(dimension) {
                        return reject(
                            evidence,
                            "accelerator demand names an absent or zero dimension",
                        );
                    }
                    let key = (
                        host_id.clone(),
                        boot_id.clone(),
                        pool_id.clone(),
                        *resource_generation,
                        dimension.clone(),
                    );
                    let Some(total) = totals.get(&key).copied().unwrap_or(0).checked_add(*units)
                    else {
                        return reject(evidence, "accelerator reservation total overflowed");
                    };
                    if total > *observation.unreserved.get(dimension).unwrap_or(&0)
                        || total > *offer.capacities.get(dimension).unwrap_or(&0)
                    {
                        return reject(
                            evidence,
                            "aggregate accelerator reservations exceed finite capacity",
                        );
                    }
                    totals.insert(key, total);
                }
                if residency_artifact
                    .as_ref()
                    .is_some_and(|artifact| observation.resident_artifacts.contains(artifact))
                {
                    evidence.residency_credit_work_units = evidence
                        .residency_credit_work_units
                        .saturating_add(basis.residency_credit_work_units);
                }
                evidence
                    .supporting_sign_ids
                    .push(observation.provenance.sign_id.clone());
                evidence.reservations.push(AcceleratorReservation {
                    gear_id: demand.gear_id.clone(),
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    capability_id: capability_id.clone(),
                    implementation_id: implementation_id.clone(),
                    pool_id: pool_id.clone(),
                    resource_generation: *resource_generation,
                    dimensions: demand.dimensions.clone(),
                });
            }
        }
    }
    let Some(gross) = candidate
        .compute_work_units
        .checked_add(candidate.transfer_work_units)
        .and_then(|value| value.checked_add(candidate.setup_work_units))
    else {
        return reject(evidence, "accelerator candidate cost overflowed");
    };
    evidence.total_work_units =
        gross.saturating_sub(evidence.residency_credit_work_units.min(gross));
    evidence
}

fn reject(
    mut evidence: AcceleratorCandidateEvidence,
    reason: &str,
) -> AcceleratorCandidateEvidence {
    evidence.disposition = AcceleratorCandidateDisposition::Rejected(reason.to_string());
    evidence
}

fn invalid<T>(reason: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidPlanningObservation(reason.to_string()))
}
