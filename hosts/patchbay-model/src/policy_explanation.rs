//! Bounded, renderer-neutral explanation of planner-owned realization choices.

use conduit_core::{GearId, Plan, PlanId, SignId};
use conduit_planner::{
    ObservationBasis, PlannerFactRef, PlanningPolicyBasis, PolicySourceRevision,
    RealizationDecisionDisposition, RealizationDecisionRecord, RealizationRejection, StyleId,
    StylePreferenceEvidence, StylePreferenceOutcome, MAXIMUM_REALIZATION_DECISION_RECORDS,
    MAXIMUM_RETAINED_POLICY_OBSERVATIONS,
};

pub const MAX_POLICY_EXPLANATIONS: usize = 64;
pub const MAX_STYLE_EXPLANATION_CLAUSES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyChoiceDomain {
    Realization,
    ComputeResource,
    PresentationStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChoiceSummary {
    pub domain: PolicyChoiceDomain,
    pub subject_label: String,
    pub selected_label: String,
    pub reason: String,
    pub style_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChoiceDetails {
    pub plan_id: PlanId,
    pub gear_id: GearId,
    pub candidates: Vec<RealizationDecisionRecord>,
    pub hard_requirements: Vec<PlannerFactRef>,
    pub policy_sources: Vec<PolicySourceRevision>,
    pub observations: Vec<ObservationBasis>,
    pub selected_realization_facts: Vec<String>,
    pub selected_resource_facts: Vec<String>,
    pub current_observation_signs: Vec<SignId>,
    pub style_preferences: Vec<StylePreferenceEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyChoiceExplanation {
    pub summary: PolicyChoiceSummary,
    details: PolicyChoiceDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReplanRequest {
    pub prior_plan_id: PlanId,
    pub gear_id: GearId,
    pub policy_source: PolicySourceRevision,
    pub style_id: Option<StyleId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyExplanationError {
    InvalidPlan,
    UnknownGear,
    SelectedCandidateMismatch,
    EvidenceTooLarge,
    ObservationBasisTooLarge,
    StyleEvidenceTooLarge,
    EmptySummary,
    StaleReplanBasis,
    InvalidPolicySource,
}

impl core::fmt::Display for PolicyExplanationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid planning explanation: {self:?}")
    }
}

impl std::error::Error for PolicyExplanationError {}

impl PolicyChoiceExplanation {
    #[allow(clippy::too_many_arguments)]
    pub fn from_planner_evidence(
        plan: &Plan,
        gear_id: &GearId,
        domain: PolicyChoiceDomain,
        subject_label: impl Into<String>,
        selected_label: impl Into<String>,
        reason: impl Into<String>,
        decisions: Vec<RealizationDecisionRecord>,
        hard_requirements: Vec<PlannerFactRef>,
        basis: PlanningPolicyBasis,
        style: Option<(StyleId, Vec<StylePreferenceEvidence>)>,
    ) -> Result<Self, PolicyExplanationError> {
        if !conduit_core::verify_plan(plan) {
            return Err(PolicyExplanationError::InvalidPlan);
        }
        if decisions.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
            return Err(PolicyExplanationError::EvidenceTooLarge);
        }
        if basis.observations.len() > MAXIMUM_RETAINED_POLICY_OBSERVATIONS {
            return Err(PolicyExplanationError::ObservationBasisTooLarge);
        }
        if style
            .as_ref()
            .is_some_and(|(_, clauses)| clauses.len() > MAX_STYLE_EXPLANATION_CLAUSES)
        {
            return Err(PolicyExplanationError::StyleEvidenceTooLarge);
        }
        let placement = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| &placement.gear_id == gear_id)
            .ok_or(PolicyExplanationError::UnknownGear)?;
        if !decisions.iter().any(|candidate| {
            candidate.gear_id == *gear_id
                && candidate.disposition == RealizationDecisionDisposition::Selected
                && candidate.host_id == placement.host_id
                && candidate.capability_id == placement.capability_id
                && candidate.implementation_id == placement.implementation_id
        }) {
            return Err(PolicyExplanationError::SelectedCandidateMismatch);
        }
        let subject_label = subject_label.into();
        let selected_label = selected_label.into();
        let reason = reason.into();
        if subject_label.is_empty() || selected_label.is_empty() || reason.is_empty() {
            return Err(PolicyExplanationError::EmptySummary);
        }
        let style_label = style
            .as_ref()
            .map(|(style_id, _)| friendly_identity(style_id.as_str()));
        let selected_realization_facts = placement
            .realization_characteristics
            .iter()
            .map(|fact| fact.definition.characteristic_id.as_str().to_owned())
            .collect();
        let selected_resource_facts = placement
            .resources
            .iter()
            .map(|binding| binding.class_id.as_str().to_owned())
            .collect();
        let current_observation_signs = basis
            .observations
            .iter()
            .map(|observation| observation.sign_id.clone())
            .collect();
        Ok(Self {
            summary: PolicyChoiceSummary {
                domain,
                subject_label,
                selected_label,
                reason,
                style_label,
            },
            details: PolicyChoiceDetails {
                plan_id: plan.plan_id.clone(),
                gear_id: gear_id.clone(),
                candidates: decisions,
                hard_requirements,
                policy_sources: basis.policy_sources,
                observations: basis.observations,
                selected_realization_facts,
                selected_resource_facts,
                current_observation_signs,
                style_preferences: style.map_or_else(Vec::new, |(_, clauses)| clauses),
            },
        })
    }

    /// Exact IDs, candidate dispositions, fact ownership, and provenance are
    /// available only through explicit renderer-local disclosure.
    pub fn details(&self) -> &PolicyChoiceDetails {
        &self.details
    }

    pub fn request_replan(
        &self,
        active_plan_id: &PlanId,
        policy_source: PolicySourceRevision,
        style_id: Option<StyleId>,
    ) -> Result<PolicyReplanRequest, PolicyExplanationError> {
        if active_plan_id != &self.details.plan_id {
            return Err(PolicyExplanationError::StaleReplanBasis);
        }
        if policy_source.source_id.as_str().is_empty() || policy_source.revision == 0 {
            return Err(PolicyExplanationError::InvalidPolicySource);
        }
        Ok(PolicyReplanRequest {
            prior_plan_id: active_plan_id.clone(),
            gear_id: self.details.gear_id.clone(),
            policy_source,
            style_id,
        })
    }

    pub fn candidate_text(candidate: &RealizationDecisionRecord) -> String {
        match &candidate.disposition {
            RealizationDecisionDisposition::Selected => {
                candidate.decisive_preference_clause.map_or_else(
                    || "selected: deterministic admissible choice".into(),
                    |clause| format!("selected: first preferred match at clause {}", clause + 1),
                )
            }
            RealizationDecisionDisposition::Admitted => {
                candidate.decisive_preference_clause.map_or_else(
                    || "admitted: deterministic tie-break".into(),
                    |clause| format!("admitted: lost at preference {}", clause + 1),
                )
            }
            RealizationDecisionDisposition::Rejected(rejection) => {
                format!("rejected: {}", rejection_text(rejection))
            }
        }
    }

    pub fn style_text(clause: &StylePreferenceEvidence) -> String {
        let status = match clause.outcome {
            StylePreferenceOutcome::Matched => "matched",
            StylePreferenceOutcome::Unmatched => "unmatched",
            StylePreferenceOutcome::Unavailable => "unavailable",
            StylePreferenceOutcome::Ranked => "ranked",
        };
        format!("{status}: {}", fact_label(&clause.fact))
    }
}

fn rejection_text(rejection: &RealizationRejection) -> String {
    match rejection {
        RealizationRejection::HardPredicate { fact, .. } => {
            format!("hard requirement failed for {}", fact_label(fact))
        }
        RealizationRejection::CurrentResourceObservation => {
            "current resource observation unavailable".into()
        }
        other => format!("{other:?}"),
    }
}

fn fact_label(fact: &PlannerFactRef) -> String {
    match fact {
        PlannerFactRef::RealizationCharacteristic(id) => {
            format!("REALIZATION {}", id.as_str())
        }
        PlannerFactRef::ResourceUnits(id)
        | PlannerFactRef::ComputeServiceGuarantee(id)
        | PlannerFactRef::ObservationUnreservedUnits(id)
        | PlannerFactRef::ObservationUtilizedUnits(id) => {
            format!("RESOURCE {}", id.as_str())
        }
        PlannerFactRef::ComputeHasPerformanceClass {
            resource_class_id, ..
        }
        | PlannerFactRef::ComputePerformanceClass {
            resource_class_id, ..
        }
        | PlannerFactRef::ComputeNominalClockHz {
            resource_class_id, ..
        } => format!("RESOURCE / TOPOLOGY {}", resource_class_id.as_str()),
        _ => format!("HARD REQUIREMENT {fact:?}"),
    }
}

fn friendly_identity(identity: &str) -> String {
    identity
        .rsplit('/')
        .next()
        .unwrap_or(identity)
        .split('@')
        .next()
        .unwrap_or(identity)
        .replace('-', " ")
}
