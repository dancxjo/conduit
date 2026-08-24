use super::*;

pub(super) fn validate_inventory(
    inventory: &VoyagerBodyInventory,
) -> Result<(), VoyagerCapstoneRefusal> {
    if inventory.general_purpose_hosts < 2
        || inventory.accelerator_hosts == 0
        || inventory.constrained_hosts < 2
        || inventory.sensor_input_capabilities == 0
        || inventory.presentation_capabilities == 0
        || inventory.line_mechanisms < 2
        || inventory.dormant_equipment == 0
        || inventory.recursive_realization_families == 0
    {
        return Err(VoyagerCapstoneRefusal::InvalidInventory);
    }
    Ok(())
}

pub(super) fn validate_metrics(
    metrics: &VoyagerStageMetrics,
) -> Result<(), VoyagerCapstoneRefusal> {
    if metrics.surviving_hosts == 0
        || metrics.surviving_bases == 0
        || metrics.full_capabilities
            + metrics.degraded_capabilities
            + metrics.unavailable_capabilities
            == 0
        || metrics.realization_gears == 0
        || metrics.realization_depth == 0
        || metrics.planning_work == 0
        || metrics.admitted_sessions == 0
    {
        return Err(VoyagerCapstoneRefusal::InvalidStageSequence);
    }
    Ok(())
}

fn validate_plan_metrics(
    plan: &Plan,
    metrics: &VoyagerStageMetrics,
) -> Result<(), VoyagerCapstoneRefusal> {
    let placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    let connections = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    let gears =
        u16::try_from(placements.len()).map_err(|_| VoyagerCapstoneRefusal::EvidenceOverflow)?;
    let hops = u16::try_from(
        connections
            .iter()
            .map(|connection| connection.admitted_lines.len())
            .sum::<usize>(),
    )
    .map_err(|_| VoyagerCapstoneRefusal::EvidenceOverflow)?;
    let bytes = connections
        .iter()
        .map(|connection| u64::from(connection.byte_capacity))
        .sum::<u64>();
    let resources = placements
        .iter()
        .flat_map(|placement| &placement.resources)
        .map(|binding| u64::from(binding.units))
        .sum::<u64>();
    let sessions =
        u16::try_from(connections.len()).map_err(|_| VoyagerCapstoneRefusal::EvidenceOverflow)?;
    if metrics.realization_gears != gears
        || metrics.line_hops != hops
        || metrics.admitted_line_bytes != bytes
        || metrics.reserved_resource_units != resources
        || metrics.admitted_sessions != sessions
    {
        return Err(VoyagerCapstoneRefusal::IncoherentTypedEvidence);
    }
    Ok(())
}

pub(super) fn validate_phenomenon(
    index: usize,
    stage: &VoyagerDamageStage<'_>,
    phenomenon: &VoyagerPhenomenon,
    plans: &[Plan],
    observed: &mut ObservedPhenomena,
) -> Result<(), VoyagerCapstoneRefusal> {
    if let Some(plan) = stage.plan {
        validate_plan_metrics(plan, &stage.metrics)?;
    }
    let plan_id = stage.plan.map(|plan| &plan.plan_id);
    match phenomenon {
        VoyagerPhenomenon::HealthyPreferred {
            retained_dominated_families,
            activated_dominated_families,
        } if index == 0
            && *retained_dominated_families > 0
            && *activated_dominated_families == 0
            && stage.plan.is_some_and(|plan| plan.fragments.len() == 1) =>
        {
            observed.healthy = true
        }
        VoyagerPhenomenon::ExactRedundantReplacement { previous_plan_id }
            if stage.previous_plan_id.as_ref() == Some(previous_plan_id)
                && stage.plan.is_some_and(|current| {
                    plans.iter().any(|previous| {
                        &previous.plan_id == previous_plan_id
                            && current.plan_id != previous.plan_id
                            && current.expanded_form_id == previous.expanded_form_id
                            && placement_kinds(current) == placement_kinds(previous)
                            && placement_hosts(current) != placement_hosts(previous)
                    })
                }) =>
        {
            observed.redundancy = true;
        }
        VoyagerPhenomenon::DiverseReplacement(evidence)
            if stage.previous_plan_id.as_ref() == Some(&evidence.previous_plan_id)
                && plan_id == Some(&evidence.replacement_plan_id)
                && !evidence.replacement_mechanisms.is_empty()
                && !evidence.replacement_line_path.is_empty()
                && !evidence.unavailable_previous_dependencies.is_empty() =>
        {
            match evidence.relationship {
                DiversityRelationship::MechanismDiverse => observed.mechanism = true,
                DiversityRelationship::LinePathDiverse => observed.path = true,
                DiversityRelationship::MechanismAndLinePathDiverse => {
                    observed.mechanism = true;
                    observed.path = true;
                }
                _ => return Err(VoyagerCapstoneRefusal::IncoherentTypedEvidence),
            }
        }
        VoyagerPhenomenon::ExplicitDegradation {
            plan_id: id,
            admission,
        } if plan_id == Some(id)
            && admission.disposition == ServiceProfileDisposition::Degraded
            && admission.policy_id.as_ref().is_some_and(|id| valid_id(id))
            && admission.policy_revision.is_some()
            && !admission.dimensions.is_empty()
            && !admission.observation_signs.is_empty() =>
        {
            observed.degradation = true;
        }
        VoyagerPhenomenon::DormantReadmission(evidence)
            if stage.previous_plan_id.as_ref() == Some(&evidence.previous_plan_id)
                && plan_id == Some(&evidence.plan_id)
                && evidence.candidate.available_now
                && evidence.candidate.unused_before
                && !evidence.candidate.resource_observation_signs.is_empty()
                && !evidence.candidate.line_observation_signs.is_empty()
                && !evidence.candidate.authority_grant_ids.is_empty()
                && evidence.selected_because_preferred_path_is_gone
                && !evidence.historical_boot_reused
                && !evidence.historical_authority_restored =>
        {
            observed.dormant = true;
        }
        VoyagerPhenomenon::RecursiveRecovery {
            lost_plan_id,
            evidence,
        } if stage.previous_plan_id.as_ref() == Some(lost_plan_id)
            && plan_id.is_some_and(|id| id != lost_plan_id)
            && evidence.host_count >= 2
            && evidence.remote_connection_count > 0
            && evidence.expansion_depth > 0 =>
        {
            observed.recursive = true;
        }
        VoyagerPhenomenon::SurvivalPolicyDecision {
            truth_generation,
            normal_refusal,
            survival_selection,
            scarce_resource_triage,
            hard_failure_refused_under_both,
        } if *truth_generation == stage.observation_generation
            && *normal_refusal == SurvivalPolicyRefusal::NormalCostEnvelopeExceeded
            && survival_selection.mode == SurvivalPlanningMode::Survival
            && survival_selection.fresh_plan
            && plan_id == Some(&survival_selection.selected_plan_id)
            && stage.previous_plan_id == survival_selection.previous_plan_id
            && scarce_resource_triage.reserved_units > 0
            && scarce_resource_triage
                .decisions
                .iter()
                .any(|decision| decision.disposition == ScarceResourceDisposition::Reserved)
            && scarce_resource_triage.decisions.iter().any(|decision| {
                decision.disposition == ScarceResourceDisposition::RefusedCapacity
            })
            && *hard_failure_refused_under_both =>
        {
            observed.policy = true;
        }
        VoyagerPhenomenon::Irrecoverable {
            requirement_id,
            reason: _,
        } if valid_id(requirement_id)
            && stage.plan.is_none()
            && stage.metrics.unavailable_capabilities > 0 =>
        {
            observed.irrecoverable = true
        }
        _ => return Err(VoyagerCapstoneRefusal::IncoherentTypedEvidence),
    }
    Ok(())
}

fn placement_kinds(plan: &Plan) -> Vec<&str> {
    let mut kinds = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.kind_id.as_str())
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    kinds
}

fn placement_hosts(plan: &Plan) -> Vec<&str> {
    let mut hosts = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.host_id.as_str())
        .collect::<Vec<_>>();
    hosts.sort_unstable();
    hosts.dedup();
    hosts
}
