//! Bounded optional gate between one exact Plan and its first Play.
//!
//! HOLD is Wake lifecycle state. It retains the immutable Plan, the exact
//! planning-basis Sign identities, the hold policy, and the explicit release
//! authority without issuing an active Play identity or invoking a platform.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayIdentity, AuthorityContractId, AuthorityGrantId,
    BootId, HostId, Plan, PlanId, SignId,
};
use serde::{Deserialize, Serialize};

use crate::identity::validate_ids;
use crate::{
    BodyLifecycleError, Wake, WakeLifecycle, WakeLifecycleEvent, WakePlan, WakePlanState,
    MAX_WAKE_PLANS,
};

pub const HOLD_RELEASE_AUTHORITY_CONTRACT: &str = "conduit.authority/release-held-plan@1";
pub const MAX_HOLD_BASIS_SIGNS: usize = 32;

macro_rules! descriptive_identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

descriptive_identity!(HoldReasonId);
descriptive_identity!(HoldSourceId);

/// Exact authority contract and issued grant required to release one HOLD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldReleaseAuthority {
    pub contract_id: AuthorityContractId,
    pub grant_id: AuthorityGrantId,
}

impl HoldReleaseAuthority {
    pub fn new(grant_id: AuthorityGrantId) -> Self {
        Self {
            contract_id: AuthorityContractId::from(HOLD_RELEASE_AUTHORITY_CONTRACT),
            grant_id,
        }
    }

    fn validate(&self) -> Result<(), BodyLifecycleError> {
        validate_ids(&[self.contract_id.as_str(), self.grant_id.as_str()])?;
        if self.contract_id.as_str() != HOLD_RELEASE_AUTHORITY_CONTRACT {
            return Err(BodyLifecycleError::AuthorityDenied);
        }
        Ok(())
    }
}

/// Inspectable reason/source and replacement behavior for one held Plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldPolicy {
    pub reason: HoldReasonId,
    pub source: HoldSourceId,
    pub release_authority: HoldReleaseAuthority,
    pub hold_replacement_plan: bool,
}

impl HoldPolicy {
    fn validate(&self) -> Result<(), BodyLifecycleError> {
        validate_ids(&[self.reason.as_str(), self.source.as_str()])?;
        self.release_authority.validate()
    }
}

/// Exact finite set of planning-basis Signs represented by their Sign IDs.
///
/// The producer must include every current fact whose change can invalidate
/// the Plan. Release compares this complete set exactly; it never infers
/// authority or validity from visible Hosts or reachable Lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningBasis {
    sign_ids: Vec<SignId>,
}

impl PlanningBasis {
    pub fn new(sign_ids: Vec<SignId>) -> Result<Self, BodyLifecycleError> {
        let basis = Self { sign_ids };
        basis.validate()?;
        Ok(basis)
    }

    pub fn sign_ids(&self) -> &[SignId] {
        &self.sign_ids
    }

    fn validate(&self) -> Result<(), BodyLifecycleError> {
        validate_planning_basis_signs(&self.sign_ids)
    }
}

pub(crate) fn validate_planning_basis_signs(sign_ids: &[SignId]) -> Result<(), BodyLifecycleError> {
    if sign_ids.is_empty() {
        return Err(BodyLifecycleError::InvalidPlanningBasis);
    }
    if sign_ids.len() > MAX_HOLD_BASIS_SIGNS {
        return Err(BodyLifecycleError::PlanningBasisCapacityExhausted);
    }
    for (index, sign) in sign_ids.iter().enumerate() {
        validate_ids(&[sign.as_str()])?;
        if sign_ids[..index].contains(sign) {
            return Err(BodyLifecycleError::InvalidPlanningBasis);
        }
    }
    Ok(())
}

/// Immutable held Plan together with the basis and policy that admitted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanHold {
    pub plan: Plan,
    pub basis: PlanningBasis,
    pub policy: HoldPolicy,
}

impl PlanHold {
    pub(crate) fn validate_for_plan(
        &self,
        expected_plan_id: &PlanId,
    ) -> Result<(), BodyLifecycleError> {
        self.basis.validate()?;
        self.policy.validate()?;
        if &self.plan.plan_id != expected_plan_id || !verify_plan(&self.plan) {
            return Err(BodyLifecycleError::InvalidPlan);
        }
        Ok(())
    }
}

/// Read-only core projection of one currently held Plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldPlanInspection<'a> {
    pub plan: &'a Plan,
    pub basis: &'a PlanningBasis,
    pub policy: &'a HoldPolicy,
    pub remains_valid: bool,
}

/// The only two successful dispositions of an authorized release attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldReleaseOutcome {
    PlayStarted {
        wake: Wake,
        active_play: ActivePlayIdentity,
    },
    ReplanRequired {
        wake: Wake,
    },
}

impl Wake {
    pub fn plan_held(
        &self,
        plan: &Plan,
        basis: PlanningBasis,
        policy: HoldPolicy,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        self.validate_plan(plan)?;
        basis.validate()?;
        policy.validate()?;
        let prior = match self.lifecycle {
            WakeLifecycle::AwaitingPlan if self.plans.is_empty() => None,
            WakeLifecycle::Unsatisfied => self.plans.last(),
            WakeLifecycle::AwaitingReplacement => {
                let previous = self
                    .plans
                    .last()
                    .ok_or(BodyLifecycleError::InvalidTransition)?;
                let previous_policy = &previous
                    .hold
                    .as_ref()
                    .ok_or(BodyLifecycleError::InvalidTransition)?
                    .policy;
                if !previous_policy.hold_replacement_plan || previous_policy != &policy {
                    return Err(BodyLifecycleError::HoldRequired);
                }
                Some(previous)
            }
            _ => return Err(BodyLifecycleError::InvalidTransition),
        };
        if prior.is_some_and(|prior| prior.plan_id == plan.plan_id) {
            return Err(BodyLifecycleError::StalePlan);
        }
        if self.plans.len() >= MAX_WAKE_PLANS {
            return Err(BodyLifecycleError::PlanCapacityExhausted);
        }
        let hold = PlanHold {
            plan: plan.clone(),
            basis,
            policy,
        };
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::PlanHeld {
            prior_plan_id: prior.map(|prior| prior.plan_id.clone()),
            plan_id: plan.plan_id.clone(),
            basis_sign_ids: hold.basis.sign_ids.clone(),
            policy: hold.policy.clone(),
            sign_id,
        })?;
        if let Some(previous) = next.plans.last_mut() {
            previous.state = WakePlanState::Superseded;
        }
        next.plans.push(WakePlan {
            plan_id: plan.plan_id.clone(),
            active_play_id: None,
            state: WakePlanState::Held,
            hold: Some(hold),
        });
        next.lifecycle = WakeLifecycle::Held;
        Ok(next)
    }

    pub fn inspect_hold<'a>(
        &'a self,
        current_basis: &PlanningBasis,
    ) -> Result<Option<HeldPlanInspection<'a>>, BodyLifecycleError> {
        self.validate()?;
        current_basis.validate()?;
        if self.lifecycle != WakeLifecycle::Held {
            return Ok(None);
        }
        let hold = self
            .plans
            .last()
            .and_then(|plan| plan.hold.as_ref())
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        Ok(Some(HeldPlanInspection {
            plan: &hold.plan,
            basis: &hold.basis,
            policy: &hold.policy,
            remains_valid: &hold.basis == current_basis,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn release_hold(
        &self,
        authority: &HoldReleaseAuthority,
        current_basis: &PlanningBasis,
        host_id: &HostId,
        boot_id: &BootId,
        play_sequence: u64,
        sign_id: SignId,
    ) -> Result<HoldReleaseOutcome, BodyLifecycleError> {
        self.validate()?;
        authority.validate()?;
        current_basis.validate()?;
        validate_ids(&[host_id.as_str(), boot_id.as_str()])?;
        if self.lifecycle != WakeLifecycle::Held {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let hold = self
            .plans
            .last()
            .and_then(|plan| plan.hold.as_ref())
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        if &hold.policy.release_authority != authority {
            return Err(BodyLifecycleError::AuthorityDenied);
        }

        let mut next = self.clone();
        if &hold.basis != current_basis {
            next.push_event(WakeLifecycleEvent::HeldPlanInvalidated {
                plan_id: hold.plan.plan_id.clone(),
                current_basis_sign_ids: current_basis.sign_ids.clone(),
                sign_id,
            })?;
            next.plans
                .last_mut()
                .ok_or(BodyLifecycleError::InvalidTransition)?
                .state = WakePlanState::Invalidated;
            next.lifecycle = WakeLifecycle::AwaitingReplacement;
            return Ok(HoldReleaseOutcome::ReplanRequired { wake: next });
        }

        let active_play = bind_active_play(&hold.plan.plan_id, host_id, boot_id, play_sequence);
        next.push_event(WakeLifecycleEvent::HeldPlanReleased {
            plan_id: hold.plan.plan_id.clone(),
            active_play_id: active_play.active_play_id.clone(),
            sign_id,
        })?;
        let current = next
            .plans
            .last_mut()
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        current.active_play_id = Some(active_play.active_play_id.clone());
        current.state = WakePlanState::Playing;
        next.lifecycle = WakeLifecycle::Playing;
        Ok(HoldReleaseOutcome::PlayStarted {
            wake: next,
            active_play,
        })
    }
}
