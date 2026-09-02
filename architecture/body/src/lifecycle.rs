use alloc::{vec, vec::Vec};
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayId, ActivePlayIdentity, CheckedFormId, Plan, PlanId,
    SignId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::identity::{bind_identity, validate_ids};
use crate::validation::{validate_new_sign, validate_plan_history, validate_sign};
use crate::{
    BodyId, BodyLifecycleEvent, BodyWorkset, BodyWorksetError, ResidentForm, SeedId, WakeId,
    WakeLifecycleEvent,
};

mod workload;

pub const MAX_BODY_SIGNS: usize = 16;
pub const MAX_WAKE_SIGNS: usize = 32;
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
    /// Exact current Form workload. Empty with revision zero is the explicit
    /// compatibility encoding for pre-workset single-Form Body evidence.
    #[serde(default)]
    pub workset: BodyWorkset,
    #[serde(default)]
    pub workload_revision: u64,
    pub birth_sequence: u64,
    pub state: BodyState,
    pub sign_ids: Vec<SignId>,
    pub events: Vec<BodyLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeLifecycle {
    AwaitingPlan,
    AwaitingPlay,
    Held,
    AwaitingReplacement,
    Playing,
    Unsatisfied,
    Lulled,
    Failed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakePlanState {
    AwaitingPlay,
    Held,
    Invalidated,
    Playing,
    Unsatisfied,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakePlan {
    pub plan_id: PlanId,
    pub active_play_id: Option<ActivePlayId>,
    pub state: WakePlanState,
    #[serde(default)]
    pub hold: Option<crate::PlanHold>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wake {
    pub wake_id: WakeId,
    pub body_id: BodyId,
    pub seed_id: SeedId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    #[serde(default)]
    pub workset: BodyWorkset,
    #[serde(default)]
    pub workload_revision: u64,
    pub wake_sequence: u64,
    pub lifecycle: WakeLifecycle,
    pub plans: Vec<WakePlan>,
    pub sign_ids: Vec<SignId>,
    pub events: Vec<WakeLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyLifecycleError {
    EmptyIdentity,
    IdentityTooLong,
    InvalidIdentity,
    InvalidTransition,
    DuplicateSign,
    SignCapacityExhausted,
    PlanCapacityExhausted,
    InvalidPlan,
    StalePlan,
    StalePlay,
    InvalidPlanningBasis,
    PlanningBasisCapacityExhausted,
    AuthorityDenied,
    HoldRequired,
    MismatchedWake,
    DuplicateForm,
    FormAbsent,
    FormCapacityExhausted,
    FormIdentityBytesExhausted,
}

impl core::fmt::Display for BodyLifecycleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid Body lifecycle transition: {self:?}")
    }
}

impl From<BodyWorksetError> for BodyLifecycleError {
    fn from(value: BodyWorksetError) -> Self {
        match value {
            BodyWorksetError::InvalidFormIdentity => Self::InvalidIdentity,
            BodyWorksetError::DuplicateForm => Self::DuplicateForm,
            BodyWorksetError::FormAbsent => Self::FormAbsent,
            BodyWorksetError::FormCapacityExhausted => Self::FormCapacityExhausted,
            BodyWorksetError::IdentityBytesExhausted => Self::FormIdentityBytesExhausted,
        }
    }
}

impl Body {
    pub fn born(
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        birth_sequence: u64,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[
            source_document_id.as_str(),
            checked_form_id.as_str(),
            sign_id.as_str(),
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
        let workset = BodyWorkset::seed(ResidentForm::new(
            source_document_id.clone(),
            checked_form_id.clone(),
        ))?;
        Ok(Self {
            body_id,
            seed_id,
            source_document_id,
            checked_form_id,
            workset,
            workload_revision: 1,
            birth_sequence,
            state: BodyState::Lulled,
            sign_ids: vec![sign_id.clone()],
            events: vec![BodyLifecycleEvent::Born { sign_id }],
        })
    }

    pub fn wake(
        &self,
        wake_sequence: u64,
        sign_id: SignId,
    ) -> Result<(Self, Wake), BodyLifecycleError> {
        self.validate()?;
        if self.state != BodyState::Lulled {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        validate_new_sign(&self.sign_ids, &sign_id, MAX_BODY_SIGNS)?;
        let wake_id = WakeId::bound(bind_identity(
            "wake",
            &[self.body_id.as_str()],
            wake_sequence,
        ));
        let mut body = self.clone();
        body.state = BodyState::Awake {
            wake_id: wake_id.clone(),
        };
        body.sign_ids.push(sign_id.clone());
        body.events.push(BodyLifecycleEvent::Woke {
            wake_id: wake_id.clone(),
            sign_id: sign_id.clone(),
        });
        let wake = Wake {
            wake_id,
            body_id: self.body_id.clone(),
            seed_id: self.seed_id.clone(),
            source_document_id: self.source_document_id.clone(),
            checked_form_id: self.checked_form_id.clone(),
            workset: self.effective_workset()?,
            workload_revision: self.workload_revision.max(1),
            wake_sequence,
            lifecycle: WakeLifecycle::AwaitingPlan,
            plans: Vec::new(),
            sign_ids: vec![sign_id],
            events: vec![WakeLifecycleEvent::Woke {
                sign_id: body.sign_ids.last().cloned().expect("wake sign is present"),
            }],
        };
        Ok((body, wake))
    }

    pub fn retain_after_lull(
        &self,
        wake: &Wake,
        sign_id: SignId,
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
        validate_new_sign(&self.sign_ids, &sign_id, MAX_BODY_SIGNS)?;
        let mut next = self.clone();
        next.state = BodyState::Lulled;
        next.sign_ids.push(sign_id.clone());
        next.events.push(BodyLifecycleEvent::LullRetained {
            wake_id: wake.wake_id.clone(),
            sign_id,
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
        validate_sign(&self.sign_ids, MAX_BODY_SIGNS)?;
        let effective_workset = self.effective_workset()?;
        crate::events::validate_body_events(
            &self.events,
            &self.sign_ids,
            &self.state,
            ResidentForm::new(
                self.source_document_id.clone(),
                self.checked_form_id.clone(),
            ),
            &effective_workset,
            self.workload_revision.max(1),
        )?;
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
            && self.effective_workset().ok().as_ref() == Some(&wake.workset)
            && self.workload_revision.max(1) == wake.workload_revision
            && matches!(&self.state, BodyState::Awake { wake_id } if wake_id == &wake.wake_id)
    }
}

impl Wake {
    pub fn plan_ready(&self, plan: &Plan, sign_id: SignId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        self.validate_plan(plan)?;
        self.accept_plan_id(&plan.plan_id, sign_id)
    }

    fn accept_plan_id(
        &self,
        plan_id: &PlanId,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        let prior = match self.lifecycle {
            WakeLifecycle::AwaitingPlan if self.plans.is_empty() => None,
            WakeLifecycle::Unsatisfied => self.plans.last().map(|p| &p.plan_id),
            WakeLifecycle::AwaitingReplacement => {
                let previous = self
                    .plans
                    .last()
                    .ok_or(BodyLifecycleError::InvalidTransition)?;
                if previous
                    .hold
                    .as_ref()
                    .is_some_and(|hold| hold.policy.hold_replacement_plan)
                {
                    return Err(BodyLifecycleError::HoldRequired);
                }
                Some(&previous.plan_id)
            }
            _ => return Err(BodyLifecycleError::InvalidTransition),
        };
        if prior == Some(plan_id) {
            return Err(BodyLifecycleError::StalePlan);
        }
        if self.plans.len() >= MAX_WAKE_PLANS {
            return Err(BodyLifecycleError::PlanCapacityExhausted);
        }
        let mut next = self.clone();
        let event = if let Some(prior_plan_id) = prior {
            WakeLifecycleEvent::Replanned {
                prior_plan_id: prior_plan_id.clone(),
                replacement_plan_id: plan_id.clone(),
                sign_id,
            }
        } else {
            WakeLifecycleEvent::PlanReady {
                plan_id: plan_id.clone(),
                sign_id,
            }
        };
        next.push_event(event)?;
        if let Some(previous) = next.plans.last_mut() {
            previous.state = WakePlanState::Superseded;
        }
        next.plans.push(WakePlan {
            plan_id: plan_id.clone(),
            active_play_id: None,
            state: WakePlanState::AwaitingPlay,
            hold: None,
        });
        next.lifecycle = WakeLifecycle::AwaitingPlay;
        Ok(next)
    }

    pub fn play_started(
        &self,
        play: &ActivePlayIdentity,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if self.lifecycle != WakeLifecycle::AwaitingPlay {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let expected = bind_active_play(
            &play.plan_id,
            &play.host_id,
            &play.boot_id,
            play.play_sequence,
        );
        if &expected != play || self.plans.last().map(|p| &p.plan_id) != Some(&play.plan_id) {
            return Err(BodyLifecycleError::StalePlay);
        }
        self.start_play_identity(&play.plan_id, &play.active_play_id, sign_id)
    }

    fn start_play_identity(
        &self,
        plan_id: &PlanId,
        active_play_id: &ActivePlayId,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        if self.lifecycle != WakeLifecycle::AwaitingPlay
            || self.plans.last().map(|plan| &plan.plan_id) != Some(plan_id)
        {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::PlayStarted {
            plan_id: plan_id.clone(),
            active_play_id: active_play_id.clone(),
            sign_id,
        })?;
        let current = next
            .plans
            .last_mut()
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        current.active_play_id = Some(active_play_id.clone());
        current.state = WakePlanState::Playing;
        next.lifecycle = WakeLifecycle::Playing;
        Ok(next)
    }

    pub fn became_unsatisfied(
        &self,
        plan_id: &PlanId,
        sign_id: SignId,
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
            sign_id,
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
        sign_id: SignId,
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
            sign_id,
        })?;
        Ok(next)
    }

    pub fn lull(&self, sign_id: SignId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Lulled { sign_id })?;
        next.lifecycle = WakeLifecycle::Lulled;
        Ok(next)
    }

    pub fn fail(&self, sign_id: SignId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Failed { sign_id })?;
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
        validate_sign(&self.sign_ids, MAX_WAKE_SIGNS)?;
        self.workset.validate()?;
        crate::events::validate_wake_events(
            &self.events,
            &self.sign_ids,
            self.lifecycle,
            &self.plans,
            &self.workset,
            self.workload_revision,
        )?;
        if self.seed_id != SeedId::bind(&self.source_document_id, &self.checked_form_id)
            || self.wake_id.as_str()
                != bind_identity("wake", &[self.body_id.as_str()], self.wake_sequence)
            || self.plans.len() > MAX_WAKE_PLANS
            || self.workload_revision == 0
        {
            return Err(BodyLifecycleError::InvalidIdentity);
        }
        validate_plan_history(self.lifecycle, &self.plans)
    }

    pub(crate) fn validate_plan(&self, plan: &Plan) -> Result<(), BodyLifecycleError> {
        if !verify_plan(plan) {
            return Err(BodyLifecycleError::InvalidPlan);
        }
        let form = ResidentForm::new(
            plan.source_document_id.clone(),
            plan.checked_form_id.clone(),
        );
        if self.workset.len() != 1 || !self.workset.contains(&form) {
            return Err(BodyLifecycleError::StalePlan);
        }
        Ok(())
    }
    pub(crate) fn push_event(
        &mut self,
        event: WakeLifecycleEvent,
    ) -> Result<(), BodyLifecycleError> {
        validate_new_sign(&self.sign_ids, event.sign_id(), MAX_WAKE_SIGNS)?;
        self.sign_ids.push(event.sign_id().clone());
        self.events.push(event);
        Ok(())
    }
}
