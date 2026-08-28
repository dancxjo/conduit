use crate::observations::{observations_admit, validate_resource_observations};
use crate::prelude::*;
use crate::realization::consume_selected_capacity;
use crate::PlannerError;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{HostAdvertisement, LineAvailability, LineOffer};
use conduit_form::CheckedForm;
mod model;
pub use model::*;

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
    if line_offers.len() > MAXIMUM_LOCALITY_LINE_OFFERS {
        return invalid("Line offer count exceeds the locality planning bound");
    }
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
        + basis.local_cords.len()
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
    let mut observation_signs = BTreeSet::new();
    for provenance in core::iter::once(&basis.data_flow.provenance)
        .chain(basis.reductions.iter().map(|item| &item.provenance))
        .chain(basis.realization_work.iter().map(|item| &item.provenance))
        .chain(basis.transports.iter().map(|item| &item.provenance))
        .chain(basis.local_cords.iter().map(|item| &item.provenance))
    {
        if provenance.sign_id.as_str().is_empty()
            || !observation_signs.insert(&provenance.sign_id)
            || provenance.source.is_empty()
            || provenance.observed_at_ms > basis.now_ms
            || basis.now_ms > provenance.valid_until_ms
        {
            return invalid("planning observations must have non-empty, fresh provenance");
        }
    }
    if basis
        .resources
        .iter()
        .any(|item| !observation_signs.insert(&item.sign_id))
    {
        return invalid("planning observation Sign identities must be unique");
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
    let initial_bytes_per_second = match basis
        .data_flow
        .items_per_second
        .checked_mul(u64::from(basis.data_flow.bytes_per_item))
    {
        Some(value) => value,
        None => {
            reject(&mut evidence, "data-flow byte rate overflowed");
            return evidence;
        }
    };
    let mut rates = BTreeMap::from([(
        basis.data_flow.source_gear_id.clone(),
        (basis.data_flow.items_per_second, initial_bytes_per_second),
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
                let Some(output_items) = items
                    .checked_mul(u64::from(reduction.output_items_numerator))
                    .map(|value| value / u64::from(reduction.input_items_denominator))
                else {
                    reject(&mut evidence, "reduced item rate overflowed");
                    return evidence;
                };
                let Some(output_bytes) = bytes
                    .checked_mul(u64::from(reduction.output_bytes_numerator))
                    .map(|value| value / u64::from(reduction.input_bytes_denominator))
                else {
                    reject(&mut evidence, "reduced byte rate overflowed");
                    return evidence;
                };
                rates.insert(gear.gear_id.clone(), (output_items, output_bytes));
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
            let rate = rates
                .get(&gear.gear_id)
                .copied()
                .unwrap_or((basis.data_flow.items_per_second, initial_bytes_per_second));
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
                if basis
                    .remote_bytes_per_second_ceiling
                    .is_some_and(|ceiling| bytes_per_second > ceiling)
                {
                    reject(&mut evidence, "remote transport policy ceiling is exceeded");
                    return evidence;
                }
                if bytes_per_second > line.throughput_bytes_per_second {
                    reject(&mut evidence, "observed Line throughput is insufficient");
                    return evidence;
                }
                let Some(bytes) = bytes_per_second.checked_mul(u64::from(basis.horizon_seconds))
                else {
                    reject(&mut evidence, "transport byte horizon overflowed");
                    return evidence;
                };
                let Some(transfer) = bytes.checked_add(1023).and_then(|value| {
                    let kibibytes = value / 1024;
                    kibibytes
                        .checked_mul(line.bandwidth_work_units_per_kibibyte)
                        .and_then(|bandwidth| {
                            kibibytes
                                .checked_mul(line.serialization_work_units_per_kibibyte)
                                .and_then(|serialization| bandwidth.checked_add(serialization))
                        })
                }) else {
                    reject(&mut evidence, "transport work overflowed");
                    return evidence;
                };
                let Some(cost) = line
                    .setup_work_units
                    .checked_add(transfer)
                    .and_then(|value| value.checked_add(line.framing_work_units))
                    .and_then(|value| value.checked_add(line.queueing_work_units))
                    .and_then(|value| value.checked_add(line.latency_work_units))
                    .and_then(|value| value.checked_add(line.jitter_work_units))
                    .and_then(|value| value.checked_add(line.pressure_work_units))
                    .and_then(|value| value.checked_add(line.cancellation_work_units))
                    .and_then(|value| value.checked_add(line.loss_work_units))
                else {
                    reject(&mut evidence, "transport fixed cost overflowed");
                    return evidence;
                };
                let Some(transported) = evidence.transported_bytes.checked_add(bytes) else {
                    reject(&mut evidence, "transported byte total overflowed");
                    return evidence;
                };
                let Some(transport_work) = evidence.transport_work_units.checked_add(cost) else {
                    reject(&mut evidence, "transport work total overflowed");
                    return evidence;
                };
                evidence.transported_bytes = transported;
                evidence.transport_work_units = transport_work;
                evidence
                    .supporting_sign_ids
                    .push(line.provenance.sign_id.clone());
            } else {
                let Some(local) = basis.local_cords.iter().find(|item| {
                    item.source_gear_id == connection.source_gear_id
                        && item.sink_gear_id == connection.sink_gear_id
                        && item.host_id == source.host_id
                        && hosts.iter().any(|host| {
                            host.host_id == item.host_id && host.boot_id == item.boot_id
                        })
                }) else {
                    reject(
                        &mut evidence,
                        "local Cord lacks current coordination-cost evidence",
                    );
                    return evidence;
                };
                let Some(local_total) = evidence.transport_work_units.checked_add(local.work_units)
                else {
                    reject(&mut evidence, "local Cord work total overflowed");
                    return evidence;
                };
                evidence.transport_work_units = local_total;
                evidence
                    .supporting_sign_ids
                    .push(local.provenance.sign_id.clone());
            }
        }
    }
    let Some(total) = evidence
        .compute_work_units
        .checked_add(evidence.transport_work_units)
    else {
        reject(&mut evidence, "candidate total work overflowed");
        return evidence;
    };
    evidence.total_work_units = total;
    evidence
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidPlanningObservation(detail.to_string()))
}
