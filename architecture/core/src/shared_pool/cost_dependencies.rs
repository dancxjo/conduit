//! Independently current dependencies of a producer-prepared pool cost.
//! These borrowed observations are supplied by the admitted producer boundary;
//! checking their scope neither authenticates a producer nor grants capacity.
use super::{
    cost_preparation::validate_window, PoolCostPreparationRefusal as Refusal, PreparedPoolCost,
};
use crate::{
    ArtifactId, CapabilityOffer, HostAdvertisement, MonotonicInstant, ResourceOffer, SignId,
};

/// Producer-admitted execution condition, never a request preference.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CostExecutionCondition {
    Unknown,
    Cold,
    /// Positive residency within a sampled window, without a continuous epoch.
    ResidentObserved,
    Warm {
        residency_revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCostCalibrationRevision {
    identity: ArtifactId,
    revision: u64,
}
impl PoolCostCalibrationRevision {
    pub fn new(identity: ArtifactId, revision: u64) -> Result<Self, Refusal> {
        if identity.as_str().is_empty() || identity.as_str().len() > 256 || revision == 0 {
            return Err(Refusal::InvalidIdentity);
        }
        Ok(Self { identity, revision })
    }
    pub fn identity(&self) -> &ArtifactId {
        &self.identity
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// A separate current execution sample, not the forecast's historical assumption.
pub struct PoolCostExecutionEvaluation<'a> {
    pub host: &'a HostAdvertisement,
    pub offer: &'a CapabilityOffer,
    pub resource: Option<&'a ResourceOffer>,
    pub calibration: &'a PoolCostCalibrationRevision,
    pub sign: &'a SignId,
    pub observed: &'a MonotonicInstant,
    pub expires: &'a MonotonicInstant,
    pub condition: CostExecutionCondition,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCostExecutionReceipt {
    pub sign: SignId,
    pub observed: MonotonicInstant,
    pub expires: MonotonicInstant,
    pub condition: CostExecutionCondition,
    pub calibration: PoolCostCalibrationRevision,
}
impl PoolCostExecutionEvaluation<'_> {
    pub(super) fn validate_for(
        &self,
        cost: &PreparedPoolCost<'_>,
        expected: CostExecutionCondition,
        now: &MonotonicInstant,
    ) -> Result<(), Refusal> {
        if self.host != cost.host()
            || self.offer != cost.offer()
            || self.resource != cost.resource()
        {
            return Err(Refusal::SourceChanged);
        }
        if self.calibration != cost.calibration() {
            return Err(Refusal::CalibrationChanged);
        }
        if self.sign.as_str().is_empty() || self.sign.as_str().len() > 256 {
            return Err(Refusal::InvalidIdentity);
        }
        validate_window(cost.host(), self.observed, self.expires, now)?;
        validate_condition(self.condition)?;
        if self.condition != expected {
            return Err(Refusal::ExecutionConditionChanged);
        }
        Ok(())
    }
}

pub(super) fn validate_condition(condition: CostExecutionCondition) -> Result<(), Refusal> {
    if matches!(
        condition,
        CostExecutionCondition::Warm {
            residency_revision: 0
        }
    ) {
        return Err(Refusal::InvalidIdentity);
    }
    Ok(())
}
