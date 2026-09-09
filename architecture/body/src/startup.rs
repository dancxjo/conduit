//! Startup eligibility derived from ordinary retained Body lifecycle evidence.
//!
//! This is a projection, not a second once flag or permission to start execution.
//! The first Play in a Wake is its startup realization. Replacing its Plan does
//! not replay startup; a later Wake has its own first Play. A Host must retain
//! the started lifecycle before exposing startup input to effects and must own
//! the exact current Play through the ordinary admission boundary.

use conduit_core::{ActivePlayId, SignId};

use crate::{
    BodyBiographyEvidence, BodyId, BodyLifecycleEvent, BodyPlan, BodyPlayIdentity, BodyState,
    WakeId, WakeLifecycle, WakeLifecycleEvent, WakePlanState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupScope {
    /// Once in each Wake, in its first started Play.
    Wake,
    /// Once in the Body's first Wake, in that Wake's first started Play.
    Body,
}

/// Exact causal evidence for a startup input. Identity is not delivery authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyStartup {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub active_play_id: ActivePlayId,
    pub wake_sign_id: SignId,
    pub play_start_sign_id: SignId,
    first_body_wake: bool,
    first_play_in_wake: bool,
}

impl BodyStartup {
    pub fn eligible(&self, scope: StartupScope) -> bool {
        self.first_play_in_wake && (scope == StartupScope::Wake || self.first_body_wake)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyStartupRefusal {
    InvalidEvidence,
    NotAwake,
    StalePlan,
    StalePlay,
    MissingStartupEvidence,
}

impl BodyBiographyEvidence {
    /// Derive eligibility from retained Woke/PlayStarted records, including the
    /// records preceding the current Plan. The query neither records consumption
    /// nor claims an effect occurred. The installed source emits at most once
    /// inside the admitted Play; old Plays cannot be revived from this value.
    pub fn startup_for_play(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<BodyStartup, BodyStartupRefusal> {
        self.validate()
            .map_err(|_| BodyStartupRefusal::InvalidEvidence)?;
        let BodyState::Awake { wake_id } = &self.body.state else {
            return Err(BodyStartupRefusal::NotAwake);
        };
        let wake = self
            .wakes
            .iter()
            .find(|wake| &wake.wake_id == wake_id)
            .ok_or(BodyStartupRefusal::MissingStartupEvidence)?;
        plan.validate_for(wake)
            .map_err(|_| BodyStartupRefusal::StalePlan)?;
        if wake.lifecycle != WakeLifecycle::Playing
            || !play.validate_for(plan)
            || !wake.plans.iter().any(|candidate| {
                candidate.plan_id == plan.plan_id
                    && candidate.state == WakePlanState::Playing
                    && candidate.active_play_id.as_ref() == Some(&play.active_play_id)
            })
        {
            return Err(BodyStartupRefusal::StalePlay);
        }
        let first_wake = self
            .body
            .events
            .iter()
            .find_map(|event| match event {
                BodyLifecycleEvent::Woke { wake_id, .. } => Some(wake_id),
                _ => None,
            })
            .ok_or(BodyStartupRefusal::MissingStartupEvidence)?;
        let wake_sign_id = wake
            .events
            .iter()
            .find_map(|event| match event {
                WakeLifecycleEvent::Woke { sign_id } => Some(sign_id),
                _ => None,
            })
            .ok_or(BodyStartupRefusal::MissingStartupEvidence)?;
        let first_play = wake
            .events
            .iter()
            .find_map(|event| match event {
                WakeLifecycleEvent::PlayStarted { active_play_id, .. }
                | WakeLifecycleEvent::HeldPlanReleased { active_play_id, .. } => {
                    Some(active_play_id)
                }
                _ => None,
            })
            .ok_or(BodyStartupRefusal::MissingStartupEvidence)?;
        let play_start_sign_id = wake
            .events
            .iter()
            .find_map(|event| match event {
                WakeLifecycleEvent::PlayStarted {
                    plan_id,
                    active_play_id,
                    sign_id,
                }
                | WakeLifecycleEvent::HeldPlanReleased {
                    plan_id,
                    active_play_id,
                    sign_id,
                } if plan_id == &plan.plan_id && active_play_id == &play.active_play_id => {
                    Some(sign_id)
                }
                _ => None,
            })
            .ok_or(BodyStartupRefusal::MissingStartupEvidence)?;
        Ok(BodyStartup {
            body_id: self.body_id.clone(),
            wake_id: wake_id.clone(),
            active_play_id: play.active_play_id.clone(),
            wake_sign_id: wake_sign_id.clone(),
            play_start_sign_id: play_start_sign_id.clone(),
            first_body_wake: first_wake == wake_id,
            first_play_in_wake: first_play == &play.active_play_id,
        })
    }
}
