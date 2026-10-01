//! Bounded evidence for partial realization loss and ordinary replacement planning.

use crate::{
    CandidateEvaluationDisposition, CandidateStructure, IncrementalPlan, PlannerError,
    PlanningFactKey, MAXIMUM_INCREMENTAL_CANDIDATES, MAXIMUM_PLANNING_FACTS,
};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::PlanId;

pub const MAXIMUM_DEGRADATION_FRAGMENTS: usize = 32;
pub const MAXIMUM_DEGRADATION_FRAGMENT_ID_BYTES: usize = 256;
pub const MAXIMUM_DEGRADATION_REFUSAL_BYTES: usize = 512;
pub const MAXIMUM_DEGRADATION_EXPLANATION_BYTES: usize = 2_048;

/// Bounded, renderer-neutral explanation of scoped realization loss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationExplanation {
    pub previous_plan_id: String,
    pub replacement_plan_id: Option<String>,
    pub what_failed: Vec<String>,
    pub what_still_works: Vec<String>,
    pub what_changed: String,
    pub automatic_retry_count: u32,
}

impl DegradationExplanation {
    pub fn from_assessment(assessment: &DegradationAssessment) -> Result<Self, String> {
        if assessment.automatic_retry_count != 0 {
            return Err("degradation cannot be presented as an automatic retry".into());
        }
        let mut what_failed = Vec::new();
        let mut what_still_works = Vec::new();
        for fragment in &assessment.fragments {
            let dependencies = fragment
                .changed_dependencies
                .iter()
                .map(|fact| format!("{:?}:{}", fact.domain, fact.identity))
                .collect::<Vec<_>>()
                .join(",");
            match &fragment.disposition {
                DegradationFragmentDisposition::StillWorks => what_still_works.push(format!(
                    "{} still works as {}; unaffected structure reused={}",
                    fragment.fragment_id,
                    fragment.previous_candidate_id,
                    fragment.reused_unaffected_structure
                )),
                DegradationFragmentDisposition::Replaced { candidate_id } => {
                    what_failed.push(format!(
                        "{} lost [{}]; {} stopped; replacement={candidate_id}",
                        fragment.fragment_id, dependencies, fragment.previous_candidate_id
                    ));
                }
                DegradationFragmentDisposition::Refused { reason } => {
                    what_failed.push(format!(
                        "{} lost [{}]; {} stopped; refusal={reason}",
                        fragment.fragment_id, dependencies, fragment.previous_candidate_id
                    ));
                }
            }
        }
        if what_failed.is_empty() || what_still_works.is_empty() {
            return Err(
                "scoped-loss explanation requires failed and still-working fragments".into(),
            );
        }
        let replacement_plan_id = assessment
            .replacement_plan_id
            .as_ref()
            .map(|identity| identity.as_str().to_owned());
        let what_changed = replacement_plan_id.as_ref().map_or_else(
            || {
                format!(
                    "historical Plan {} remains immutable; no complete replacement Plan exists",
                    assessment.previous_plan_id.as_str()
                )
            },
            |replacement| {
                format!(
                    "historical Plan {} remains immutable; fresh ordinary plan={replacement}",
                    assessment.previous_plan_id.as_str()
                )
            },
        );
        let encoded_bytes = what_failed
            .iter()
            .chain(&what_still_works)
            .map(String::len)
            .sum::<usize>()
            .checked_add(what_changed.len())
            .ok_or_else(|| "degradation explanation length overflowed".to_owned())?;
        if encoded_bytes > MAXIMUM_DEGRADATION_EXPLANATION_BYTES {
            return Err("degradation explanation exceeds its finite bound".into());
        }
        Ok(Self {
            previous_plan_id: assessment.previous_plan_id.as_str().to_owned(),
            replacement_plan_id,
            what_failed,
            what_still_works,
            what_changed,
            automatic_retry_count: 0,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationInput {
    pub fragment_id: String,
    pub previous_candidate_id: String,
    pub candidates: Vec<CandidateStructure>,
    /// The result of fresh ordinary incremental planning when a replacement exists.
    pub fresh_plan: Option<IncrementalPlan>,
    /// A specific fresh-planning refusal when no candidate remains admissible.
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradationFragmentDisposition {
    StillWorks,
    Replaced { candidate_id: String },
    Refused { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationFragment {
    pub fragment_id: String,
    pub previous_candidate_id: String,
    pub changed_dependencies: Vec<PlanningFactKey>,
    pub disposition: DegradationFragmentDisposition,
    pub reused_unaffected_structure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradationAssessment {
    pub previous_plan_id: PlanId,
    pub replacement_plan_id: Option<PlanId>,
    pub fragments: Vec<DegradationFragment>,
    /// Partial loss never authorizes an implicit migration or retry.
    pub automatic_retry_count: u32,
}

impl DegradationAssessment {
    pub fn what_failed(&self) -> Vec<&DegradationFragment> {
        self.fragments
            .iter()
            .filter(|fragment| {
                !matches!(
                    fragment.disposition,
                    DegradationFragmentDisposition::StillWorks
                )
            })
            .collect()
    }

    pub fn what_still_works(&self) -> Vec<&DegradationFragment> {
        self.fragments
            .iter()
            .filter(|fragment| {
                matches!(
                    fragment.disposition,
                    DegradationFragmentDisposition::StillWorks
                )
            })
            .collect()
    }
}

pub fn assess_scoped_degradation(
    previous_plan_id: PlanId,
    replacement_plan_id: Option<PlanId>,
    changed_facts: &[PlanningFactKey],
    inputs: &[DegradationInput],
) -> Result<DegradationAssessment, PlannerError> {
    if previous_plan_id.as_str().is_empty() {
        return invalid("partial-loss assessment requires an exact historical Plan identity");
    }
    if inputs.is_empty() || inputs.len() > MAXIMUM_DEGRADATION_FRAGMENTS {
        return invalid("partial-loss fragment count is empty or exceeds its finite bound");
    }
    if changed_facts.is_empty() || changed_facts.len() > MAXIMUM_PLANNING_FACTS {
        return invalid("partial-loss changed fact count is empty or exceeds its finite bound");
    }
    let changed = changed_facts.iter().cloned().collect::<BTreeSet<_>>();
    if changed.len() != changed_facts.len() {
        return invalid("partial-loss changed facts contain duplicate exact identities");
    }

    let mut fragment_ids = BTreeSet::new();
    let mut fragments = Vec::with_capacity(inputs.len());
    let mut changed_any = false;
    for input in inputs {
        if input.fragment_id.is_empty()
            || input.fragment_id.len() > MAXIMUM_DEGRADATION_FRAGMENT_ID_BYTES
            || !fragment_ids.insert(input.fragment_id.as_str())
        {
            return invalid("partial-loss fragment identities are empty, oversized, or duplicated");
        }
        if input.candidates.is_empty() || input.candidates.len() > MAXIMUM_INCREMENTAL_CANDIDATES {
            return invalid("partial-loss candidate count is empty or exceeds its finite bound");
        }
        let previous = input
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == input.previous_candidate_id)
            .ok_or_else(|| {
                PlannerError::InvalidRealizationPolicy(
                    "partial-loss previous candidate is absent from its exact fragment".into(),
                )
            })?;
        let changed_dependencies = previous
            .dependencies
            .iter()
            .filter(|dependency| changed.contains(*dependency))
            .cloned()
            .collect::<Vec<_>>();

        let (disposition, reused_unaffected_structure) = match (&input.fresh_plan, &input.refusal) {
            (Some(_), Some(_)) | (None, None) => {
                return invalid(
                    "partial-loss fragment must carry exactly one fresh Plan result or refusal",
                )
            }
            (Some(plan), None) => {
                let evidence_ids = plan
                    .considered
                    .iter()
                    .map(|evidence| evidence.candidate_id.as_str())
                    .collect::<BTreeSet<_>>();
                let candidate_ids = input
                    .candidates
                    .iter()
                    .map(|candidate| candidate.candidate_id.as_str())
                    .collect::<BTreeSet<_>>();
                if evidence_ids.len() != plan.considered.len()
                    || candidate_ids.len() != input.candidates.len()
                    || evidence_ids != candidate_ids
                    || plan.considered.iter().any(|evidence| {
                        input.candidates.iter().all(|candidate| {
                            candidate.candidate_id != evidence.candidate_id
                                || candidate.placement_id != evidence.placement_id
                        })
                    })
                {
                    return invalid(
                        "fresh incremental evidence does not exactly cover fragment candidates",
                    );
                }
                let selected = input
                    .candidates
                    .iter()
                    .find(|candidate| candidate.candidate_id == plan.selected_candidate_id)
                    .ok_or_else(|| {
                        PlannerError::InvalidRealizationPolicy(
                            "fresh incremental result selected a candidate outside its fragment"
                                .into(),
                        )
                    })?;
                let selected_evidence = plan
                    .considered
                    .iter()
                    .find(|evidence| evidence.candidate_id == selected.candidate_id)
                    .ok_or_else(|| {
                        PlannerError::InvalidRealizationPolicy(
                            "fresh incremental result omitted selected candidate evidence".into(),
                        )
                    })?;
                if selected_evidence.evaluation.disposition
                    != CandidateEvaluationDisposition::Admitted
                {
                    return invalid("fresh incremental result selected a refused candidate");
                }
                if changed_dependencies.is_empty() {
                    if selected.candidate_id != previous.candidate_id || !selected_evidence.reused {
                        return invalid(
                            "unaffected fragment was replaced or reevaluated without dependency loss",
                        );
                    }
                    (DegradationFragmentDisposition::StillWorks, true)
                } else {
                    changed_any = true;
                    let previous_evidence = plan
                        .considered
                        .iter()
                        .find(|evidence| evidence.candidate_id == previous.candidate_id)
                        .expect("exact candidate/evidence sets were validated");
                    if previous_evidence.reused
                        || !matches!(
                            previous_evidence.evaluation.disposition,
                            CandidateEvaluationDisposition::Rejected(_)
                        )
                    {
                        return invalid(
                            "affected previous candidate was not freshly and specifically refused",
                        );
                    }
                    if selected.candidate_id == previous.candidate_id && selected_evidence.reused {
                        return invalid("affected fragment reused stale candidate truth");
                    }
                    (
                        DegradationFragmentDisposition::Replaced {
                            candidate_id: selected.candidate_id.clone(),
                        },
                        false,
                    )
                }
            }
            (None, Some(reason)) => {
                if changed_dependencies.is_empty() {
                    return invalid("unaffected fragment cannot become a loss refusal");
                }
                if reason.is_empty() || reason.len() > MAXIMUM_DEGRADATION_REFUSAL_BYTES {
                    return invalid("partial-loss refusal is empty or exceeds its finite bound");
                }
                changed_any = true;
                (
                    DegradationFragmentDisposition::Refused {
                        reason: reason.clone(),
                    },
                    false,
                )
            }
        };
        fragments.push(DegradationFragment {
            fragment_id: input.fragment_id.clone(),
            previous_candidate_id: input.previous_candidate_id.clone(),
            changed_dependencies,
            disposition,
            reused_unaffected_structure,
        });
    }

    if !changed_any {
        return invalid("changed facts do not affect any selected realization fragment");
    }
    match &replacement_plan_id {
        Some(replacement)
            if replacement.as_str().is_empty() || replacement == &previous_plan_id =>
        {
            return invalid("fresh replacement Plan identity must be exact and distinct")
        }
        Some(_) => {
            if fragments.iter().any(|fragment| {
                matches!(
                    fragment.disposition,
                    DegradationFragmentDisposition::Refused { .. }
                )
            }) {
                return invalid(
                    "a refused fragment cannot be represented as a complete replacement Plan",
                );
            }
        }
        None => {
            if !fragments.iter().any(|fragment| {
                matches!(
                    fragment.disposition,
                    DegradationFragmentDisposition::Refused { .. }
                )
            }) {
                return invalid("successful replacement requires a distinct fresh Plan identity");
            }
        }
    }

    Ok(DegradationAssessment {
        previous_plan_id,
        replacement_plan_id,
        fragments,
        automatic_retry_count: 0,
    })
}

fn invalid(message: &str) -> Result<DegradationAssessment, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(message.to_string()))
}
