//! Human-readable Patchbay truth for reviewed degraded-profile admission.

use conduit_core::{verify_plan, Plan};
use conduit_planner::{
    DegradedProfileRefusal, PlannerFactValue, ServiceProfileAdmission, ServiceProfileDisposition,
};
use serde::{Deserialize, Serialize};

pub const MAX_DEGRADED_PROFILE_EXPLANATION_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradedProfileState {
    Full,
    Degraded,
    Unrealizable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDimensionExplanation {
    pub characteristic_id: String,
    pub human_name: String,
    pub requested: String,
    pub weakest_permitted: String,
    pub surviving: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedProfileExplanation {
    pub state: DegradedProfileState,
    pub profile_id: String,
    pub previous_plan_id: Option<String>,
    pub plan_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub implementation_id: String,
    pub policy_id: Option<String>,
    pub policy_revision: Option<u64>,
    pub dimensions: Vec<ProfileDimensionExplanation>,
    pub observation_signs: Vec<String>,
    pub hard_requirements_relaxed: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradedProfileExplanationError {
    InvalidPlan,
    PlacementMismatch,
    MissingReplacementPlan,
    FormChanged,
    PlanReused,
    MissingPolicy,
    EvidenceTooLarge,
}

pub fn explain_degraded_profile(
    previous_plan: Option<&Plan>,
    plan: &Plan,
    admission: &ServiceProfileAdmission,
) -> Result<DegradedProfileExplanation, DegradedProfileExplanationError> {
    if !verify_plan(plan) || previous_plan.is_some_and(|prior| !verify_plan(prior)) {
        return Err(DegradedProfileExplanationError::InvalidPlan);
    }
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| {
            placement.host_id == admission.choice.host_id
                && placement.capability_id == admission.choice.capability_id
        })
        .ok_or(DegradedProfileExplanationError::PlacementMismatch)?;
    if admission.disposition == ServiceProfileDisposition::Degraded {
        let prior = previous_plan.ok_or(DegradedProfileExplanationError::MissingReplacementPlan)?;
        if prior.source_document_id != plan.source_document_id
            || prior.checked_form_id != plan.checked_form_id
            || prior.expanded_form_id != plan.expanded_form_id
        {
            return Err(DegradedProfileExplanationError::FormChanged);
        }
        if prior.plan_id == plan.plan_id {
            return Err(DegradedProfileExplanationError::PlanReused);
        }
        if admission.policy_id.is_none() || admission.policy_revision.is_none() {
            return Err(DegradedProfileExplanationError::MissingPolicy);
        }
    }
    let dimensions = admission
        .dimensions
        .iter()
        .map(|dimension| ProfileDimensionExplanation {
            characteristic_id: dimension.characteristic_id.as_str().into(),
            human_name: dimension.human_name.clone(),
            requested: fact_text(&dimension.requested_value),
            weakest_permitted: fact_text(&dimension.weakest_permitted_value),
            surviving: fact_text(&dimension.admitted_value),
        })
        .collect::<Vec<_>>();
    let state = match admission.disposition {
        ServiceProfileDisposition::Full => DegradedProfileState::Full,
        ServiceProfileDisposition::Degraded => DegradedProfileState::Degraded,
    };
    let summary = format!(
        "Profile {} is {:?}. Requested [{}]; surviving [{}]. Plan={} Host={} Boot={} implementation={}; admitted by {}. Hard requirements were not relaxed.",
        admission.profile_id,
        state,
        dimensions.iter().map(|item| format!("{}={}", item.human_name, item.requested)).collect::<Vec<_>>().join(", "),
        dimensions.iter().map(|item| format!("{}={}", item.human_name, item.surviving)).collect::<Vec<_>>().join(", "),
        plan.plan_id.as_str(),
        placement.host_id.as_str(),
        placement.boot_id.as_str(),
        placement.implementation_id.as_str(),
        admission.policy_id.as_deref().unwrap_or("full compatibility"),
    );
    if summary.len() > MAX_DEGRADED_PROFILE_EXPLANATION_BYTES {
        return Err(DegradedProfileExplanationError::EvidenceTooLarge);
    }
    Ok(DegradedProfileExplanation {
        state,
        profile_id: admission.profile_id.clone(),
        previous_plan_id: previous_plan.map(|prior| prior.plan_id.as_str().into()),
        plan_id: plan.plan_id.as_str().into(),
        host_id: placement.host_id.as_str().into(),
        boot_id: placement.boot_id.as_str().into(),
        implementation_id: placement.implementation_id.as_str().into(),
        policy_id: admission.policy_id.clone(),
        policy_revision: admission.policy_revision,
        dimensions,
        observation_signs: admission
            .observation_signs
            .iter()
            .map(|sign| sign.as_str().into())
            .collect(),
        hard_requirements_relaxed: false,
        summary,
    })
}

pub fn explain_degraded_profile_refusal(
    refusal: &DegradedProfileRefusal,
) -> (&'static str, DegradedProfileState) {
    let text = match refusal {
        DegradedProfileRefusal::DegradationForbidden => "degradation forbidden by policy",
        DegradedProfileRefusal::PolicyOutsideReviewedBounds => "policy exceeds reviewed bounds",
        DegradedProfileRefusal::HardRequirementUnsatisfied => "hard requirement unsatisfied",
        DegradedProfileRefusal::MissingRequiredEvidence => "required evidence class unavailable",
        DegradedProfileRefusal::SemanticallyDifferentDimension => {
            "quality dimension is semantically incompatible"
        }
        DegradedProfileRefusal::StaleOrMissingObservation => {
            "current quality observation unavailable"
        }
        DegradedProfileRefusal::NoMeaningfulWeakerProfile => "no meaningful weaker profile exists",
        DegradedProfileRefusal::Unrealizable => {
            "no realization satisfies the weakest permitted profile"
        }
        _ => "invalid degraded-profile admission",
    };
    (text, DegradedProfileState::Unrealizable)
}

fn fact_text(value: &PlannerFactValue) -> String {
    match value {
        PlannerFactValue::Boolean(value) => value.to_string(),
        PlannerFactValue::Quantity { value, unit } => format!("{value} {unit:?}"),
        PlannerFactValue::Category(value) => value.clone(),
        PlannerFactValue::ServiceGuarantee(value) => format!("{value:?}"),
    }
}
