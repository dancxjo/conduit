use crate::prelude::*;
use crate::{
    plan_with_options, select_data_locality_candidate, CandidatePlacementDisposition,
    LocalityPlanningBasis, PlannerError, PlanningOptions,
};
use alloc::collections::BTreeSet;
use conduit_core::{
    seal_plan_with_realization_backs, ConnectionBase, FusionId, GearId, HostAdvertisement,
    PlannedFusion,
};
use conduit_form::CheckedForm;
mod model;
pub use model::*;

pub fn select_fusion_candidate(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidates: &[FusionCandidate],
    locality_basis: &LocalityPlanningBasis,
    inputs: FusionPlanningInputs<'_>,
) -> Result<FusionSelection, PlannerError> {
    validate_fusion_inputs(
        candidates,
        inputs.offers,
        inputs.observations,
        locality_basis.now_ms,
    )?;
    let mut considered = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let locality = match select_data_locality_candidate(
            form,
            hosts,
            core::slice::from_ref(&candidate.realization),
            locality_basis,
            inputs.line_offers,
        ) {
            Ok(selection) => selection
                .considered
                .into_iter()
                .next()
                .expect("one candidate"),
            Err(error) => {
                considered.push(rejected(candidate, error.to_string()));
                continue;
            }
        };
        let mut evidence = FusionCandidateEvidence {
            candidate_id: candidate.candidate_id.clone(),
            disposition: CandidatePlacementDisposition::Admitted,
            compute_work_units: locality.compute_work_units,
            transport_work_units: locality.transport_work_units,
            transported_bytes: locality.transported_bytes,
            total_work_units: locality.total_work_units,
            fusion_groups: Vec::new(),
            supporting_sign_ids: locality.supporting_sign_ids,
        };
        if let Err(reason) = apply_fusions(
            form,
            hosts,
            candidate,
            locality_basis,
            inputs,
            &mut evidence,
        ) {
            evidence.disposition = CandidatePlacementDisposition::Rejected(reason);
        }
        considered.push(evidence);
    }
    let selected_index = considered
        .iter()
        .enumerate()
        .filter(|(_, item)| item.disposition == CandidatePlacementDisposition::Admitted)
        .min_by_key(|(_, item)| (item.total_work_units, item.candidate_id.as_str()))
        .map(|(index, _)| index)
        .ok_or_else(|| {
            PlannerError::CurrentResourceObservationUnavailable(
                "no fusion candidate has complete admissible evidence".to_string(),
            )
        })?;
    considered[selected_index].disposition = CandidatePlacementDisposition::Selected;
    let selected = &candidates[selected_index];
    Ok(FusionSelection {
        checked_form_id: form.checked_form_id.clone(),
        selected_candidate_id: selected.candidate_id.clone(),
        selected_realization: selected.realization.clone(),
        selected_fusion_groups: considered[selected_index].fusion_groups.clone(),
        considered,
        locality_basis: locality_basis.clone(),
        fusion_observations: inputs.observations.to_vec(),
    })
}

pub fn plan_selected_optimization(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    selection: &FusionSelection,
    bases: &[ConnectionBase],
    options: PlanningOptions<'_>,
) -> Result<OptimizedPlan, PlannerError> {
    if selection.checked_form_id != form.checked_form_id {
        return Err(PlannerError::InvalidFormIdentity(
            "fusion selection belongs to a different checked Form".to_string(),
        ));
    }
    let mut plan = plan_with_options(
        form,
        hosts,
        &selection.selected_realization.placements,
        bases,
        options,
    )?;
    for group in &selection.selected_fusion_groups {
        let fragment = plan
            .fragments
            .iter_mut()
            .find(|fragment| fragment.host_id == group.host_id && fragment.boot_id == group.boot_id)
            .ok_or_else(|| {
                PlannerError::InvalidPlanningObservation(
                    "fusion Host has no matching Plan fragment".to_string(),
                )
            })?;
        let mut preserved_placements = group
            .preserved_gear_ids
            .iter()
            .map(|gear_id| {
                fragment
                    .placements
                    .iter()
                    .find(|placement| &placement.gear_id == gear_id)
                    .map(|placement| placement.placement_id.clone())
                    .ok_or_else(|| {
                        PlannerError::InvalidPlanningObservation(
                            "fusion Gear has no exact Plan placement".to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        preserved_placements.sort();
        let mut preserved_connections = group
            .preserved_cords
            .iter()
            .map(|(source_gear, sink_gear)| {
                let source = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.gear_id == *source_gear)
                    .expect("fusion Gear placement was collected");
                let sink = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.gear_id == *sink_gear)
                    .expect("fusion Gear placement was collected");
                fragment
                    .connections
                    .iter()
                    .find(|connection| {
                        connection.source_placement_id == source.placement_id
                            && connection.sink_placement_id == sink.placement_id
                    })
                    .map(|connection| connection.connection_id.clone())
                    .ok_or_else(|| {
                        PlannerError::InvalidPlanningObservation(
                            "fusion Cord has no exact Plan connection".to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        preserved_connections.sort();
        fragment.execution_fusions.push(PlannedFusion {
            fusion_id: FusionId::from(group.fusion_id.clone()),
            execution_profile_id: group.execution_profile_id.clone(),
            implementation_id: group.implementation_id.clone(),
            artifact_id: group.artifact_id.clone(),
            preserved_placements,
            preserved_connections,
        });
        fragment
            .execution_fusions
            .sort_by(|left, right| left.fusion_id.cmp(&right.fusion_id));
    }
    plan = seal_plan_with_realization_backs(
        form.identity(),
        plan.realization_backs.clone(),
        plan.fragments,
    );
    let optimized = OptimizedPlan {
        plan,
        fusion_groups: selection.selected_fusion_groups.clone(),
    };
    if !optimized.verify() {
        return Err(PlannerError::InvalidPlanningObservation(
            "fusion explanation does not match the ordinary Plan".to_string(),
        ));
    }
    Ok(optimized)
}

fn apply_fusions(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidate: &FusionCandidate,
    basis: &LocalityPlanningBasis,
    inputs: FusionPlanningInputs<'_>,
    evidence: &mut FusionCandidateEvidence,
) -> Result<(), String> {
    let mut fused_gears = BTreeSet::new();
    for fusion_id in &candidate.fusion_ids {
        let offer = inputs
            .offers
            .iter()
            .find(|offer| &offer.fusion_id == fusion_id)
            .ok_or_else(|| "selected fusion is not offered".to_string())?;
        let observation = inputs
            .observations
            .iter()
            .find(|item| &item.fusion_id == fusion_id)
            .ok_or_else(|| "selected fusion lacks current work evidence".to_string())?;
        validate_fusion_offer(
            form,
            hosts,
            candidate,
            offer,
            inputs.boundaries,
            &mut fused_gears,
        )?;
        let replaced_work = offer
            .gear_ids
            .iter()
            .try_fold(0u64, |total, gear_id| {
                let placement = candidate
                    .realization
                    .placements
                    .by_gear
                    .get(gear_id)
                    .ok_or(())?;
                let work = basis
                    .realization_work
                    .iter()
                    .find(|item| {
                        item.gear_id == *gear_id
                            && item.host_id == placement.host_id
                            && item.capability_id == placement.capability_id
                    })
                    .ok_or(())?;
                total.checked_add(work.work_units).ok_or(())
            })
            .map_err(|()| "fusion replacement work is missing or overflows".to_string())?;
        evidence.compute_work_units = evidence
            .compute_work_units
            .checked_sub(replaced_work)
            .and_then(|value| value.checked_add(observation.fused_work_units))
            .ok_or_else(|| "fusion work accounting overflowed".to_string())?;
        let removed_local_coordination = offer
            .internal_cords
            .iter()
            .try_fold(0u64, |total, (source_gear_id, sink_gear_id)| {
                let local = basis
                    .local_cords
                    .iter()
                    .find(|item| {
                        item.source_gear_id == *source_gear_id
                            && item.sink_gear_id == *sink_gear_id
                            && item.host_id == offer.host_id
                            && item.boot_id == offer.boot_id
                    })
                    .ok_or(())?;
                total.checked_add(local.work_units).ok_or(())
            })
            .map_err(|()| "fusion local-Cord cost is missing or overflows".to_string())?;
        evidence.transport_work_units = evidence
            .transport_work_units
            .checked_sub(removed_local_coordination)
            .ok_or_else(|| "fusion local-Cord accounting underflowed".to_string())?;
        evidence.total_work_units = evidence
            .compute_work_units
            .checked_add(evidence.transport_work_units)
            .ok_or_else(|| "fusion total work overflowed".to_string())?;
        evidence
            .supporting_sign_ids
            .push(observation.provenance.sign_id.clone());
        evidence.fusion_groups.push(FusionDecisionGroup {
            fusion_id: offer.fusion_id.clone(),
            host_id: offer.host_id.clone(),
            boot_id: offer.boot_id.clone(),
            execution_profile_id: offer.execution_profile_id.clone(),
            implementation_id: offer.implementation_id.clone(),
            artifact_id: offer.artifact_id.clone(),
            preserved_gear_ids: offer.gear_ids.clone(),
            preserved_cords: offer.internal_cords.clone(),
        });
    }
    Ok(())
}

fn validate_fusion_offer(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    candidate: &FusionCandidate,
    offer: &FusionRealizationOffer,
    boundaries: &[FusionBoundary],
    fused_gears: &mut BTreeSet<GearId>,
) -> Result<(), String> {
    let host = hosts
        .iter()
        .find(|host| host.host_id == offer.host_id)
        .ok_or_else(|| "fusion Host is absent".to_string())?;
    if host.boot_id != offer.boot_id || host.offer_generation != offer.offer_generation {
        return Err("fusion offer is stale for the selected Boot".to_string());
    }
    if offer.gear_ids.len() < 2
        || offer
            .gear_ids
            .iter()
            .any(|gear_id| !fused_gears.insert(gear_id.clone()))
        || offer.implementation_id.as_str().is_empty()
        || offer.execution_profile_id.as_str().is_empty()
        || offer.artifact_id.as_str().is_empty()
        || !(offer.preserves_typed_ports
            && offer.preserves_atomic_pressure
            && offer.preserves_cancellation
            && offer.preserves_required_evidence)
    {
        return Err("fusion offer does not preserve required runtime semantics".to_string());
    }
    for gear_id in &offer.gear_ids {
        let placement = candidate
            .realization
            .placements
            .by_gear
            .get(gear_id)
            .ok_or_else(|| "fusion omits an authored Gear placement".to_string())?;
        if placement.host_id != offer.host_id
            || !form.gears.iter().any(|gear| gear.gear_id == *gear_id)
        {
            return Err("fusion Gears are not all local on the offered Host".to_string());
        }
        host.capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or_else(|| "fusion member capability is not installed".to_string())?;
    }
    let gear_set = offer.gear_ids.iter().collect::<BTreeSet<_>>();
    let expected_cords = form
        .connections
        .iter()
        .filter(|connection| {
            gear_set.contains(&connection.source_gear_id)
                && gear_set.contains(&connection.sink_gear_id)
        })
        .map(|connection| {
            (
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let offered_cords = offer
        .internal_cords
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_cords.is_empty()
        || offered_cords.len() != offer.internal_cords.len()
        || offered_cords != expected_cords
    {
        return Err("fusion must preserve every exact internal authored Cord".to_string());
    }
    for cord in &offer.internal_cords {
        if !form.connections.iter().any(|connection| {
            connection.source_gear_id == cord.0 && connection.sink_gear_id == cord.1
        }) {
            return Err("fusion names a Cord absent from the authored Form".to_string());
        }
        if boundaries.iter().any(|boundary| {
            boundary.source_gear_id == cord.0
                && boundary.sink_gear_id == cord.1
                && (boundary.requires_observation || boundary.requires_authority)
        }) {
            return Err("required observation or authority boundary forbids fusion".to_string());
        }
    }
    Ok(())
}

fn validate_fusion_inputs(
    candidates: &[FusionCandidate],
    offers: &[FusionRealizationOffer],
    observations: &[FusionPlanningObservation],
    now_ms: u64,
) -> Result<(), PlannerError> {
    if candidates.is_empty() || candidates.len() > MAXIMUM_FUSION_CANDIDATES {
        return invalid("fusion candidate count is empty or exceeds its bound");
    }
    if candidates.iter().any(|item| {
        item.candidate_id.is_empty()
            || item.candidate_id != item.realization.candidate_id
            || item.fusion_ids.len() > MAXIMUM_FUSION_GROUPS
    }) {
        return invalid("fusion candidate identity or group count is invalid");
    }
    if offers.len() > MAXIMUM_FUSION_OFFERS
        || observations.len() > MAXIMUM_FUSION_OFFERS
        || offers.iter().any(|offer| {
            offer.fusion_id.is_empty()
                || offer.gear_ids.len() > MAXIMUM_FUSION_MEMBERS
                || offer.internal_cords.len() > MAXIMUM_FUSION_MEMBERS
        })
    {
        return invalid("fusion offer, observation, or member count exceeds its bound");
    }
    let unique = |values: Vec<&str>| {
        let count = values.len();
        values.into_iter().collect::<BTreeSet<_>>().len() == count
    };
    if !unique(
        candidates
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect(),
    ) || !unique(offers.iter().map(|item| item.fusion_id.as_str()).collect())
        || !unique(
            observations
                .iter()
                .map(|item| item.fusion_id.as_str())
                .collect(),
        )
    {
        return invalid("fusion candidate, offer, and observation identities must be unique");
    }
    for observation in observations {
        let provenance = &observation.provenance;
        if observation.fusion_id.is_empty()
            || provenance.sign_id.as_str().is_empty()
            || provenance.source.is_empty()
            || provenance.observed_at_ms > now_ms
            || now_ms > provenance.valid_until_ms
        {
            return invalid("fusion work observations require fresh exact provenance");
        }
    }
    Ok(())
}

fn rejected(candidate: &FusionCandidate, reason: String) -> FusionCandidateEvidence {
    FusionCandidateEvidence {
        candidate_id: candidate.candidate_id.clone(),
        disposition: CandidatePlacementDisposition::Rejected(reason),
        compute_work_units: 0,
        transport_work_units: 0,
        transported_bytes: 0,
        total_work_units: 0,
        fusion_groups: Vec::new(),
        supporting_sign_ids: Vec::new(),
    }
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidPlanningObservation(detail.to_string()))
}
