//! Durable Realm deployment and activation continuity around exact Plans and Plays.

use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    bind_active_play, verify_plan, ActivePlayId, ActivePlayIdentity, CheckedFormId, EvidenceId,
    Plan, PlanId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::lifecycle_events::{
    validate_activation_events, validate_deployment_event_state, validate_deployment_events,
};
use crate::lifecycle_identity::{bind_lifecycle_identity, validate_lifecycle_ids};
use crate::lifecycle_validation::{push_evidence, validate_evidence, validate_plan_history};
use crate::{
    ActivationId, ActivationLifecycleEvent, DeploymentId, DeploymentLifecycleEvent, RealmId,
};

pub const MAX_DEPLOYMENT_EVIDENCE: usize = 16;
pub const MAX_ACTIVATION_EVIDENCE: usize = 32;
pub const MAX_ACTIVATION_PLANS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentState {
    Inactive,
    Active { activation_id: ActivationId },
    Undeployed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmDeployment {
    pub deployment_id: DeploymentId,
    pub realm_id: RealmId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub deployment_sequence: u64,
    pub state: DeploymentState,
    pub evidence_ids: Vec<EvidenceId>,
    pub events: Vec<DeploymentLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationLifecycle {
    AwaitingPlan,
    AwaitingPlay,
    Active,
    Unsatisfied,
    Deactivated,
    Failed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationPlanState {
    AwaitingPlay,
    Playing,
    Unsatisfied,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationPlan {
    pub plan_id: PlanId,
    pub active_play_id: Option<ActivePlayId>,
    pub state: ActivationPlanState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealmActivation {
    pub activation_id: ActivationId,
    pub deployment_id: DeploymentId,
    pub realm_id: RealmId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub activation_sequence: u64,
    pub lifecycle: ActivationLifecycle,
    pub plans: Vec<ActivationPlan>,
    pub evidence_ids: Vec<EvidenceId>,
    pub events: Vec<ActivationLifecycleEvent>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RealmLifecycleError {
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
    MismatchedActivation,
}

impl core::fmt::Display for RealmLifecycleError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid Realm lifecycle transition: {self:?}")
    }
}

impl RealmDeployment {
    pub fn install(
        realm_id: RealmId,
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        deployment_sequence: u64,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        validate_lifecycle_ids(&[
            realm_id.as_str(),
            source_document_id.as_str(),
            checked_form_id.as_str(),
            evidence_id.as_str(),
        ])?;
        let deployment_id = DeploymentId::bound(bind_lifecycle_identity(
            "realm-deployment",
            &[
                realm_id.as_str(),
                source_document_id.as_str(),
                checked_form_id.as_str(),
            ],
            deployment_sequence,
        ));
        Ok(Self {
            deployment_id,
            realm_id,
            source_document_id,
            checked_form_id,
            deployment_sequence,
            state: DeploymentState::Inactive,
            evidence_ids: vec![evidence_id.clone()],
            events: vec![DeploymentLifecycleEvent::Installed { evidence_id }],
        })
    }

    pub fn activate(
        &self,
        activation_sequence: u64,
        evidence_id: EvidenceId,
    ) -> Result<(Self, RealmActivation), RealmLifecycleError> {
        self.validate()?;
        if self.state != DeploymentState::Inactive {
            return Err(RealmLifecycleError::InvalidTransition);
        }
        let activation_id = ActivationId::bound(bind_lifecycle_identity(
            "realm-activation",
            &[self.realm_id.as_str(), self.deployment_id.as_str()],
            activation_sequence,
        ));
        let mut deployment = self.clone();
        deployment.push_event(DeploymentLifecycleEvent::Activated {
            activation_id: activation_id.clone(),
            evidence_id: evidence_id.clone(),
        })?;
        deployment.state = DeploymentState::Active {
            activation_id: activation_id.clone(),
        };
        let activation = RealmActivation {
            activation_id,
            deployment_id: self.deployment_id.clone(),
            realm_id: self.realm_id.clone(),
            source_document_id: self.source_document_id.clone(),
            checked_form_id: self.checked_form_id.clone(),
            activation_sequence,
            lifecycle: ActivationLifecycle::AwaitingPlan,
            plans: Vec::new(),
            evidence_ids: vec![evidence_id.clone()],
            events: vec![ActivationLifecycleEvent::Activated { evidence_id }],
        };
        Ok((deployment, activation))
    }

    pub fn retain_after_activation(
        &self,
        activation: &RealmActivation,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        activation.validate()?;
        if !self.matches_activation(activation)
            || !matches!(
                activation.lifecycle,
                ActivationLifecycle::Deactivated | ActivationLifecycle::Failed
            )
        {
            return Err(RealmLifecycleError::MismatchedActivation);
        }
        let mut next = self.clone();
        next.push_event(DeploymentLifecycleEvent::ActivationRetained {
            activation_id: activation.activation_id.clone(),
            evidence_id,
        })?;
        next.state = DeploymentState::Inactive;
        Ok(next)
    }

    pub fn undeploy(&self, evidence_id: EvidenceId) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if self.state != DeploymentState::Inactive {
            return Err(RealmLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(DeploymentLifecycleEvent::Undeployed { evidence_id })?;
        next.state = DeploymentState::Undeployed;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), RealmLifecycleError> {
        validate_lifecycle_ids(&[
            self.realm_id.as_str(),
            self.source_document_id.as_str(),
            self.checked_form_id.as_str(),
            self.deployment_id.as_str(),
        ])?;
        validate_evidence(&self.evidence_ids, MAX_DEPLOYMENT_EVIDENCE)?;
        validate_deployment_events(&self.events, &self.evidence_ids)?;
        validate_deployment_event_state(&self.events, &self.state)?;
        if self.deployment_id.as_str()
            != bind_lifecycle_identity(
                "realm-deployment",
                &[
                    self.realm_id.as_str(),
                    self.source_document_id.as_str(),
                    self.checked_form_id.as_str(),
                ],
                self.deployment_sequence,
            )
        {
            return Err(RealmLifecycleError::InvalidIdentity);
        }
        Ok(())
    }

    fn matches_activation(&self, activation: &RealmActivation) -> bool {
        self.realm_id == activation.realm_id
            && self.deployment_id == activation.deployment_id
            && self.source_document_id == activation.source_document_id
            && self.checked_form_id == activation.checked_form_id
            && matches!(
                &self.state,
                DeploymentState::Active { activation_id }
                    if activation_id == &activation.activation_id
            )
    }

    fn push_event(&mut self, event: DeploymentLifecycleEvent) -> Result<(), RealmLifecycleError> {
        push_evidence(
            &mut self.evidence_ids,
            event.evidence_id().clone(),
            MAX_DEPLOYMENT_EVIDENCE,
        )?;
        self.events.push(event);
        Ok(())
    }
}

impl RealmActivation {
    pub fn plan_ready(
        &self,
        plan: &Plan,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        self.validate_plan(plan)?;
        let prior_plan = match self.lifecycle {
            ActivationLifecycle::AwaitingPlan if self.plans.is_empty() => None,
            ActivationLifecycle::Unsatisfied => self.plans.last().map(|item| &item.plan_id),
            _ => return Err(RealmLifecycleError::InvalidTransition),
        };
        if prior_plan == Some(&plan.plan_id) {
            return Err(RealmLifecycleError::StalePlan);
        }
        if self.plans.len() >= MAX_ACTIVATION_PLANS {
            return Err(RealmLifecycleError::PlanCapacityExhausted);
        }
        let mut next = self.clone();
        let event = if let Some(prior_plan_id) = prior_plan {
            ActivationLifecycleEvent::Replanned {
                prior_plan_id: prior_plan_id.clone(),
                replacement_plan_id: plan.plan_id.clone(),
                evidence_id,
            }
        } else {
            ActivationLifecycleEvent::PlanReady {
                plan_id: plan.plan_id.clone(),
                evidence_id,
            }
        };
        next.push_event(event)?;
        if let Some(previous) = next.plans.last_mut() {
            previous.state = ActivationPlanState::Superseded;
        }
        next.plans.push(ActivationPlan {
            plan_id: plan.plan_id.clone(),
            active_play_id: None,
            state: ActivationPlanState::AwaitingPlay,
        });
        next.lifecycle = ActivationLifecycle::AwaitingPlay;
        Ok(next)
    }

    pub fn play_started(
        &self,
        identity: &ActivePlayIdentity,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if self.lifecycle != ActivationLifecycle::AwaitingPlay {
            return Err(RealmLifecycleError::InvalidTransition);
        }
        validate_lifecycle_ids(&[
            identity.active_play_id.as_str(),
            identity.plan_id.as_str(),
            identity.host_id.as_str(),
            identity.boot_id.as_str(),
        ])?;
        let expected = bind_active_play(
            &identity.plan_id,
            &identity.host_id,
            &identity.boot_id,
            identity.activation_sequence,
        );
        let Some(current) = self.plans.last() else {
            return Err(RealmLifecycleError::InvalidTransition);
        };
        if &expected != identity || current.plan_id != identity.plan_id {
            return Err(RealmLifecycleError::StalePlay);
        }
        let mut next = self.clone();
        next.push_event(ActivationLifecycleEvent::PlayStarted {
            plan_id: identity.plan_id.clone(),
            active_play_id: identity.active_play_id.clone(),
            evidence_id,
        })?;
        let current = next
            .plans
            .last_mut()
            .ok_or(RealmLifecycleError::InvalidTransition)?;
        current.active_play_id = Some(identity.active_play_id.clone());
        current.state = ActivationPlanState::Playing;
        next.lifecycle = ActivationLifecycle::Active;
        Ok(next)
    }

    pub fn became_unsatisfied(
        &self,
        plan_id: &PlanId,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if self.lifecycle != ActivationLifecycle::Active
            || self.plans.last().map(|item| &item.plan_id) != Some(plan_id)
        {
            return Err(RealmLifecycleError::StalePlan);
        }
        let mut next = self.clone();
        next.push_event(ActivationLifecycleEvent::BecameUnsatisfied {
            plan_id: plan_id.clone(),
            evidence_id,
        })?;
        next.plans
            .last_mut()
            .ok_or(RealmLifecycleError::InvalidTransition)?
            .state = ActivationPlanState::Unsatisfied;
        next.lifecycle = ActivationLifecycle::Unsatisfied;
        Ok(next)
    }

    pub fn same_plan_observed(
        &self,
        plan_id: &PlanId,
        evidence_id: EvidenceId,
    ) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if self.lifecycle != ActivationLifecycle::Active
            || self.plans.last().map(|item| &item.plan_id) != Some(plan_id)
        {
            return Err(RealmLifecycleError::StalePlan);
        }
        let mut next = self.clone();
        next.push_event(ActivationLifecycleEvent::SamePlanObserved {
            plan_id: plan_id.clone(),
            evidence_id,
        })?;
        Ok(next)
    }

    pub fn deactivate(&self, evidence_id: EvidenceId) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            ActivationLifecycle::Deactivated | ActivationLifecycle::Failed
        ) {
            return Err(RealmLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(ActivationLifecycleEvent::Deactivated { evidence_id })?;
        next.lifecycle = ActivationLifecycle::Deactivated;
        Ok(next)
    }

    pub fn fail(&self, evidence_id: EvidenceId) -> Result<Self, RealmLifecycleError> {
        self.validate()?;
        if matches!(
            self.lifecycle,
            ActivationLifecycle::Deactivated | ActivationLifecycle::Failed
        ) {
            return Err(RealmLifecycleError::InvalidTransition);
        }
        let mut next = self.clone();
        next.push_event(ActivationLifecycleEvent::Failed { evidence_id })?;
        next.lifecycle = ActivationLifecycle::Failed;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), RealmLifecycleError> {
        validate_lifecycle_ids(&[
            self.activation_id.as_str(),
            self.deployment_id.as_str(),
            self.realm_id.as_str(),
            self.source_document_id.as_str(),
            self.checked_form_id.as_str(),
        ])?;
        validate_evidence(&self.evidence_ids, MAX_ACTIVATION_EVIDENCE)?;
        validate_activation_events(
            &self.events,
            &self.evidence_ids,
            self.lifecycle,
            &self.plans,
        )?;
        if self.plans.len() > MAX_ACTIVATION_PLANS
            || self.activation_id.as_str()
                != bind_lifecycle_identity(
                    "realm-activation",
                    &[self.realm_id.as_str(), self.deployment_id.as_str()],
                    self.activation_sequence,
                )
        {
            return Err(RealmLifecycleError::InvalidIdentity);
        }
        validate_plan_history(self.lifecycle, &self.plans)
    }

    fn validate_plan(&self, plan: &Plan) -> Result<(), RealmLifecycleError> {
        validate_lifecycle_ids(&[
            plan.plan_id.as_str(),
            plan.source_document_id.as_str(),
            plan.checked_form_id.as_str(),
            plan.expanded_form_id.as_str(),
        ])?;
        if !verify_plan(plan) {
            return Err(RealmLifecycleError::InvalidPlan);
        }
        if plan.source_document_id != self.source_document_id
            || plan.checked_form_id != self.checked_form_id
        {
            return Err(RealmLifecycleError::StalePlan);
        }
        Ok(())
    }

    fn push_event(&mut self, event: ActivationLifecycleEvent) -> Result<(), RealmLifecycleError> {
        push_evidence(
            &mut self.evidence_ids,
            event.evidence_id().clone(),
            MAX_ACTIVATION_EVIDENCE,
        )?;
        self.events.push(event);
        Ok(())
    }
}
