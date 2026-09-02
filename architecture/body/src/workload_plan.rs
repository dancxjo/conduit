//! One immutable Body-wide Plan over the current exact Form workset.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{verify_plan, ActivePlayId, Plan, PlanId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{BodyId, BodyWorkset, ResidentForm, Wake, WakeId, MAX_BODY_FORMS};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyFormPlan {
    pub form: ResidentForm,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlan {
    pub plan_id: PlanId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub workload_revision: u64,
    pub workset: BodyWorkset,
    pub forms: Vec<BodyFormPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlayIdentity {
    pub active_play_id: ActivePlayId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub plan_id: PlanId,
    pub play_sequence: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyPlanError {
    EmptyWorkset,
    FormCapacityExceeded,
    InvalidPlan,
    DuplicateForm,
    MissingForm,
    UnexpectedForm,
    WrongBody,
    WrongWake,
    StaleWorkload,
    InvalidIdentity,
}

impl BodyPlan {
    pub fn seal(wake: &Wake, mut forms: Vec<BodyFormPlan>) -> Result<Self, BodyPlanError> {
        wake.workset
            .validate()
            .map_err(|_| BodyPlanError::InvalidIdentity)?;
        if wake.workset.is_empty() {
            return Err(BodyPlanError::EmptyWorkset);
        }
        if forms.len() > MAX_BODY_FORMS {
            return Err(BodyPlanError::FormCapacityExceeded);
        }
        forms.sort_by(|left, right| left.form.cmp(&right.form));
        if forms.windows(2).any(|pair| pair[0].form >= pair[1].form) {
            return Err(BodyPlanError::DuplicateForm);
        }
        for partition in &forms {
            if !wake.workset.contains(&partition.form) {
                return Err(BodyPlanError::UnexpectedForm);
            }
            if !verify_plan(&partition.plan)
                || partition.plan.source_document_id != partition.form.source_document_id
                || partition.plan.checked_form_id != partition.form.checked_form_id
            {
                return Err(BodyPlanError::InvalidPlan);
            }
        }
        if wake.workset.forms().iter().any(|form| {
            forms
                .binary_search_by(|value| value.form.cmp(form))
                .is_err()
        }) {
            return Err(BodyPlanError::MissingForm);
        }
        let plan_id = bind_body_plan(&wake.body_id, &wake.wake_id, wake.workload_revision, &forms);
        Ok(Self {
            plan_id,
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            workload_revision: wake.workload_revision,
            workset: wake.workset.clone(),
            forms,
        })
    }

    pub fn validate_for(&self, wake: &Wake) -> Result<(), BodyPlanError> {
        if self.body_id != wake.body_id {
            return Err(BodyPlanError::WrongBody);
        }
        if self.wake_id != wake.wake_id {
            return Err(BodyPlanError::WrongWake);
        }
        if self.workload_revision != wake.workload_revision || self.workset != wake.workset {
            return Err(BodyPlanError::StaleWorkload);
        }
        let resealed = Self::seal(wake, self.forms.clone())?;
        if resealed.plan_id != self.plan_id {
            return Err(BodyPlanError::InvalidIdentity);
        }
        Ok(())
    }
}

impl BodyPlayIdentity {
    pub fn bind(plan: &BodyPlan, play_sequence: u64) -> Self {
        Self {
            active_play_id: bind_body_play(
                &plan.body_id,
                &plan.wake_id,
                &plan.plan_id,
                play_sequence,
            ),
            body_id: plan.body_id.clone(),
            wake_id: plan.wake_id.clone(),
            plan_id: plan.plan_id.clone(),
            play_sequence,
        }
    }

    pub fn validate_for(&self, plan: &BodyPlan) -> bool {
        self.body_id == plan.body_id
            && self.wake_id == plan.wake_id
            && self.plan_id == plan.plan_id
            && self.active_play_id
                == bind_body_play(
                    &self.body_id,
                    &self.wake_id,
                    &self.plan_id,
                    self.play_sequence,
                )
    }
}

fn bind_body_plan(
    body_id: &BodyId,
    wake_id: &WakeId,
    workload_revision: u64,
    forms: &[BodyFormPlan],
) -> PlanId {
    let mut bytes = Vec::new();
    push(&mut bytes, "conduit.body/body-plan@1");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    bytes.extend_from_slice(&workload_revision.to_le_bytes());
    bytes.extend_from_slice(&(forms.len() as u32).to_le_bytes());
    for form in forms {
        push(&mut bytes, form.form.source_document_id.as_str());
        push(&mut bytes, form.form.checked_form_id.as_str());
        push(&mut bytes, form.plan.plan_id.as_str());
    }
    PlanId::from(digest_id("body-plan", &bytes))
}

fn bind_body_play(
    body_id: &BodyId,
    wake_id: &WakeId,
    plan_id: &PlanId,
    play_sequence: u64,
) -> ActivePlayId {
    let mut bytes = Vec::new();
    push(&mut bytes, "conduit.body/body-play@1");
    push(&mut bytes, body_id.as_str());
    push(&mut bytes, wake_id.as_str());
    push(&mut bytes, plan_id.as_str());
    bytes.extend_from_slice(&play_sequence.to_le_bytes());
    ActivePlayId::from(digest_id("body-play", &bytes))
}

fn push(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn digest_id(prefix: &str, bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{prefix}/sha256:{hex}")
}
