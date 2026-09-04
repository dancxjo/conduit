//! Renderer-neutral explanation of a scarred but semantically full realization.

use conduit_core::Plan;
use conduit_planner::RecursiveRecoveryEvidence;
use serde::{Deserialize, Serialize};

pub const MAX_RECURSIVE_RECOVERY_EXPLANATION_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecursiveRecoveryExplanation {
    pub semantic_profile: String,
    pub lost_direct_plan_id: String,
    pub replacement_plan_id: String,
    pub expanded_form_id: String,
    pub realization_backs: Vec<String>,
    pub hosts: Vec<String>,
    pub gears: Vec<String>,
    pub lines: Vec<String>,
    pub resource_binding_count: u16,
    pub authority_binding_count: u16,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursiveRecoveryExplanationError {
    IncoherentEvidence,
    EvidenceTooLarge,
}

pub fn explain_recursive_recovery(
    lost_direct: &Plan,
    replacement: &Plan,
    evidence: &RecursiveRecoveryEvidence,
) -> Result<RecursiveRecoveryExplanation, RecursiveRecoveryExplanationError> {
    let hosts = replacement
        .fragments
        .iter()
        .map(|fragment| {
            format!(
                "{}@{}",
                fragment.host_id.as_str(),
                fragment.boot_id.as_str()
            )
        })
        .collect::<Vec<_>>();
    let gears = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|gear| {
            format!(
                "{}:{}@{}:{}",
                gear.gear_id.as_str(),
                gear.implementation_id.as_str(),
                gear.host_id.as_str(),
                gear.boot_id.as_str()
            )
        })
        .collect::<Vec<_>>();
    let lines = replacement
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter_map(|connection| connection.selected_line.as_ref())
        .map(|line| line.line_id.as_str().to_string())
        .collect::<Vec<_>>();
    let realization_backs = replacement
        .realization_backs
        .iter()
        .map(|back| format!("{}:{}", back.invocation_path, back.checked_form_id.as_str()))
        .collect::<Vec<_>>();
    if lost_direct.plan_id == replacement.plan_id
        || lost_direct.source_document_id != replacement.source_document_id
        || lost_direct.checked_form_id != replacement.checked_form_id
        || hosts.len() != usize::from(evidence.host_count)
        || gears.len() != usize::from(evidence.expanded_gear_count)
        || lines.len() != usize::from(evidence.remote_connection_count)
        || realization_backs.is_empty()
    {
        return Err(RecursiveRecoveryExplanationError::IncoherentEvidence);
    }
    let summary = format!(
        "Semantic capability {} is unchanged. Preferred direct Plan {} is unavailable; fresh Plan {} exposes {} reviewed Back(s), {} exact Gear placements across {} Hosts, {} admitted Line appearances, {} resource bindings, and {} authority bindings. This is full-profile recursive realization, not fallback=true or automatic migration.",
        evidence.semantic_profile,
        lost_direct.plan_id.as_str(),
        replacement.plan_id.as_str(),
        realization_backs.len(),
        gears.len(),
        hosts.len(),
        lines.len(),
        evidence.resource_binding_count,
        evidence.authority_binding_count,
    );
    if summary.len() > MAX_RECURSIVE_RECOVERY_EXPLANATION_BYTES {
        return Err(RecursiveRecoveryExplanationError::EvidenceTooLarge);
    }
    Ok(RecursiveRecoveryExplanation {
        semantic_profile: evidence.semantic_profile.clone(),
        lost_direct_plan_id: lost_direct.plan_id.as_str().into(),
        replacement_plan_id: replacement.plan_id.as_str().into(),
        expanded_form_id: replacement.expanded_form_id.as_str().into(),
        realization_backs,
        hosts,
        gears,
        lines,
        resource_binding_count: evidence.resource_binding_count,
        authority_binding_count: evidence.authority_binding_count,
        summary,
    })
}
