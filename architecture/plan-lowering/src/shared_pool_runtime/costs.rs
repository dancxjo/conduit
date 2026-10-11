//! One bounded prepared-cost handoff to the existing atomic kernel pool.
use super::*;
use alloc::vec;
use conduit_core::{
    HostAdvertisement, MonotonicInstant, PoolCostCalibrationRevision, PoolCostCandidateAccount,
    PoolCostChoiceBasis, PoolCostDisposition, PoolCostExecutionEvaluation, PoolCostHardDisposition,
    PoolCostSelectionAccount, PoolCostSourceAvailability, PoolCostUnavailable, PreparedPoolCost,
    SharedPoolSelectionPolicy,
};
use conduit_kernel::shared_pool::{
    admit_cost_selected_pool_member, LoweredPoolCost, LoweredPoolDuration,
    PoolCostCandidateDecision, PoolCostSelection, MAXIMUM_POOL_COST_REALIZATIONS,
};

pub struct PoolCostEvaluation<'a> {
    pub host: &'a HostAdvertisement,
    pub now: &'a MonotonicInstant,
    pub calibrations: &'a [PoolCostCalibrationRevision],
    pub executions: &'a [PoolCostExecutionEvaluation<'a>],
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostLoweringError {
    InputLimit,
    PolicyMismatch,
    Correlation,
    DuplicateRealization,
    DuplicateHost,
    DuplicateCalibration,
}
pub use conduit_core::PoolCostSourceReceipt;
#[derive(Debug)]
pub struct CostAwareSharedPoolAdmissionOutcome {
    pub admission: SharedPoolAdmissionOutcome,
    pub account: conduit_core::PoolCostSelectionAccount,
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
        if [operation_id.as_str(), selection_sign_id.as_str()]
            .iter()
            .any(|id| id.is_empty() || id.len() > 256)
        {
            return Err(SharedPoolRuntimeError::InvalidEvidence);
        }
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
        if current
            .iter()
            .flat_map(|o| {
                core::iter::once(&o.sign_id).chain(o.resources.iter().map(|r| &r.sign_id))
            })
            .any(|sign| sign.as_str().is_empty() || sign.as_str().len() > 256)
        {
            return Err(SharedPoolRuntimeError::InvalidEvidence);
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
            || evaluations
                .iter()
                .try_fold(0usize, |total, context| {
                    total
                        .checked_add(context.calibrations.len())?
                        .checked_add(context.executions.len())
                })
                .is_none_or(|total| total > 256)
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
            for (index, revision) in context.calibrations.iter().enumerate() {
                if context.calibrations[..index]
                    .iter()
                    .any(|old| old.identity() == revision.identity())
                {
                    return Err(SharedPoolRuntimeError::Cost(E::DuplicateCalibration));
                }
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
                .find(|c| c.host.host_id == cost.host().host_id);
            let calibration = context.and_then(|c| {
                c.calibrations
                    .iter()
                    .find(|r| r.identity() == cost.calibration().identity())
            });
            let checked = (|| {
                let context = context.ok_or(PoolCostUnavailable::MissingHost)?;
                cost.validate_at(context.host, context.now)
                    .map_err(PoolCostUnavailable::Prepared)?;
                let calibration = calibration.ok_or(PoolCostUnavailable::MissingCalibration)?;
                cost.validate_dependencies(calibration, context.executions, context.now)
                    .map_err(PoolCostUnavailable::Prepared)
            })();
            let (availability, execution) = match checked {
                Ok(execution) => (PoolCostSourceAvailability::Known, execution),
                Err(reason) => (PoolCostSourceAvailability::Unavailable(reason), None),
            };
            let realization = *facts
                .envelope
                .get(usize::from(cost.realization()))
                .ok_or(SharedPoolRuntimeError::Cost(E::Correlation))?;
            if availability == PoolCostSourceAvailability::Known {
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
                    evaluation_ticks: context
                        .expect("known source checked its current Host")
                        .now
                        .ticks(),
                    sign: index as u16,
                });
            }
            sources.push(PoolCostSourceReceipt {
                realization: cost.realization(),
                host: cost.host().host_id.clone(),
                boot: cost.host().boot_id.clone(),
                generation: cost.host().offer_generation,
                host_profile: cost.host().profile.clone(),
                sign: cost.sign().clone(),
                supporting_signs: cost.supporting_signs().to_vec(),
                capability: cost.offer().capability_id.clone(),
                kind: cost.offer().kind_id.clone(),
                kind_contract: cost.offer().kind_contract_revision.clone(),
                implementation: cost.offer().implementation.implementation_id.clone(),
                artifact: cost.offer().implementation.artifact_id.clone(),
                execution_profile: cost.offer().implementation.execution_profile_id.clone(),
                resource: cost
                    .resource()
                    .map(|r| (r.pool_id.clone(), r.class_id.clone())),
                observed: cost.observed().clone(),
                expires: cost.expires().clone(),
                evaluated_at: context.map(|c| c.now.clone()),
                duration: cost.duration(),
                calibration: cost.calibration().clone(),
                evaluated_calibration: calibration.cloned(),
                expected_execution: cost.expected_execution(),
                availability,
                current_execution: execution,
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
        signs.extend(
            sources
                .iter()
                .flat_map(|source| source.supporting_signs.iter().cloned()),
        );
        signs.extend(
            sources
                .iter()
                .filter_map(|source| source.current_execution.as_ref().map(|e| e.sign.clone())),
        );
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
        let candidates = decisions[..facts.envelope.len()]
            .iter()
            .map(|decision| {
                let admission = match decision.admission {
                    conduit_kernel::shared_pool::PoolCostAdmission::NotCurrent => {
                        PoolCostHardDisposition::NotCurrent
                    }
                    conduit_kernel::shared_pool::PoolCostAdmission::Full => {
                        PoolCostHardDisposition::Full
                    }
                    conduit_kernel::shared_pool::PoolCostAdmission::Eligible => {
                        PoolCostHardDisposition::Eligible
                    }
                };
                let source = sources
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.realization == decision.realization);
                let cost = if admission == PoolCostHardDisposition::NotCurrent {
                    PoolCostDisposition::NotCompared
                } else {
                    match source {
                        None => PoolCostDisposition::Missing,
                        Some((index, s)) => match s.availability {
                            PoolCostSourceAvailability::Known => PoolCostDisposition::Known {
                                upper_nanoseconds: s.duration.upper_nanoseconds(),
                                source: index as u16,
                            },
                            PoolCostSourceAvailability::Unavailable(reason) => {
                                PoolCostDisposition::Unavailable(reason)
                            }
                        },
                    }
                };
                PoolCostCandidateAccount {
                    realization: decision.realization,
                    admission,
                    cost,
                }
            })
            .collect::<Vec<_>>();
        let evidence = match &admission {
            SharedPoolAdmissionOutcome::Selected { evidence, .. }
            | SharedPoolAdmissionOutcome::Refused(evidence) => evidence.clone(),
        };
        let choice =
            evidence
                .selected_realization
                .map(|index| match candidates[usize::from(index)].cost {
                    PoolCostDisposition::Known { .. } => {
                        PoolCostChoiceBasis::MinimumPredictedCompletion
                    }
                    _ => PoolCostChoiceBasis::CapacityReference,
                });
        let account = PoolCostSelectionAccount {
            evidence,
            demand,
            choice,
            candidates,
            sources,
        };
        debug_assert!(account.validate(&self.plan).is_ok());
        Ok(CostAwareSharedPoolAdmissionOutcome { admission, account })
    }
}
