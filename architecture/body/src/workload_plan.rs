//! One immutable Body-wide Plan over the current exact Form workset.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{verify_plan, ActivePlayId, PlacementId, Plan, PlanId};
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
    /// Exact Presenter realizations selected independently of authored Forms.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presenter_topologies: Vec<BodyPresenterTopology>,
}

pub const MAX_BODY_PRESENTER_TOPOLOGIES: usize = 16;
pub const MAX_BODY_PRESENTER_CHAINS: usize = 8;
pub const MAX_BODY_PRESENTER_STAGES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyPresentationSelector {
    /// The exact resident Form when the Presentation is Form-scoped. `None`
    /// selects the Body-scoped Presentation without inventing a source Form.
    pub form: Option<ResidentForm>,
    pub source_placement_id: PlacementId,
}

/// One independently admitted linear chain. `plan` is an ordinary immutable
/// Plan over reviewed realization Forms; `stage_placement_ids` fixes its path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPresenterChainPlan {
    pub plan: Plan,
    pub stage_placement_ids: Vec<PlacementId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPresenterTopology {
    pub presentation: BodyPresentationSelector,
    pub chains: Vec<BodyPresenterChainPlan>,
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
    PresenterTopologyCapacityExceeded,
    PresenterChainCapacityExceeded,
    InvalidPresenterChain,
    DuplicatePresenterTopology,
}

impl BodyPlan {
    pub fn seal(wake: &Wake, forms: Vec<BodyFormPlan>) -> Result<Self, BodyPlanError> {
        Self::seal_with_presenters(wake, forms, Vec::new())
    }

    pub fn seal_with_presenters(
        wake: &Wake,
        mut forms: Vec<BodyFormPlan>,
        mut presenter_topologies: Vec<BodyPresenterTopology>,
    ) -> Result<Self, BodyPlanError> {
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
        validate_presenter_topologies(wake, &presenter_topologies)?;
        presenter_topologies.sort_by(|left, right| left.presentation.cmp(&right.presentation));
        let plan_id = bind_body_plan(
            &wake.body_id,
            &wake.wake_id,
            wake.workload_revision,
            &forms,
            &presenter_topologies,
        );
        Ok(Self {
            plan_id,
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            workload_revision: wake.workload_revision,
            workset: wake.workset.clone(),
            forms,
            presenter_topologies,
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
        let resealed = Self::seal_with_presenters(
            wake,
            self.forms.clone(),
            self.presenter_topologies.clone(),
        )?;
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
    presenter_topologies: &[BodyPresenterTopology],
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
    bytes.extend_from_slice(&(presenter_topologies.len() as u32).to_le_bytes());
    for topology in presenter_topologies {
        if let Some(form) = &topology.presentation.form {
            push(&mut bytes, "form");
            push(&mut bytes, form.source_document_id.as_str());
            push(&mut bytes, form.checked_form_id.as_str());
        } else {
            push(&mut bytes, "body");
        }
        push(
            &mut bytes,
            topology.presentation.source_placement_id.as_str(),
        );
        bytes.extend_from_slice(&(topology.chains.len() as u32).to_le_bytes());
        for chain in &topology.chains {
            push(&mut bytes, chain.plan.plan_id.as_str());
            bytes.extend_from_slice(&(chain.stage_placement_ids.len() as u32).to_le_bytes());
            for placement in &chain.stage_placement_ids {
                push(&mut bytes, placement.as_str());
            }
        }
    }
    PlanId::from(digest_id("body-plan", &bytes))
}

fn validate_presenter_topologies(
    wake: &Wake,
    topologies: &[BodyPresenterTopology],
) -> Result<(), BodyPlanError> {
    if topologies.len() > MAX_BODY_PRESENTER_TOPOLOGIES {
        return Err(BodyPlanError::PresenterTopologyCapacityExceeded);
    }
    for (index, topology) in topologies.iter().enumerate() {
        if topology
            .presentation
            .form
            .as_ref()
            .is_some_and(|form| !wake.workset.contains(form))
            || topology
                .presentation
                .source_placement_id
                .as_str()
                .is_empty()
            || topology.chains.is_empty()
            || topology.chains.len() > MAX_BODY_PRESENTER_CHAINS
        {
            return Err(BodyPlanError::InvalidPresenterChain);
        }
        if topologies[index + 1..]
            .iter()
            .any(|other| other.presentation == topology.presentation)
        {
            return Err(BodyPlanError::DuplicatePresenterTopology);
        }
        for chain in &topology.chains {
            if !verify_plan(&chain.plan)
                || chain.stage_placement_ids.is_empty()
                || chain.stage_placement_ids.len() > MAX_BODY_PRESENTER_STAGES
            {
                return Err(BodyPlanError::InvalidPresenterChain);
            }
            let placements = chain
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .collect::<Vec<_>>();
            for (stage_index, stage_id) in chain.stage_placement_ids.iter().enumerate() {
                if chain.stage_placement_ids[..stage_index].contains(stage_id)
                    || placements
                        .iter()
                        .filter(|placement| &placement.placement_id == stage_id)
                        .count()
                        != 1
                {
                    return Err(BodyPlanError::InvalidPresenterChain);
                }
                if let Some(next_id) = chain.stage_placement_ids.get(stage_index + 1) {
                    let connected = chain
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|f| &f.connections)
                        .any(|cord| {
                            &cord.source_placement_id == stage_id
                                && &cord.sink_placement_id == next_id
                        });
                    if !connected {
                        return Err(BodyPlanError::InvalidPresenterChain);
                    }
                }
            }
        }
    }
    Ok(())
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
