//! Exact LLM interruption and replacement evidence over immutable Plans.

use alloc::{collections::BTreeSet, string::String, vec::Vec};
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayId, ActivePlayIdentity, BootId, CheckedFormId,
    ExpandedFormId, HostId, ImplementationId, OfferGeneration, Plan, PlanId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmInterruptionReason {
    ModelProviderLost,
    PartOrLineLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmPlanningRefusal {
    MissingLlmRealization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmRealizationPart {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementations: Vec<ImplementationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossHostLlmRun {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub request_id: String,
    pub parts: Vec<LlmRealizationPart>,
    pub remote_line_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptedLlmRun {
    pub run: CrossHostLlmRun,
    pub reason: LlmInterruptionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementLlmRun {
    pub interrupted: InterruptedLlmRun,
    pub current: CrossHostLlmRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossHostLlmError {
    InvalidPlan,
    InvalidPlay,
    MissingProviderBack,
    NotCrossHost,
    InvalidBound,
    InvalidIdentity,
    FormChanged,
    PlanReused,
    PlayReused,
    RealizationTruthNotFresh,
    StaleCompletion,
    UnknownCompletion,
}

impl CrossHostLlmRun {
    pub fn observe(
        plan: &Plan,
        play: &ActivePlayIdentity,
        request_id: String,
    ) -> Result<Self, CrossHostLlmError> {
        if !verify_plan(plan) {
            return Err(CrossHostLlmError::InvalidPlan);
        }
        if bind_active_play(
            &play.plan_id,
            &play.host_id,
            &play.boot_id,
            play.play_sequence,
        ) != *play
            || play.plan_id != plan.plan_id
        {
            return Err(CrossHostLlmError::InvalidPlay);
        }
        if request_id.is_empty() {
            return Err(CrossHostLlmError::InvalidIdentity);
        }
        if !plan
            .realization_backs
            .iter()
            .any(|back| back.kind_id.as_str() == crate::GENERATE_TEXT_KIND)
        {
            return Err(CrossHostLlmError::MissingProviderBack);
        }
        let mut parts = plan
            .fragments
            .iter()
            .map(|fragment| {
                let mut implementations = fragment
                    .placements
                    .iter()
                    .map(|placement| placement.implementation_id.clone())
                    .collect::<Vec<_>>();
                implementations.sort();
                implementations.dedup();
                LlmRealizationPart {
                    host_id: fragment.host_id.clone(),
                    boot_id: fragment.boot_id.clone(),
                    offer_generation: fragment.offer_generation,
                    implementations,
                }
            })
            .collect::<Vec<_>>();
        parts.sort_by(|left, right| left.host_id.cmp(&right.host_id));
        let remote_lines = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter_map(|connection| connection.selected_line.as_ref())
            .collect::<Vec<_>>();
        if parts.len() < 2 || remote_lines.is_empty() {
            return Err(CrossHostLlmError::NotCrossHost);
        }
        if remote_lines.iter().any(|line| {
            line.binding.limits.maximum_in_flight_items == 0
                || line.binding.limits.maximum_payload_bytes == 0
                || line.binding.limits.maximum_buffered_bytes == 0
                || line.binding.limits.maximum_frame_bytes == 0
        }) {
            return Err(CrossHostLlmError::InvalidBound);
        }
        Ok(Self {
            source_document_id: plan.source_document_id.clone(),
            checked_form_id: plan.checked_form_id.clone(),
            expanded_form_id: plan.expanded_form_id.clone(),
            plan_id: plan.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            request_id,
            parts,
            remote_line_count: u16::try_from(remote_lines.len())
                .map_err(|_| CrossHostLlmError::InvalidBound)?,
        })
    }

    pub fn interrupted(self, reason: LlmInterruptionReason) -> InterruptedLlmRun {
        InterruptedLlmRun { run: self, reason }
    }
}

impl ReplacementLlmRun {
    pub fn start(
        interrupted: InterruptedLlmRun,
        current: CrossHostLlmRun,
    ) -> Result<Self, CrossHostLlmError> {
        let old = &interrupted.run;
        if (
            old.source_document_id.clone(),
            old.checked_form_id.clone(),
            old.expanded_form_id.clone(),
        ) != (
            current.source_document_id.clone(),
            current.checked_form_id.clone(),
            current.expanded_form_id.clone(),
        ) {
            return Err(CrossHostLlmError::FormChanged);
        }
        if old.plan_id == current.plan_id {
            return Err(CrossHostLlmError::PlanReused);
        }
        if old.active_play_id == current.active_play_id {
            return Err(CrossHostLlmError::PlayReused);
        }
        let old_truth = old
            .parts
            .iter()
            .map(|part| (&part.host_id, &part.boot_id, part.offer_generation))
            .collect::<BTreeSet<_>>();
        let new_truth = current
            .parts
            .iter()
            .map(|part| (&part.host_id, &part.boot_id, part.offer_generation))
            .collect::<BTreeSet<_>>();
        if old_truth == new_truth {
            return Err(CrossHostLlmError::RealizationTruthNotFresh);
        }
        Ok(Self {
            interrupted,
            current,
        })
    }

    pub fn accept_completion(
        &self,
        plan_id: &PlanId,
        play_id: &ActivePlayId,
        request_id: &str,
    ) -> Result<(), CrossHostLlmError> {
        if plan_id == &self.interrupted.run.plan_id
            || play_id == &self.interrupted.run.active_play_id
        {
            return Err(CrossHostLlmError::StaleCompletion);
        }
        if plan_id != &self.current.plan_id
            || play_id != &self.current.active_play_id
            || request_id != self.current.request_id
        {
            return Err(CrossHostLlmError::UnknownCompletion);
        }
        Ok(())
    }
}

pub fn classify_missing_llm_plan<T>(candidate: Option<T>) -> Result<T, LlmPlanningRefusal> {
    candidate.ok_or(LlmPlanningRefusal::MissingLlmRealization)
}
