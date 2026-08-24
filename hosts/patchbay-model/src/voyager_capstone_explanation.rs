//! Renderer-neutral scar-tissue timeline for the Voyager capstone.

use conduit_planner::{VoyagerCapstoneEvidence, VoyagerProofClass, VoyagerScarKind};
use serde::{Deserialize, Serialize};

pub const MAX_VOYAGER_CAPSTONE_EXPLANATION_BYTES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoyagerCapstoneExplanation {
    pub proof_class: String,
    pub stages: Vec<VoyagerScarStageExplanation>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoyagerScarStageExplanation {
    pub stage_id: String,
    pub observation_generation: u64,
    pub observation_signs: Vec<String>,
    pub plan_id: Option<String>,
    pub selected_hosts: Vec<String>,
    pub selected_implementations: Vec<String>,
    pub resource_binding_count: usize,
    pub authority_binding_count: usize,
    pub what_failed: Vec<String>,
    pub what_still_works: String,
    pub what_is_degraded: Vec<String>,
    pub what_returned: Vec<String>,
    pub what_old_equipment_reentered: Vec<String>,
    pub what_realization_expanded: Vec<String>,
    pub lines_carrying_work: Vec<String>,
    pub why_selected: Vec<String>,
    pub what_remains_impossible: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyagerCapstoneExplanationError {
    IncoherentEvidence,
    EvidenceTooLarge,
}

pub fn explain_voyager_capstone(
    evidence: &VoyagerCapstoneEvidence,
) -> Result<VoyagerCapstoneExplanation, VoyagerCapstoneExplanationError> {
    if evidence.stages.len() < 7
        || evidence.historical_plan_ids.is_empty()
        || evidence.observation_sign_count == 0
        || evidence.stages.iter().any(|stage| {
            stage.stage_id.is_empty()
                || stage.observation_signs.is_empty()
                || stage.scars.is_empty()
        })
    {
        return Err(VoyagerCapstoneExplanationError::IncoherentEvidence);
    }
    let proof_class = match evidence.proof_class {
        VoyagerProofClass::DeterministicCiFixture => "deterministic-ci-fixture (not physical/HIL)",
        VoyagerProofClass::PhysicalHil => "physical-HIL",
    };
    let stages = evidence
        .stages
        .iter()
        .map(|stage| {
            let mut degraded = Vec::new();
            let mut returned = Vec::new();
            let mut old = Vec::new();
            let mut expanded = Vec::new();
            let mut why = Vec::new();
            let mut impossible = Vec::new();
            for scar in &stage.scars {
                match scar {
                    VoyagerScarKind::HealthyPreferred => {
                        why.push("ordinary policy selected the efficient preferred realization".into())
                    }
                    VoyagerScarKind::ExactRedundancy => {
                        returned.push("equivalent redundant realization".into())
                    }
                    VoyagerScarKind::MechanismReroute => {
                        returned.push("materially different implementation mechanism".into())
                    }
                    VoyagerScarKind::LinePathReroute => {
                        returned.push("materially different Line path".into())
                    }
                    VoyagerScarKind::ExplicitDegradation { profile_id } => {
                        degraded.push(format!("explicit reviewed profile {profile_id}"))
                    }
                    VoyagerScarKind::DormantReadmission { host_id } => {
                        old.push(format!("{host_id} from fresh current truth"));
                        returned.push("dormant equipment capacity".into());
                    }
                    VoyagerScarKind::RecursiveRecovery { semantic_profile } => {
                        expanded.push(format!("recursive cross-Host graph for {semantic_profile}"));
                        returned.push("lost direct semantic capability".into());
                    }
                    VoyagerScarKind::SurvivalPolicy { policy_id } => {
                        why.push(format!("explicit survival policy {policy_id}"))
                    }
                    VoyagerScarKind::Irrecoverable { requirement_id } => {
                        impossible.push(requirement_id.clone())
                    }
                }
            }
            VoyagerScarStageExplanation {
                stage_id: stage.stage_id.clone(),
                observation_generation: stage.observation_generation,
                observation_signs: stage
                    .observation_signs
                    .iter()
                    .map(|sign| sign.as_str().into())
                    .collect(),
                plan_id: stage.plan_id.as_ref().map(|id| id.as_str().into()),
                selected_hosts: stage.host_ids.clone(),
                selected_implementations: stage.implementation_ids.clone(),
                resource_binding_count: stage.resource_binding_count,
                authority_binding_count: stage.authority_binding_count,
                what_failed: stage.failed_facts.clone(),
                what_still_works: format!(
                    "{} full, {} degraded, {} unavailable capabilities across {} Hosts, {} Bases, and {} Lines",
                    stage.metrics.full_capabilities,
                    stage.metrics.degraded_capabilities,
                    stage.metrics.unavailable_capabilities,
                    stage.metrics.surviving_hosts,
                    stage.metrics.surviving_bases,
                    stage.metrics.surviving_lines,
                ),
                what_is_degraded: degraded,
                what_returned: returned,
                what_old_equipment_reentered: old,
                what_realization_expanded: expanded,
                lines_carrying_work: core::iter::once(format!(
                    "{} admitted hops carrying {} bounded bytes at estimated {} us item latency",
                    stage.metrics.line_hops,
                    stage.metrics.admitted_line_bytes,
                    stage.metrics.estimated_item_latency_us,
                ))
                .chain(stage.line_ids.iter().cloned())
                .collect(),
                why_selected: why,
                what_remains_impossible: impossible,
            }
        })
        .collect::<Vec<_>>();
    let summary = format!(
        "Voyager {} records {} ordered damage stages, {} immutable historical Plans, and {} fresh observation Signs. It preserves distinct redundancy, mechanism rerouting, Line-path rerouting, explicit degradation, dormant re-entry, recursive recovery, survival-policy choice, and true irrecoverability; it does not claim universal self-healing.",
        proof_class,
        stages.len(),
        evidence.historical_plan_ids.len(),
        evidence.observation_sign_count,
    );
    let text_bytes = stages
        .iter()
        .flat_map(|stage| {
            core::iter::once(stage.stage_id.as_str())
                .chain(stage.observation_signs.iter().map(String::as_str))
                .chain(stage.plan_id.iter().map(String::as_str))
                .chain(stage.selected_hosts.iter().map(String::as_str))
                .chain(stage.selected_implementations.iter().map(String::as_str))
                .chain(stage.what_failed.iter().map(String::as_str))
                .chain(core::iter::once(stage.what_still_works.as_str()))
                .chain(stage.what_is_degraded.iter().map(String::as_str))
                .chain(stage.what_returned.iter().map(String::as_str))
                .chain(
                    stage
                        .what_old_equipment_reentered
                        .iter()
                        .map(String::as_str),
                )
                .chain(stage.what_realization_expanded.iter().map(String::as_str))
                .chain(stage.lines_carrying_work.iter().map(String::as_str))
                .chain(stage.why_selected.iter().map(String::as_str))
                .chain(stage.what_remains_impossible.iter().map(String::as_str))
        })
        .try_fold(proof_class.len() + summary.len(), |total, value| {
            total.checked_add(value.len())
        })
        .ok_or(VoyagerCapstoneExplanationError::EvidenceTooLarge)?;
    if text_bytes > MAX_VOYAGER_CAPSTONE_EXPLANATION_BYTES {
        return Err(VoyagerCapstoneExplanationError::EvidenceTooLarge);
    }
    Ok(VoyagerCapstoneExplanation {
        proof_class: proof_class.into(),
        stages,
        summary,
    })
}
