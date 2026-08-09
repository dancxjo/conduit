use alloc::{vec, vec::Vec};
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayId, ActivePlayIdentity, CheckedFormId, ClueId, Plan,
    PlanId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::identity::{bind_identity, validate_ids};
use crate::validation::{validate_clue, validate_new_clue, validate_plan_history};
use crate::{BodyId, BodyLifecycleEvent, SeedId, WakeId, WakeLifecycleEvent};

pub const MAX_BODY_CLUES: usize = 16;
pub const MAX_WAKE_CLUES: usize = 32;
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
    pub clue_ids: Vec<ClueId>,
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
    pub wake_sequence: u64,
    pub lifecycle: WakeLifecycle,
    pub plans: Vec<WakePlan>,
    pub clue_ids: Vec<ClueId>,
    pub events: Vec<WakeLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyLifecycleError {
    EmptyIdentity,
    IdentityTooLong,
    InvalidIdentity,
    InvalidTransition,
    DuplicateClue,
    ClueCapacityExhausted,
    PlanCapacityExhausted,
    InvalidPlan,
    StalePlan,
    StalePlay,
    InvalidPlanningBasis,
    PlanningBasisCapacityExhausted,
    AuthorityDenied,
    HoldRequired,
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
        clue_id: ClueId,
    ) -> Result<Self, BodyLifecycleError> {
        validate_ids(&[
            source_document_id.as_str(),
            checked_form_id.as_str(),
            clue_id.as_str(),
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
            clue_ids: vec![clue_id.clone()],
            events: vec![BodyLifecycleEvent::Born { clue_id }],
        })
    }

    pub fn wake(
        &self,
        wake_sequence: u64,
        clue_id: ClueId,
    ) -> Result<(Self, Wake), BodyLifecycleError> {
        self.validate()?;
        if self.state != BodyState::Lulled {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        validate_new_clue(&self.clue_ids, &clue_id, MAX_BODY_CLUES)?;
        let wake_id = WakeId::bound(bind_identity(
            "wake",
            &[self.body_id.as_str()],
            wake_sequence,
        ));
        let mut body = self.clone();
        body.state = BodyState::Awake {
            wake_id: wake_id.clone(),
        };
        body.clue_ids.push(clue_id.clone());
        body.events.push(BodyLifecycleEvent::Woke {
            wake_id: wake_id.clone(),
            clue_id: clue_id.clone(),
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
            clue_ids: vec![clue_id],
            events: vec![WakeLifecycleEvent::Woke {
                clue_id: body.clue_ids.last().cloned().expect("wake clue is present"),
            }],
        };
        Ok((body, wake))
    }

    pub fn retain_after_lull(
        &self,
        wake: &Wake,
        clue_id: ClueId,
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
        validate_new_clue(&self.clue_ids, &clue_id, MAX_BODY_CLUES)?;
        let mut next = self.clone();
        next.state = BodyState::Lulled;
        next.clue_ids.push(clue_id.clone());
        next.events.push(BodyLifecycleEvent::LullRetained {
            wake_id: wake.wake_id.clone(),
            clue_id,
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
        validate_clue(&self.clue_ids, MAX_BODY_CLUES)?;
        crate::events::validate_body_events(&self.events, &self.clue_ids, &self.state)?;
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
    pub fn plan_ready(&self, plan: &Plan, clue_id: ClueId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        self.validate_plan(plan)?;
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
                clue_id,
            }
        } else {
            WakeLifecycleEvent::PlanReady {
                plan_id: plan.plan_id.clone(),
                clue_id,
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
            hold: None,
        });
        next.lifecycle = WakeLifecycle::AwaitingPlay;
        Ok(next)
    }

    pub fn play_started(
        &self,
        play: &ActivePlayIdentity,
        clue_id: ClueId,
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
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::PlayStarted {
            plan_id: play.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            clue_id,
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
        clue_id: ClueId,
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
            clue_id,
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
        clue_id: ClueId,
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
            clue_id,
        })?;
        Ok(next)
    }

    pub fn lull(&self, clue_id: ClueId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Lulled { clue_id })?;
        next.lifecycle = WakeLifecycle::Lulled;
        Ok(next)
    }

    pub fn fail(&self, clue_id: ClueId) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            WakeLifecycle::Lulled | WakeLifecycle::Failed
        ) {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::Failed { clue_id })?;
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
        validate_clue(&self.clue_ids, MAX_WAKE_CLUES)?;
        crate::events::validate_wake_events(
            &self.events,
            &self.clue_ids,
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

    pub(crate) fn validate_plan(&self, plan: &Plan) -> Result<(), BodyLifecycleError> {
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
    pub(crate) fn push_event(
        &mut self,
        event: WakeLifecycleEvent,
    ) -> Result<(), BodyLifecycleError> {
        validate_new_clue(&self.clue_ids, event.clue_id(), MAX_WAKE_CLUES)?;
        self.clue_ids.push(event.clue_id().clone());
        self.events.push(event);
        Ok(())
    }
}
