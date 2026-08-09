use alloc::{vec, vec::Vec};
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayId, ActivePlayIdentity, CheckedFormId, EvidenceId,
    Plan, PlanId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::identity::{bind_identity, validate_ids};
use crate::{BodyId, BodyLifecycleEvent, SeedId, WakeId, WakeLifecycleEvent};

pub const MAX_BODY_EVIDENCE: usize = 16;
pub const MAX_WAKE_EVIDENCE: usize = 32;
pub const MAX_WAKE_PLANS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyState {
    Lulled,
    Awake { wake_id: WakeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub body_id: BodyId,
    pub seed_id: SeedId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub birth_sequence: u64,
    pub state: BodyState,
    pub evidence_ids: Vec<EvidenceId>,
    pub events: Vec<BodyLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeLifecycle {
    AwaitingPlan,
    AwaitingPlay,
    Playing,
    Unsatisfied,
    Lulled,
    Failed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakePlanState {
    AwaitingPlay,
    Playing,
    Unsatisfied,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakePlan {
    pub plan_id: PlanId,
    pub active_play_id: Option<ActivePlayId>,
    pub state: WakePlanState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wake {
    pub wake_id: WakeId,
    pub body_id: BodyId,
    pub seed_id: SeedId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub wake_sequence: u64,
    pub lifecycle: WakeLifecycle,
    pub plans: Vec<WakePlan>,
    pub evidence_ids: Vec<EvidenceId>,
    pub events: Vec<WakeLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyLifecycleError {
    EmptyIdentity,
    IdentityTooLong,
    InvalidIdentity,
    InvalidTransition,
    DuplicateEvidence,
    EvidenceCapacityExhausted,
    PlanCapacityExhausted,
    InvalidPlan,
    StalePlan,
    StalePlay,
    MismatchedWake,
}

impl core::fmt::Display for BodyLifecycleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid Body lifecycle transition: {self:?}")
    }
}

impl Body {
    pub fn born(
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        birth_sequence: u64,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[
            source_document_id.as_str(),
            checked_form_id.as_str(),
            evidence_id.as_str(),
        ])?;
        let seed_id = SeedId::bind(&source_document_id, &checked_form_id);
        let body_id = BodyId::bound(bind_identity(
            "body",
            &[
                seed_id.as_str(),
                source_document_id.as_str(),
                checked_form_id.as_str(),
            ],
            birth_sequence,
        ));
        Ok(Self {
            body_id,
            seed_id,
            source_document_id,
            checked_form_id,
            birth_sequence,
            state: BodyState::Lulled,
            evidence_ids: vec![evidence_id.clone()],
            events: vec![BodyLifecycleEvent::Born { evidence_id }],
        })
    }

    pub fn wake(
        &self,
        wake_sequence: u64,
        evidence_id: EvidenceId,
    ) -> Result<(Self, Wake), BodyLifecycleError> {
        self.validate()?;
        if self.state != BodyState::Lulled {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        validate_new_evidence(&self.evidence_ids, &evidence_id, MAX_BODY_EVIDENCE)?;
        let wake_id = WakeId::bound(bind_identity(
            "wake",
            &[self.body_id.as_str()],
            wake_sequence,
        ));
        let mut body = self.clone();
        body.state = BodyState::Awake {
            wake_id: wake_id.clone(),
        };
        body.evidence_ids.push(evidence_id.clone());
        body.events.push(BodyLifecycleEvent::Woke {
            wake_id: wake_id.clone(),
            evidence_id: evidence_id.clone(),
        });
        let wake = Wake {
            wake_id,
            body_id: self.body_id.clone(),
            seed_id: self.seed_id.clone(),
            source_document_id: self.source_document_id.clone(),
            checked_form_id: self.checked_form_id.clone(),
            wake_sequence,
            lifecycle: WakeLifecycle::AwaitingPlan,
            plans: Vec::new(),
            evidence_ids: vec![evidence_id],
            events: vec![WakeLifecycleEvent::Woke {
                evidence_id: body
                    .evidence_ids
                    .last()
                    .cloned()
                    .expect("wake evidence is present"),
            }],
        };
        Ok((body, wake))
    }

    pub fn retain_after_lull(
        &self,
        wake: &Wake,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        wake.validate()?;
        if !self.matches(wake)
            || !matches!(
                wake.lifecycle,
                WakeLifecycle::Lulled | WakeLifecycle::Failed
            )
        {
            return Err(BodyLifecycleError::MismatchedWake);
        }
        validate_new_evidence(&self.evidence_ids, &evidence_id, MAX_BODY_EVIDENCE)?;
        let mut next = self.clone();
        next.state = BodyState::Lulled;
        next.evidence_ids.push(evidence_id.clone());
        next.events.push(BodyLifecycleEvent::LullRetained {
            wake_id: wake.wake_id.clone(),
            evidence_id,
        });
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), BodyLifecycleError> {
        validate_ids(&[
            self.body_id.as_str(),
            self.seed_id.as_str(),
            self.source_document_id.as_str(),
            self.checked_form_id.as_str(),
        ])?;
        validate_evidence(&self.evidence_ids, MAX_BODY_EVIDENCE)?;
        crate::events::validate_body_events(&self.events, &self.evidence_ids, &self.state)?;
        if self.seed_id != SeedId::bind(&self.source_document_id, &self.checked_form_id)
            || self.body_id.as_str()
                != bind_identity(
                    "body",
                    &[
                        self.seed_id.as_str(),
                        self.source_document_id.as_str(),
                        self.checked_form_id.as_str(),
                    ],
                    self.birth_sequence,
                )
        {
            return Err(BodyLifecycleError::InvalidIdentity);
        }
        Ok(())
    }

    fn matches(&self, wake: &Wake) -> bool {
        self.body_id == wake.body_id
            && self.seed_id == wake.seed_id
            && self.source_document_id == wake.source_document_id
            && self.checked_form_id == wake.checked_form_id
            && matches!(&self.state, BodyState::Awake { wake_id } if wake_id == &wake.wake_id)
    }
}

impl Wake {
    pub fn plan_ready(
        &self,
        plan: &Plan,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        self.validate_plan(plan)?;
        let prior = match self.lifecycle {
            WakeLifecycle::AwaitingPlan if self.plans.is_empty() => None,
            WakeLifecycle::Unsatisfied => self.plans.last().map(|p| &p.plan_id),
            _ => return Err(BodyLifecycleError::InvalidTransition),
        };
        if prior == Some(&plan.plan_id) {
            return Err(BodyLifecycleError::StalePlan);
        }
        if self.plans.len() >= MAX_WAKE_PLANS {
            return Err(BodyLifecycleError::PlanCapacityExhausted);
        }
        let mut next = self.clone();
        let event = if let Some(prior_plan_id) = prior {
            WakeLifecycleEvent::Replanned {
                prior_plan_id: prior_plan_id.clone(),
                replacement_plan_id: plan.plan_id.clone(),
                evidence_id,
            }
        } else {
            WakeLifecycleEvent::PlanReady {
                plan_id: plan.plan_id.clone(),
                evidence_id,
            }
        };
        next.push_event(event)?;
        if let Some(previous) = next.plans.last_mut() {
            previous.state = WakePlanState::Superseded;
        }
        next.plans.push(WakePlan {
            plan_id: plan.plan_id.clone(),
            active_play_id: None,
            state: WakePlanState::AwaitingPlay,
        });
        next.lifecycle = WakeLifecycle::AwaitingPlay;
        Ok(next)
    }

    pub fn play_started(
        &self,
        play: &ActivePlayIdentity,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if self.lifecycle != WakeLifecycle::AwaitingPlay {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let expected = bind_active_play(
            &play.plan_id,
            &play.host_id,
            &play.boot_id,
            play.activation_sequence,
        );
        if &expected != play || self.plans.last().map(|p| &p.plan_id) != Some(&play.plan_id) {
            return Err(BodyLifecycleError::StalePlay);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::PlayStarted {
            plan_id: play.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            evidence_id,
        })?;
        let current = next
            .plans
            .last_mut()
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        current.active_play_id = Some(play.active_play_id.clone());
        current.state = WakePlanState::Playing;
        next.lifecycle = WakeLifecycle::Playing;
        Ok(next)
    }

    pub fn became_unsatisfied(
        &self,
        plan_id: &PlanId,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if self.lifecycle != WakeLifecycle::Playing
            || self.plans.last().map(|p| &p.plan_id) != Some(plan_id)
        {
            return Err(BodyLifecycleError::StalePlan);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::BecameUnsatisfied {
            plan_id: plan_id.clone(),
            evidence_id,
        })?;
        next.plans
            .last_mut()
            .ok_or(BodyLifecycleError::InvalidTransition)?
            .state = WakePlanState::Unsatisfied;
        next.lifecycle = WakeLifecycle::Unsatisfied;
        Ok(next)
    }

    pub fn same_plan_observed(
        &self,
        plan_id: &PlanId,
        evidence_id: EvidenceId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if self.lifecycle != WakeLifecycle::Playing
            || self.plans.last().map(|p| &p.plan_id) != Some(plan_id)
        {
            return Err(BodyLifecycleError::StalePlan);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::SamePlanObserved {
            plan_id: plan_id.clone(),
            evidence_id,
        })?;
        Ok(next)
    }

    pub fn lull(&self, evidence_id: EvidenceId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Lulled { evidence_id })?;
        next.lifecycle = WakeLifecycle::Lulled;
        Ok(next)
    }

    pub fn fail(&self, evidence_id: EvidenceId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Failed { evidence_id })?;
        next.lifecycle = WakeLifecycle::Failed;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), BodyLifecycleError> {
        validate_ids(&[
            self.wake_id.as_str(),
            self.body_id.as_str(),
            self.seed_id.as_str(),
            self.source_document_id.as_str(),
            self.checked_form_id.as_str(),
        ])?;
        validate_evidence(&self.evidence_ids, MAX_WAKE_EVIDENCE)?;
        crate::events::validate_wake_events(
            &self.events,
            &self.evidence_ids,
            self.lifecycle,
            &self.plans,
        )?;
        if self.seed_id != SeedId::bind(&self.source_document_id, &self.checked_form_id)
            || self.wake_id.as_str()
                != bind_identity("wake", &[self.body_id.as_str()], self.wake_sequence)
            || self.plans.len() > MAX_WAKE_PLANS
        {
            return Err(BodyLifecycleError::InvalidIdentity);
        }
        validate_plan_history(self.lifecycle, &self.plans)
    }

    fn validate_plan(&self, plan: &Plan) -> Result<(), BodyLifecycleError> {
        if !verify_plan(plan) {
            return Err(BodyLifecycleError::InvalidPlan);
        }
        if plan.source_document_id != self.source_document_id
            || plan.checked_form_id != self.checked_form_id
        {
            return Err(BodyLifecycleError::StalePlan);
        }
        Ok(())
    }
    fn push_event(&mut self, event: WakeLifecycleEvent) -> Result<(), BodyLifecycleError> {
        validate_new_evidence(&self.evidence_ids, event.evidence_id(), MAX_WAKE_EVIDENCE)?;
        self.evidence_ids.push(event.evidence_id().clone());
        self.events.push(event);
        Ok(())
    }
}

fn validate_evidence(values: &[EvidenceId], capacity: usize) -> Result<(), BodyLifecycleError> {
    if values.is_empty() || values.len() > capacity {
        return Err(BodyLifecycleError::EvidenceCapacityExhausted);
    }
    for (index, value) in values.iter().enumerate() {
        validate_ids(&[value.as_str()])?;
        if values[..index].contains(value) {
            return Err(BodyLifecycleError::DuplicateEvidence);
        }
    }
    Ok(())
}
fn validate_new_evidence(
    values: &[EvidenceId],
    value: &EvidenceId,
    capacity: usize,
) -> Result<(), BodyLifecycleError> {
    validate_ids(&[value.as_str()])?;
    if values.contains(value) {
        return Err(BodyLifecycleError::DuplicateEvidence);
    }
    if values.len() >= capacity {
        return Err(BodyLifecycleError::EvidenceCapacityExhausted);
    }
    Ok(())
}
fn validate_plan_history(
    lifecycle: WakeLifecycle,
    plans: &[WakePlan],
) -> Result<(), BodyLifecycleError> {
    for plan in plans {
        validate_ids(&[plan.plan_id.as_str()])?;
        if let Some(play) = &plan.active_play_id {
            validate_ids(&[play.as_str()])?;
        }
    }
    if plans
        .iter()
        .enumerate()
        .any(|(i, p)| plans[..i].iter().any(|q| q.plan_id == p.plan_id))
        || plans
            .iter()
            .enumerate()
            .any(|(i, p)| i + 1 < plans.len() && p.state != WakePlanState::Superseded)
    {
        return Err(BodyLifecycleError::InvalidTransition);
    }
    let current = plans.last();
    let valid = match lifecycle {
        WakeLifecycle::AwaitingPlan => plans.is_empty(),
        WakeLifecycle::AwaitingPlay => current
            .is_some_and(|p| p.state == WakePlanState::AwaitingPlay && p.active_play_id.is_none()),
        WakeLifecycle::Playing => {
            current.is_some_and(|p| p.state == WakePlanState::Playing && p.active_play_id.is_some())
        }
        WakeLifecycle::Unsatisfied => current
            .is_some_and(|p| p.state == WakePlanState::Unsatisfied && p.active_play_id.is_some()),
        WakeLifecycle::Lulled | WakeLifecycle::Failed => true,
    };
    valid
        .then_some(())
        .ok_or(BodyLifecycleError::InvalidTransition)
}
