//! Renderer-neutral explanation for freshly re-admitted dormant equipment.

use conduit_planner::DormantReadmissionEvidence;
use serde::{Deserialize, Serialize};

pub const MAX_DORMANT_READMISSION_EXPLANATION_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DormantReadmissionExplanation {
    pub body_membership_id: String,
    pub previous_plan_id: String,
    pub plan_id: String,
    pub gear_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub offer_generation: u64,
    pub capability_id: String,
    pub implementation_id: String,
    pub resource_observation_signs: Vec<String>,
    pub line_observation_signs: Vec<String>,
    pub authority_grant_ids: Vec<String>,
    pub unused_before: bool,
    pub available_now: bool,
    pub selected_because_preferred_path_is_gone: bool,
    pub historical_boot_reused: bool,
    pub historical_authority_restored: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DormantReadmissionExplanationError {
    IncoherentEvidence,
    EvidenceTooLarge,
}

pub fn explain_dormant_readmission(
    evidence: &DormantReadmissionEvidence,
) -> Result<DormantReadmissionExplanation, DormantReadmissionExplanationError> {
    let candidate = &evidence.candidate;
    if !candidate.unused_before
        || !candidate.available_now
        || !evidence.selected_because_preferred_path_is_gone
        || evidence.historical_boot_reused
        || evidence.historical_authority_restored
        || evidence.previous_plan_id == evidence.plan_id
        || candidate.resource_observation_signs.is_empty()
        || candidate.line_observation_signs.is_empty()
    {
        return Err(DormantReadmissionExplanationError::IncoherentEvidence);
    }
    let summary = format!(
        "Host {} was unused before, is available now at fresh Boot {} / offer generation {}, and was selected because the preferred path is gone. Plan {} replaces {}; current resource Signs [{}], Line Signs [{}], and independently admitted authority [{}] establish the return. Historical Boot and authority were not reused.",
        candidate.host_id.as_str(),
        candidate.boot_id.as_str(),
        candidate.offer_generation.0,
        evidence.plan_id.as_str(),
        evidence.previous_plan_id.as_str(),
        candidate.resource_observation_signs.iter().map(|sign| sign.as_str()).collect::<Vec<_>>().join(", "),
        candidate.line_observation_signs.iter().map(|sign| sign.as_str()).collect::<Vec<_>>().join(", "),
        candidate.authority_grant_ids.iter().map(|grant| grant.as_str()).collect::<Vec<_>>().join(", "),
    );
    if summary.len() > MAX_DORMANT_READMISSION_EXPLANATION_BYTES {
        return Err(DormantReadmissionExplanationError::EvidenceTooLarge);
    }
    Ok(DormantReadmissionExplanation {
        body_membership_id: candidate.body_membership_id.clone(),
        previous_plan_id: evidence.previous_plan_id.as_str().into(),
        plan_id: evidence.plan_id.as_str().into(),
        gear_id: candidate.gear_id.as_str().into(),
        host_id: candidate.host_id.as_str().into(),
        boot_id: candidate.boot_id.as_str().into(),
        offer_generation: candidate.offer_generation.0,
        capability_id: candidate.capability_id.as_str().into(),
        implementation_id: candidate.implementation_id.as_str().into(),
        resource_observation_signs: candidate
            .resource_observation_signs
            .iter()
            .map(|sign| sign.as_str().into())
            .collect(),
        line_observation_signs: candidate
            .line_observation_signs
            .iter()
            .map(|sign| sign.as_str().into())
            .collect(),
        authority_grant_ids: candidate
            .authority_grant_ids
            .iter()
            .map(|grant| grant.as_str().into())
            .collect(),
        unused_before: candidate.unused_before,
        available_now: candidate.available_now,
        selected_because_preferred_path_is_gone: evidence.selected_because_preferred_path_is_gone,
        historical_boot_reused: evidence.historical_boot_reused,
        historical_authority_restored: evidence.historical_authority_restored,
        summary,
    })
}
