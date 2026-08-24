//! Renderer-neutral evidence for an explicit survival-oriented Plan selection.

use conduit_planner::{
    SurvivalCandidateDisposition, SurvivalPlanSelection, SurvivalPlanningMode, SurvivalTradeoff,
};
use serde::{Deserialize, Serialize};

pub const MAX_SURVIVAL_POLICY_EXPLANATION_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurvivalPolicyExplanation {
    pub policy_id: String,
    pub policy_revision: u64,
    pub mode: String,
    pub selected_plan_id: String,
    pub previous_plan_id: Option<String>,
    pub fresh_plan: bool,
    pub profile_disposition: String,
    pub principal_tradeoffs: Vec<String>,
    pub candidate_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivalPolicyExplanationError {
    IncoherentEvidence,
    EvidenceTooLarge,
}

pub fn explain_survival_plan_selection(
    selection: &SurvivalPlanSelection,
) -> Result<SurvivalPolicyExplanation, SurvivalPolicyExplanationError> {
    if selection.policy_id.is_empty()
        || selection.policy_revision == 0
        || selection.principal_tradeoffs.is_empty()
        || selection.candidate_evidence.is_empty()
        || !selection
            .candidate_evidence
            .iter()
            .any(|(plan_id, disposition)| {
                plan_id == &selection.selected_plan_id
                    && *disposition == conduit_planner::SurvivalCandidateEvidence::Selected
            })
    {
        return Err(SurvivalPolicyExplanationError::IncoherentEvidence);
    }
    let mode = match selection.mode {
        SurvivalPlanningMode::Normal => "normal",
        SurvivalPlanningMode::Survival => "survival",
    };
    let profile_disposition = match &selection.selected_disposition {
        SurvivalCandidateDisposition::FullyCompatible => "full-profile".to_string(),
        SurvivalCandidateDisposition::ReviewedDegraded {
            profile_id,
            admission_policy_id,
        } => format!("reviewed-degraded:{profile_id} via {admission_policy_id}"),
    };
    let principal_tradeoffs = selection
        .principal_tradeoffs
        .iter()
        .map(tradeoff_name)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let plan_freshness = if selection.fresh_plan {
        "fresh"
    } else {
        "current"
    };
    let summary = format!(
        "Policy {} revision {} selected {} ordinary Plan {} in {} mode from {} truthful candidates. Profile disposition is {}. Principal tradeoffs: {}. Hard semantics, authority, reservations, and reviewed degradation admission remain mandatory.",
        selection.policy_id,
        selection.policy_revision,
        plan_freshness,
        selection.selected_plan_id.as_str(),
        mode,
        selection.candidate_evidence.len(),
        profile_disposition,
        principal_tradeoffs.join(", "),
    );
    if summary.len() > MAX_SURVIVAL_POLICY_EXPLANATION_BYTES {
        return Err(SurvivalPolicyExplanationError::EvidenceTooLarge);
    }
    Ok(SurvivalPolicyExplanation {
        policy_id: selection.policy_id.clone(),
        policy_revision: selection.policy_revision,
        mode: mode.into(),
        selected_plan_id: selection.selected_plan_id.as_str().into(),
        previous_plan_id: selection
            .previous_plan_id
            .as_ref()
            .map(|plan_id| plan_id.as_str().into()),
        fresh_plan: selection.fresh_plan,
        profile_disposition,
        principal_tradeoffs,
        candidate_count: selection.candidate_evidence.len(),
        summary,
    })
}

fn tradeoff_name(tradeoff: &SurvivalTradeoff) -> &'static str {
    match tradeoff {
        SurvivalTradeoff::PreferFullProfile => "prefer full profile",
        SurvivalTradeoff::MinimizeUnavailablePrerequisites => "minimize unavailable prerequisites",
        SurvivalTradeoff::MinimizeSharedDependencyExposure => "minimize shared dependency exposure",
        SurvivalTradeoff::MinimizeHopCount => "minimize hop count",
        SurvivalTradeoff::MinimizeLatency => "minimize latency",
        SurvivalTradeoff::MinimizeResourceUnits => "minimize resource units",
    }
}
