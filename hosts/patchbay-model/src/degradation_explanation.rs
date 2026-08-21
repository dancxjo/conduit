//! Renderer-neutral Patchbay explanation of scoped realization loss.

use conduit_planner::{DegradationAssessment, DegradationFragmentDisposition};
use serde::{Deserialize, Serialize};

pub const MAX_DEGRADATION_EXPLANATION_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayDegradationExplanation {
    pub previous_plan_id: String,
    pub replacement_plan_id: Option<String>,
    pub what_failed: Vec<String>,
    pub what_still_works: Vec<String>,
    pub what_changed: String,
    pub automatic_retry_count: u32,
}

impl PatchbayDegradationExplanation {
    pub fn from_assessment(assessment: &DegradationAssessment) -> Result<Self, String> {
        if assessment.automatic_retry_count != 0 {
            return Err("Patchbay cannot present partial loss as an automatic retry".into());
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
                "Patchbay scoped-loss explanation requires failed and still-working fragments"
                    .into(),
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
                    "historical Plan {} remains immutable; fresh ordinary Plan={replacement}",
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
            .ok_or_else(|| "Patchbay degradation explanation length overflowed".to_owned())?;
        if encoded_bytes > MAX_DEGRADATION_EXPLANATION_BYTES {
            return Err("Patchbay degradation explanation exceeds its finite bound".into());
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
