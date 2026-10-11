//! Producer-prepared cost custody for one exact sealed pool operation.
//! This boundary checks structure and numeric realization, not producer trust,
//! resource possession or authorization. Ordinary hard admission remains required.
use super::{
    CostExecutionCondition, PoolCostCalibrationRevision, PoolCostExecutionEvaluation,
    PoolCostExecutionReceipt,
};
use super::{PoolDurationPreparationRefusal, PoolOperationId, PreparedPoolDuration, SharedPoolId};
use crate::{
    CapabilityOffer, ExactNumericWork, HostAdvertisement, MonotonicInstant, Plan, PlanId,
    RationalQuantity, ResourceOffer, SignId,
};
use alloc::vec::Vec;

pub const MAXIMUM_PREPARED_POOL_COSTS: usize = 32;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolCostPreparationRefusal {
    InvalidPlan,
    PoolMissing,
    InvalidIdentity,
    InputLimit,
    RealizationMissing,
    SourceChanged,
    IncompatibleFront,
    ClockChanged,
    InvalidWindow,
    Future,
    Stale,
    CalibrationChanged,
    MissingExecutionObservation,
    AmbiguousExecutionObservation,
    ExecutionConditionChanged,
    Duration(PoolDurationPreparationRefusal),
}

/// The admitted producer supplies demand-specific relative Time and current
/// custody. Additional calibration/execution sources must constrain this window
/// at the producer boundary; historical assumptions do not certify freshness.
pub struct PoolCostPreparation<'a, 'custody> {
    pub plan: &'custody Plan,
    pub fragment_index: usize,
    pub pool: &'custody SharedPoolId,
    pub operation: &'custody PoolOperationId,
    pub demand: [u8; 32],
    pub realization: u16,
    pub host: &'a HostAdvertisement,
    pub offer: &'a CapabilityOffer,
    pub resource: Option<&'a ResourceOffer>,
    pub calibration: &'custody PoolCostCalibrationRevision,
    pub expected_execution: Option<CostExecutionCondition>,
    pub sign: &'custody SignId,
    pub supporting_signs: &'custody [SignId],
    pub observed: &'custody MonotonicInstant,
    pub expires: &'custody MonotonicInstant,
    pub evaluated_at: &'custody MonotonicInstant,
    pub duration: RationalQuantity,
    pub tolerance: RationalQuantity,
}

/// Private preparation binds the checked projection to the exact Plan/member
/// and source snapshot. It is neither a mutable Plan nor an authorization grant.
pub struct PreparedPoolCost<'a> {
    plan: PlanId,
    fragment_index: usize,
    pool: SharedPoolId,
    operation: PoolOperationId,
    demand: [u8; 32],
    realization: u16,
    host: &'a HostAdvertisement,
    offer: &'a CapabilityOffer,
    resource: Option<&'a ResourceOffer>,
    calibration: PoolCostCalibrationRevision,
    expected_execution: Option<CostExecutionCondition>,
    sign: SignId,
    supporting_signs: Vec<SignId>,
    observed: MonotonicInstant,
    expires: MonotonicInstant,
    duration: PreparedPoolDuration,
}
impl<'a> PreparedPoolCost<'a> {
    pub fn prepare(
        input: PoolCostPreparation<'a, '_>,
        work: &mut ExactNumericWork,
    ) -> Result<Self, PoolCostPreparationRefusal> {
        use PoolCostPreparationRefusal as R;
        if input.supporting_signs.len() > 4 {
            return Err(R::InputLimit);
        }
        for (index, sign) in input.supporting_signs.iter().enumerate() {
            if sign.as_str().is_empty() || sign.as_str().len() > 256 {
                return Err(R::InvalidIdentity);
            }
            if sign == input.sign || input.supporting_signs[..index].contains(sign) {
                return Err(R::InvalidIdentity);
            }
        }
        if !crate::verify_plan(input.plan) {
            return Err(R::InvalidPlan);
        }
        if let Some(condition) = input.expected_execution {
            super::cost_dependencies::validate_condition(condition)?;
        }
        for id in [
            input.pool.as_str(),
            input.operation.as_str(),
            input.sign.as_str(),
            input.host.host_id.as_str(),
            input.host.boot_id.as_str(),
            input.host.profile.as_str(),
            input.offer.capability_id.as_str(),
            input.offer.kind_id.as_str(),
            input.offer.kind_contract_revision.as_str(),
            input.offer.implementation.execution_profile_id.as_str(),
            input.offer.implementation.implementation_id.as_str(),
            input.offer.implementation.artifact_id.as_str(),
        ] {
            if id.is_empty() || id.len() > 256 {
                return Err(R::InvalidIdentity);
            }
        }
        if input.host.capabilities.len() > 256
            || input.host.resources.len() > 256
            || input.offer.resource_requirements.len() > 32
        {
            return Err(R::InputLimit);
        }
        let fragment = input
            .plan
            .fragments
            .get(input.fragment_index)
            .ok_or(R::PoolMissing)?;
        let mut pools = fragment
            .shared_pools
            .iter()
            .filter(|pool| &pool.pool_id == input.pool);
        let pool = pools.next().ok_or(R::PoolMissing)?;
        if pools.next().is_some() {
            return Err(R::PoolMissing);
        }
        if pool.realization_envelope.len() > MAXIMUM_PREPARED_POOL_COSTS {
            return Err(R::InputLimit);
        }
        let realization = pool
            .realization_envelope
            .get(usize::from(input.realization))
            .ok_or(R::RealizationMissing)?;
        if realization.host_id != input.host.host_id
            || realization.boot_id != input.host.boot_id
            || realization.offer_generation != input.host.offer_generation
            || realization.capability_id != input.offer.capability_id
            || realization.implementation_id != input.offer.implementation.implementation_id
            || realization.artifact_id != input.offer.implementation.artifact_id
        {
            return Err(R::SourceChanged);
        }
        let mut offers = input
            .host
            .capabilities
            .iter()
            .filter(|offer| offer.capability_id == input.offer.capability_id);
        if offers.next() != Some(input.offer) || offers.next().is_some() {
            return Err(R::SourceChanged);
        }
        if !pool.permits_realization(realization, &input.offer.checked_front()) {
            return Err(R::IncompatibleFront);
        }
        if let Some(resource) = input.resource {
            for id in [resource.pool_id.as_str(), resource.class_id.as_str()] {
                if id.is_empty() || id.len() > 256 {
                    return Err(R::InvalidIdentity);
                }
            }
            if !input.host.resources.contains(resource)
                || !input
                    .offer
                    .resource_requirements
                    .iter()
                    .any(|need| need.class_id == resource.class_id)
                || !realization.resources.iter().any(|binding| {
                    binding.pool_id == resource.pool_id && binding.class_id == resource.class_id
                })
            {
                return Err(R::SourceChanged);
            }
        }
        validate_window(
            input.host,
            input.observed,
            input.expires,
            input.evaluated_at,
        )?;
        let duration = PreparedPoolDuration::prepare(input.duration, input.tolerance, work)
            .map_err(R::Duration)?;
        Ok(Self {
            plan: input.plan.plan_id.clone(),
            fragment_index: input.fragment_index,
            pool: input.pool.clone(),
            operation: input.operation.clone(),
            demand: input.demand,
            realization: input.realization,
            host: input.host,
            offer: input.offer,
            resource: input.resource,
            calibration: input.calibration.clone(),
            expected_execution: input.expected_execution,
            sign: input.sign.clone(),
            supporting_signs: input.supporting_signs.to_vec(),
            observed: input.observed.clone(),
            expires: input.expires.clone(),
            duration,
        })
    }
    /// Revalidate the entire owner snapshot and exact local clock before reuse.
    /// A same-generation profile/Offer mutation is not accepted as old evidence.
    pub fn validate_at(
        &self,
        current: &HostAdvertisement,
        now: &MonotonicInstant,
    ) -> Result<(), PoolCostPreparationRefusal> {
        if current != self.host {
            return Err(PoolCostPreparationRefusal::SourceChanged);
        }
        validate_window(current, &self.observed, &self.expires, now)
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plan
    }
    pub const fn fragment_index(&self) -> usize {
        self.fragment_index
    }
    pub fn pool_id(&self) -> &SharedPoolId {
        &self.pool
    }
    pub fn operation_id(&self) -> &PoolOperationId {
        &self.operation
    }
    pub const fn demand(&self) -> [u8; 32] {
        self.demand
    }
    pub const fn realization(&self) -> u16 {
        self.realization
    }
    pub fn host(&self) -> &'a HostAdvertisement {
        self.host
    }
    pub fn offer(&self) -> &'a CapabilityOffer {
        self.offer
    }
    pub fn resource(&self) -> Option<&'a ResourceOffer> {
        self.resource
    }
    pub fn calibration(&self) -> &PoolCostCalibrationRevision {
        &self.calibration
    }
    pub const fn expected_execution(&self) -> Option<CostExecutionCondition> {
        self.expected_execution
    }
    /// Independent catalogue/state samples fence cached projections at admission.
    pub fn validate_dependencies(
        &self,
        calibration: &PoolCostCalibrationRevision,
        executions: &[PoolCostExecutionEvaluation<'_>],
        now: &MonotonicInstant,
    ) -> Result<Option<PoolCostExecutionReceipt>, PoolCostPreparationRefusal> {
        use PoolCostPreparationRefusal as R;
        if executions.len() > MAXIMUM_PREPARED_POOL_COSTS {
            return Err(R::InputLimit);
        }
        if calibration != &self.calibration {
            return Err(R::CalibrationChanged);
        }
        let Some(expected) = self.expected_execution else {
            return Ok(None);
        };
        let mut matching = executions.iter().filter(|sample| {
            sample.host.host_id == self.host.host_id
                && sample.offer.capability_id == self.offer.capability_id
                && sample.resource.map(|r| &r.pool_id) == self.resource.map(|r| &r.pool_id)
        });
        let current = matching.next().ok_or(R::MissingExecutionObservation)?;
        if matching.next().is_some() {
            return Err(R::AmbiguousExecutionObservation);
        }
        current.validate_for(self, expected, now)?;
        Ok(Some(PoolCostExecutionReceipt {
            sign: current.sign.clone(),
            observed: current.observed.clone(),
            expires: current.expires.clone(),
            condition: current.condition,
            calibration: current.calibration.clone(),
        }))
    }
    pub fn sign(&self) -> &SignId {
        &self.sign
    }
    pub fn supporting_signs(&self) -> &[SignId] {
        &self.supporting_signs
    }
    pub fn observed(&self) -> &MonotonicInstant {
        &self.observed
    }
    pub fn expires(&self) -> &MonotonicInstant {
        &self.expires
    }
    pub const fn duration(&self) -> PreparedPoolDuration {
        self.duration
    }
}
pub(super) fn validate_window(
    host: &HostAdvertisement,
    observed: &MonotonicInstant,
    expires: &MonotonicInstant,
    now: &MonotonicInstant,
) -> Result<(), PoolCostPreparationRefusal> {
    use PoolCostPreparationRefusal as R;
    if observed.validate().is_err()
        || expires.validate().is_err()
        || now.validate().is_err()
        || observed.clock().host_id() != &host.host_id
        || observed.clock().boot_id() != &host.boot_id
        || observed.clock() != expires.clock()
        || observed.clock() != now.clock()
    {
        return Err(R::ClockChanged);
    }
    if observed.ticks() >= expires.ticks() {
        return Err(R::InvalidWindow);
    }
    if now.ticks() < observed.ticks() {
        return Err(R::Future);
    }
    if now.ticks() >= expires.ticks() {
        return Err(R::Stale);
    }
    Ok(())
}
