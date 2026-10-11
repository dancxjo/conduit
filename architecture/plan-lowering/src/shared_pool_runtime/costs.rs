//! One bounded prepared-cost handoff to the existing atomic kernel pool.
use super::*;
use alloc::vec;
use conduit_core::{
    BootId, HostAdvertisement, HostId, MonotonicInstant, OfferGeneration,
    PoolCostPreparationRefusal, PreparedPoolCost, PreparedPoolDuration, SharedPoolSelectionPolicy,
};
use conduit_kernel::shared_pool::{
    admit_cost_selected_pool_member, LoweredPoolCost, LoweredPoolDuration,
    PoolCostCandidateDecision, PoolCostSelection, MAXIMUM_POOL_COST_REALIZATIONS,
};

pub struct PoolCostEvaluation<'a> {
    pub host: &'a HostAdvertisement,
    pub now: &'a MonotonicInstant,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostLoweringError {
    InputLimit,
    PolicyMismatch,
    Correlation,
    DuplicateRealization,
    DuplicateHost,
    MissingHost,
    Prepared(PoolCostPreparationRefusal),
}
/// Prompt-free owned numeric/source custody for historical explanation.
#[derive(Debug, Clone)]
pub struct PoolCostSourceReceipt {
    pub realization: u16,
    pub host: HostId,
    pub boot: BootId,
    pub generation: OfferGeneration,
    pub sign: SignId,
    pub observed: MonotonicInstant,
    pub expires: MonotonicInstant,
    pub evaluated_at: MonotonicInstant,
    pub duration: PreparedPoolDuration,
}
#[derive(Debug)]
pub struct CostAwareSharedPoolAdmissionOutcome {
    pub admission: SharedPoolAdmissionOutcome,
    pub decisions: Vec<PoolCostCandidateDecision>,
    pub sources: Vec<PoolCostSourceReceipt>,
}
impl<const MEMBER_SLOTS: usize, const POOL_SIGNS: usize, const OBSERVATION_SIGNS: usize>
    HostedSharedPool<MEMBER_SLOTS, POOL_SIGNS, OBSERVATION_SIGNS>
{
    /// Only a Plan that seals the completion policy may consume these costs.
    /// Every source is checked before the atomic occupation transition. Empty
    /// prepared costs mean missing estimates and use the recorded capacity
    /// fallback; they never fabricate a zero duration or a completion deadline.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_new_operation_with_costs(
        &mut self,
        operation_id: PoolOperationId,
        key: MemberKey,
        current: &[PoolRealizationObservation],
        selection_sign_id: SignId,
        demand: [u8; 32],
        prepared: &[PreparedPoolCost<'_>],
        evaluations: &[PoolCostEvaluation<'_>],
    ) -> Result<CostAwareSharedPoolAdmissionOutcome, SharedPoolRuntimeError> {
        use PoolCostLoweringError as E;
        if self.lowered.selection_policy
            != SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity
        {
            return Err(SharedPoolRuntimeError::Cost(E::PolicyMismatch));
        }
        if self.lowered.realizations.len() > MAXIMUM_POOL_COST_REALIZATIONS
            || prepared.len() > MAXIMUM_POOL_COST_REALIZATIONS
            || evaluations.len() > MAXIMUM_POOL_COST_REALIZATIONS
            || current.len() > MAXIMUM_POOL_COST_REALIZATIONS
        {
            return Err(SharedPoolRuntimeError::Cost(E::InputLimit));
        }
        let requirement_count = self
            .lowered
            .realizations
            .iter()
            .try_fold(0usize, |total, r| {
                total
                    .checked_add(r.resources.len())
                    .and_then(|value| value.checked_add(1))
            });
        let observation_count = current.iter().try_fold(0usize, |total, o| {
            total
                .checked_add(o.resources.len())
                .and_then(|value| value.checked_add(1))
        });
        if requirement_count
            .is_none_or(|count| count > conduit_kernel::shared_pool::MAXIMUM_POOL_COST_FACTS)
            || observation_count
                .is_none_or(|count| count > conduit_kernel::shared_pool::MAXIMUM_POOL_COST_FACTS)
            || evaluations.iter().any(|c| c.host.capabilities.len() > 256)
        {
            return Err(SharedPoolRuntimeError::Cost(E::InputLimit));
        }
        for (index, context) in evaluations.iter().enumerate() {
            if evaluations[..index]
                .iter()
                .any(|prior| prior.host.host_id == context.host.host_id)
            {
                return Err(SharedPoolRuntimeError::Cost(E::DuplicateHost));
            }
        }
        // Validate ordinary evidence identities before changing occupation.
        self.evidence(
            operation_id.clone(),
            None,
            vec![],
            PoolSelectionDisposition::EnvelopeExhausted,
            selection_sign_id.clone(),
        )?;
        let facts = self
            .lowered
            .lower_selection_facts(current, self.first_member_node, self.play)
            .map_err(SharedPoolRuntimeError::Observation)?;
        let mut costs = Vec::with_capacity(prepared.len());
        let mut sources = Vec::with_capacity(prepared.len());
        for (index, cost) in prepared.iter().enumerate() {
            if prepared[..index]
                .iter()
                .any(|prior| prior.realization() == cost.realization())
            {
                return Err(SharedPoolRuntimeError::Cost(E::DuplicateRealization));
            }
            if cost.plan_id() != self.plan_id()
                || cost.fragment_index() != self.fragment_index
                || cost.pool_id() != &self.pool_id
                || cost.operation_id() != &operation_id
                || cost.demand() != demand
            {
                return Err(SharedPoolRuntimeError::Cost(E::Correlation));
            }
            let context = evaluations
                .iter()
                .find(|c| c.host.host_id == cost.host().host_id)
                .ok_or(SharedPoolRuntimeError::Cost(E::MissingHost))?;
            cost.validate_at(context.host, context.now)
                .map_err(|e| SharedPoolRuntimeError::Cost(E::Prepared(e)))?;
            let realization = *facts
                .envelope
                .get(usize::from(cost.realization()))
                .ok_or(SharedPoolRuntimeError::Cost(E::Correlation))?;
            costs.push(LoweredPoolCost {
                realization,
                demand,
                upper_completion: LoweredPoolDuration::from_nanoseconds(
                    cost.duration().upper_nanoseconds(),
                ),
                // Tokens are local to the exact member. Rich clock quality and
                // scale equality were checked by the sealed prepared source.
                source_clock: realization.boot,
                evaluation_clock: realization.boot,
                observed_ticks: cost.observed().ticks(),
                expires_ticks: cost.expires().ticks(),
                evaluation_ticks: context.now.ticks(),
                sign: index as u16,
            });
            sources.push(PoolCostSourceReceipt {
                realization: cost.realization(),
                host: cost.host().host_id.clone(),
                boot: cost.host().boot_id.clone(),
                generation: cost.host().offer_generation,
                sign: cost.sign().clone(),
                observed: cost.observed().clone(),
                expires: cost.expires().clone(),
                evaluated_at: context.now.clone(),
                duration: cost.duration(),
            });
        }
        let mut selected_signs = [0_u16; OBSERVATION_SIGNS];
        let mut decisions = [PoolCostCandidateDecision::EMPTY; MAXIMUM_POOL_COST_REALIZATIONS];
        let selected = admit_cost_selected_pool_member(
            &mut self.kernel,
            key,
            ADMISSION_AUTHORITY_TOKEN,
            &facts.envelope,
            &facts.requirements,
            &facts.observations,
            PoolCostSelection {
                demand,
                costs: &costs,
            },
            &mut selected_signs,
            &mut decisions,
        );
        let mut signs = facts.observation_sign_ids.clone();
        signs.extend(sources.iter().map(|source| source.sign.clone()));
        let admission = match selected {
            Ok(selected) => SharedPoolAdmissionOutcome::Selected {
                member: selected.member,
                evidence: self.evidence(
                    operation_id,
                    Some(selected.member.placement.realization),
                    signs,
                    PoolSelectionDisposition::Selected,
                    selection_sign_id,
                )?,
            },
            Err(PoolSelectionError::CapacityUnavailable { .. })
            | Err(PoolSelectionError::Admission(PoolError::PoolFull)) => {
                SharedPoolAdmissionOutcome::Refused(self.evidence(
                    operation_id,
                    None,
                    signs,
                    PoolSelectionDisposition::CapacityRefused,
                    selection_sign_id,
                )?)
            }
            Err(PoolSelectionError::NoCurrentRealization { .. }) => {
                SharedPoolAdmissionOutcome::Refused(self.evidence(
                    operation_id,
                    None,
                    signs,
                    PoolSelectionDisposition::EnvelopeExhausted,
                    selection_sign_id,
                )?)
            }
            Err(error) => return Err(SharedPoolRuntimeError::Selection(error)),
        };
        Ok(CostAwareSharedPoolAdmissionOutcome {
            admission,
            decisions: decisions[..facts.envelope.len()].to_vec(),
            sources,
        })
    }
}
