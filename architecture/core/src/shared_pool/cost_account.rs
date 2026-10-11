//! Historical cost-selection accounts over an immutable pool envelope.
//! These explain one bounded decision; they grant no capacity or fresh truth.
use super::*;
use crate::{
    ArtifactId, BootId, CapabilityId, ExecutionProfileId, HostId, HostProfileId, ImplementationId,
    KindId, KindIdentity, MonotonicInstant, OfferGeneration, Plan, ResourceClassId, ResourcePoolId,
    SignId,
};
use alloc::vec::Vec;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostUnavailable {
    MissingHost,
    MissingCalibration,
    Prepared(PoolCostPreparationRefusal),
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostSourceAvailability {
    Known,
    Unavailable(PoolCostUnavailable),
}
#[derive(Debug, Clone)]
pub struct PoolCostSourceReceipt {
    pub realization: u16,
    pub host: HostId,
    pub boot: BootId,
    pub generation: OfferGeneration,
    pub host_profile: HostProfileId,
    pub sign: SignId,
    pub supporting_signs: Vec<SignId>,
    pub capability: CapabilityId,
    pub kind: KindId,
    pub kind_contract: KindIdentity,
    pub implementation: ImplementationId,
    pub artifact: ArtifactId,
    pub execution_profile: ExecutionProfileId,
    pub resource: Option<(ResourcePoolId, ResourceClassId)>,
    pub observed: MonotonicInstant,
    pub expires: MonotonicInstant,
    pub evaluated_at: Option<MonotonicInstant>,
    pub duration: PreparedPoolDuration,
    pub calibration: PoolCostCalibrationRevision,
    pub evaluated_calibration: Option<PoolCostCalibrationRevision>,
    pub expected_execution: Option<CostExecutionCondition>,
    pub current_execution: Option<PoolCostExecutionReceipt>,
    pub availability: PoolCostSourceAvailability,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostHardDisposition {
    NotCurrent,
    Full,
    Eligible,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostDisposition {
    NotCompared,
    Missing,
    Unavailable(PoolCostUnavailable),
    Known { upper_nanoseconds: u64, source: u16 },
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PoolCostCandidateAccount {
    pub realization: u16,
    pub admission: PoolCostHardDisposition,
    pub cost: PoolCostDisposition,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostChoiceBasis {
    MinimumPredictedCompletion,
    CapacityReference,
}
#[derive(Debug, Clone)]
pub struct PoolCostSelectionAccount {
    pub evidence: PoolSelectionEvidence,
    pub demand: [u8; 32],
    pub choice: Option<PoolCostChoiceBasis>,
    pub candidates: Vec<PoolCostCandidateAccount>,
    pub sources: Vec<PoolCostSourceReceipt>,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostAccountRefusal {
    Extent,
    Correlation,
    InconsistentDecision,
    InvalidSource,
}
impl PoolCostSelectionAccount {
    /// Check historical correlation and numeric/source custody, never rerank a
    /// decision or reconstruct current capacity from a prediction.
    pub fn validate(&self, plan: &Plan) -> Result<(), PoolCostAccountRefusal> {
        use PoolCostAccountRefusal as R;
        if self.candidates.len() > MAXIMUM_PREPARED_POOL_COSTS
            || self.sources.len() > MAXIMUM_PREPARED_POOL_COSTS
            || self.evidence.observation_sign_ids.len() > 448
            || self
                .evidence
                .observation_sign_ids
                .iter()
                .any(|s| s.as_str().is_empty() || s.as_str().len() > 256)
        {
            return Err(R::Extent);
        }
        if [
            self.evidence.operation_id.as_str(),
            self.evidence.sign_id.as_str(),
        ]
        .iter()
        .any(|id| id.len() > 256)
        {
            return Err(R::Extent);
        }
        self.evidence.validate(plan).map_err(|_| R::Correlation)?;
        let pool = plan
            .fragments
            .first()
            .and_then(|f| {
                f.shared_pools
                    .iter()
                    .find(|p| p.pool_id == self.evidence.pool_id)
            })
            .ok_or(R::Correlation)?;
        if pool.selection_policy
            != SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity
            || self.candidates.len() != pool.realization_envelope.len()
        {
            return Err(R::Correlation);
        }
        for (i, source) in self.sources.iter().enumerate() {
            if self.sources[..i]
                .iter()
                .any(|s| s.realization == source.realization)
            {
                return Err(R::Correlation);
            }
            let member = pool
                .realization_envelope
                .get(usize::from(source.realization))
                .ok_or(R::Correlation)?;
            if source.host != member.host_id
                || source.boot != member.boot_id
                || source.generation != member.offer_generation
                || source.capability != member.capability_id
                || source.implementation != member.implementation_id
                || source.artifact != member.artifact_id
                || source.resource.as_ref().is_some_and(|(pool, class)| {
                    !member
                        .resources
                        .iter()
                        .any(|r| &r.pool_id == pool && &r.class_id == class)
                })
            {
                return Err(R::Correlation);
            }
            if [
                source.host_profile.as_str(),
                source.kind.as_str(),
                source.kind_contract.as_str(),
                source.execution_profile.as_str(),
            ]
            .iter()
            .any(|id| id.is_empty() || id.len() > 256)
                || source.resource.as_ref().is_some_and(|(pool, class)| {
                    [pool.as_str(), class.as_str()]
                        .iter()
                        .any(|id| id.is_empty() || id.len() > 256)
                })
            {
                return Err(R::InvalidSource);
            }
            if source.supporting_signs.len() > 4 {
                return Err(R::Extent);
            }
            if source.sign.as_str().is_empty()
                || source.sign.as_str().len() > 256
                || source
                    .supporting_signs
                    .iter()
                    .any(|s| s.as_str().is_empty() || s.as_str().len() > 256)
                || source.observed.validate().is_err()
                || source.expires.validate().is_err()
                || source.observed.clock().host_id() != &source.host
                || source.observed.clock().boot_id() != &source.boot
                || source.observed.clock() != source.expires.clock()
                || source.observed.ticks() >= source.expires.ticks()
            {
                return Err(R::InvalidSource);
            }
            if !self.evidence.observation_sign_ids.contains(&source.sign)
                || source
                    .supporting_signs
                    .iter()
                    .any(|s| !self.evidence.observation_sign_ids.contains(s))
            {
                return Err(R::Correlation);
            }
            if source
                .expected_execution
                .is_some_and(|c| cost_dependencies::validate_condition(c).is_err())
                || (matches!(
                    source.availability,
                    PoolCostSourceAvailability::Unavailable(_)
                ) && source.current_execution.is_some())
            {
                return Err(R::InvalidSource);
            }
            if source.availability == PoolCostSourceAvailability::Known {
                let now = source.evaluated_at.as_ref().ok_or(R::InvalidSource)?;
                if source.evaluated_calibration.as_ref() != Some(&source.calibration)
                    || cost_preparation::validate_window_source(
                        &source.host,
                        &source.boot,
                        &source.observed,
                        &source.expires,
                        now,
                    )
                    .is_err()
                {
                    return Err(R::InvalidSource);
                }
                match (source.expected_execution, &source.current_execution) {
                    (None, None) => {}
                    (Some(expected), Some(current))
                        if current.condition == expected
                            && current.calibration == source.calibration
                            && !current.sign.as_str().is_empty()
                            && current.sign.as_str().len() <= 256
                            && self.evidence.observation_sign_ids.contains(&current.sign)
                            && cost_preparation::validate_window_source(
                                &source.host,
                                &source.boot,
                                &current.observed,
                                &current.expires,
                                now,
                            )
                            .is_ok() => {}
                    _ => return Err(R::InvalidSource),
                }
            }
        }
        for (i, candidate) in self.candidates.iter().enumerate() {
            if usize::from(candidate.realization) != i {
                return Err(R::Correlation);
            }
            let source = self
                .sources
                .iter()
                .enumerate()
                .find(|(_, s)| s.realization == candidate.realization);
            let expected = if candidate.admission == PoolCostHardDisposition::NotCurrent {
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
            if candidate.cost != expected {
                return Err(R::InconsistentDecision);
            }
        }
        match self.evidence.selected_realization {
            None if self.choice.is_none() => {}
            Some(index) => {
                let candidate = self
                    .candidates
                    .get(usize::from(index))
                    .ok_or(R::Correlation)?;
                if candidate.admission != PoolCostHardDisposition::Eligible {
                    return Err(R::InconsistentDecision);
                }
                let expected = match candidate.cost {
                    PoolCostDisposition::Known { .. } => {
                        PoolCostChoiceBasis::MinimumPredictedCompletion
                    }
                    _ => PoolCostChoiceBasis::CapacityReference,
                };
                if self.choice != Some(expected) {
                    return Err(R::InconsistentDecision);
                }
            }
            _ => return Err(R::InconsistentDecision),
        }
        Ok(())
    }
}
