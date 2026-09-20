//! Hosted ownership of one exact planned shared-pool population.
//!
//! The controller is deliberately generic: it binds rich Plan and observation
//! truth through plan lowering, delegates finite selection and atomic
//! occupation to the existing kernel pool, and returns typed evidence. It does
//! not invoke providers, replay operations, or request replacement planning.

use crate::lowering::{lower_plan_fragment, LoweredSharedPool, PoolObservationLoweringError};
use alloc::vec::Vec;
use conduit_core::{
    Plan, PlanId, PoolOperationId, PoolRealizationObservation, PoolSelectionDisposition,
    PoolSelectionEvidence, SharedPoolId, SignId,
};
use conduit_kernel::{
    shared_pool::{
        admit_selected_pool_member, FixedSharedPool, MemberIdentity, MemberKey, PoolError,
        PoolSelectionError, PoolSelectionPolicy,
    },
    NodeId,
};

const ADMISSION_AUTHORITY_TOKEN: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedPoolAdmissionOutcome {
    Selected {
        member: MemberIdentity,
        evidence: PoolSelectionEvidence,
    },
    Refused(PoolSelectionEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharedPoolRuntimeError {
    InvalidPlan,
    FragmentOutsidePlan,
    PoolMissing,
    PoolIdentityAmbiguous,
    CapacityExceeded,
    Observation(PoolObservationLoweringError),
    Selection(PoolSelectionError),
    Lifecycle(PoolError),
    InvalidEvidence,
}

pub struct HostedSharedPool<
    const MEMBER_SLOTS: usize,
    const POOL_SIGNS: usize,
    const OBSERVATION_SIGNS: usize,
> {
    plan: Plan,
    pool_id: SharedPoolId,
    lowered: LoweredSharedPool,
    kernel: FixedSharedPool<MEMBER_SLOTS, POOL_SIGNS>,
    first_member_node: NodeId,
    play: u16,
}

impl<const MEMBER_SLOTS: usize, const POOL_SIGNS: usize, const OBSERVATION_SIGNS: usize>
    HostedSharedPool<MEMBER_SLOTS, POOL_SIGNS, OBSERVATION_SIGNS>
{
    pub fn new(
        plan: &Plan,
        fragment_index: usize,
        pool_id: SharedPoolId,
        first_member_node: NodeId,
        play: u16,
    ) -> Result<Self, SharedPoolRuntimeError> {
        if !conduit_core::verify_plan(plan) {
            return Err(SharedPoolRuntimeError::InvalidPlan);
        }
        let fragment = plan
            .fragments
            .get(fragment_index)
            .ok_or(SharedPoolRuntimeError::FragmentOutsidePlan)?;
        let lowered =
            lower_plan_fragment(fragment).map_err(|_| SharedPoolRuntimeError::InvalidPlan)?;
        let mut matching = lowered
            .shared_pools
            .into_iter()
            .filter(|pool| pool.pool_id == pool_id);
        let lowered = matching.next().ok_or(SharedPoolRuntimeError::PoolMissing)?;
        if matching.next().is_some() {
            return Err(SharedPoolRuntimeError::PoolIdentityAmbiguous);
        }
        if usize::from(lowered.maximum_members) > MEMBER_SLOTS
            || lowered.realizations.len() > usize::from(u16::MAX)
        {
            return Err(SharedPoolRuntimeError::CapacityExceeded);
        }
        let kernel = FixedSharedPool::new(
            lowered.pool,
            lowered.maximum_members,
            ADMISSION_AUTHORITY_TOKEN,
            lowered.realizations.len() as u16,
        )
        .map_err(SharedPoolRuntimeError::Lifecycle)?;
        Ok(Self {
            plan: plan.clone(),
            pool_id,
            lowered,
            kernel,
            first_member_node,
            play,
        })
    }

    pub fn plan_id(&self) -> &PlanId {
        &self.plan.plan_id
    }

    pub fn population(&self) -> u16 {
        self.kernel.population()
    }

    pub fn admit_new_operation(
        &mut self,
        operation_id: PoolOperationId,
        key: MemberKey,
        current: &[PoolRealizationObservation],
        selection_sign_id: SignId,
    ) -> Result<SharedPoolAdmissionOutcome, SharedPoolRuntimeError> {
        let facts = self
            .lowered
            .lower_selection_facts(current, self.first_member_node, self.play)
            .map_err(SharedPoolRuntimeError::Observation)?;
        let mut selected_signs = [0_u16; OBSERVATION_SIGNS];
        match admit_selected_pool_member(
            &mut self.kernel,
            key,
            ADMISSION_AUTHORITY_TOKEN,
            &facts.envelope,
            &facts.requirements,
            &facts.observations,
            lower_policy(self.lowered.selection_policy),
            &mut selected_signs,
        ) {
            Ok(selected) => {
                let evidence = self.evidence(
                    operation_id,
                    Some(selected.member.placement.realization),
                    selected_signs[..usize::from(selected.observation_sign_count)]
                        .iter()
                        .map(|index| facts.observation_sign_ids[usize::from(*index)].clone())
                        .collect(),
                    PoolSelectionDisposition::Selected,
                    selection_sign_id,
                )?;
                Ok(SharedPoolAdmissionOutcome::Selected {
                    member: selected.member,
                    evidence,
                })
            }
            Err(PoolSelectionError::CapacityUnavailable { .. })
            | Err(PoolSelectionError::Admission(PoolError::PoolFull)) => {
                Ok(SharedPoolAdmissionOutcome::Refused(self.evidence(
                    operation_id,
                    None,
                    facts.observation_sign_ids,
                    PoolSelectionDisposition::CapacityRefused,
                    selection_sign_id,
                )?))
            }
            Err(PoolSelectionError::NoCurrentRealization { .. }) => {
                Ok(SharedPoolAdmissionOutcome::Refused(self.evidence(
                    operation_id,
                    None,
                    facts.observation_sign_ids,
                    PoolSelectionDisposition::EnvelopeExhausted,
                    selection_sign_id,
                )?))
            }
            Err(error) => Err(SharedPoolRuntimeError::Selection(error)),
        }
    }

    pub fn trigger(&mut self, member: MemberIdentity) -> Result<(), SharedPoolRuntimeError> {
        self.kernel
            .trigger(member)
            .map_err(SharedPoolRuntimeError::Lifecycle)
    }

    pub fn release(&mut self, member: MemberIdentity) -> Result<(), SharedPoolRuntimeError> {
        self.kernel
            .request_release(member)
            .and_then(|()| self.kernel.complete_release(member))
            .map_err(SharedPoolRuntimeError::Lifecycle)
    }

    pub fn provider_lost(
        &mut self,
        operation_id: PoolOperationId,
        member: MemberIdentity,
        current: &[PoolRealizationObservation],
        loss_sign_id: SignId,
    ) -> Result<PoolSelectionEvidence, SharedPoolRuntimeError> {
        let facts = self
            .lowered
            .lower_selection_facts(current, self.first_member_node, self.play)
            .map_err(SharedPoolRuntimeError::Observation)?;
        let realization = member.placement.realization;
        let observation_sign_ids = facts
            .observations
            .iter()
            .filter(|observation| observation.realization == realization)
            .map(|observation| facts.observation_sign_ids[usize::from(observation.sign)].clone())
            .collect::<Vec<_>>();
        if observation_sign_ids.is_empty() {
            return Err(SharedPoolRuntimeError::InvalidEvidence);
        }
        self.kernel
            .fail_member(member)
            .map_err(SharedPoolRuntimeError::Lifecycle)?;
        self.evidence(
            operation_id,
            Some(realization),
            observation_sign_ids,
            PoolSelectionDisposition::ProviderLost,
            loss_sign_id,
        )
    }

    fn evidence(
        &self,
        operation_id: PoolOperationId,
        selected_realization: Option<u16>,
        observation_sign_ids: Vec<SignId>,
        disposition: PoolSelectionDisposition,
        sign_id: SignId,
    ) -> Result<PoolSelectionEvidence, SharedPoolRuntimeError> {
        let evidence = PoolSelectionEvidence {
            plan_id: self.plan.plan_id.clone(),
            pool_id: self.pool_id.clone(),
            operation_id,
            selected_realization,
            observation_sign_ids,
            disposition,
            sign_id,
        };
        evidence
            .validate(&self.plan)
            .map_err(|_| SharedPoolRuntimeError::InvalidEvidence)?;
        Ok(evidence)
    }
}

fn lower_policy(policy: conduit_core::SharedPoolSelectionPolicy) -> PoolSelectionPolicy {
    match policy {
        conduit_core::SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder => {
            PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder
        }
    }
}

#[cfg(test)]
#[path = "shared_pool_runtime/tests.rs"]
mod tests;
