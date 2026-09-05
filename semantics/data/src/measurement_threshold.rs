//! Presentation-independent typed threshold and hysteresis decisions.

use conduit_core::{Quantity, TemporalInstant};

use crate::MeasurementSummary;

pub const MEASUREMENT_THRESHOLD_POLICY_INFO_ID: &str = "data/measurement-threshold-policy@1";
pub const MEASUREMENT_THRESHOLD_DECISION_INFO_ID: &str = "data/measurement-threshold-decision@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MeasurementThresholdPolicy {
    pub lower: Quantity,
    pub upper: Quantity,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementThresholdState {
    Below,
    Above,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementThresholdTransition {
    RoseAbove,
    FellBelow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementThresholdDecision {
    pub state: MeasurementThresholdState,
    pub transition: Option<MeasurementThresholdTransition>,
    pub evaluated_value: Quantity,
    pub first_observed_at: TemporalInstant,
    pub last_observed_at: TemporalInstant,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementThresholdRefusal {
    PolicyUnitMismatch,
    InvalidPolicyOrder,
    SummaryUnitMismatch,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MeasurementHysteresis {
    policy: MeasurementThresholdPolicy,
    state: MeasurementThresholdState,
}

impl MeasurementHysteresis {
    pub fn new(
        policy: MeasurementThresholdPolicy,
        initial_state: MeasurementThresholdState,
    ) -> Result<Self, MeasurementThresholdRefusal> {
        if policy.lower.unit() != policy.upper.unit() {
            return Err(MeasurementThresholdRefusal::PolicyUnitMismatch);
        }
        if policy.lower.value() >= policy.upper.value() {
            return Err(MeasurementThresholdRefusal::InvalidPolicyOrder);
        }
        Ok(Self {
            policy,
            state: initial_state,
        })
    }

    pub fn evaluate(
        &mut self,
        summary: &MeasurementSummary,
    ) -> Result<MeasurementThresholdDecision, MeasurementThresholdRefusal> {
        if summary.mean.unit() != self.policy.lower.unit() {
            return Err(MeasurementThresholdRefusal::SummaryUnitMismatch);
        }
        let transition = match self.state {
            MeasurementThresholdState::Below
                if summary.mean.value() >= self.policy.upper.value() =>
            {
                self.state = MeasurementThresholdState::Above;
                Some(MeasurementThresholdTransition::RoseAbove)
            }
            MeasurementThresholdState::Above
                if summary.mean.value() <= self.policy.lower.value() =>
            {
                self.state = MeasurementThresholdState::Below;
                Some(MeasurementThresholdTransition::FellBelow)
            }
            _ => None,
        };
        Ok(MeasurementThresholdDecision {
            state: self.state,
            transition,
            evaluated_value: summary.mean,
            first_observed_at: summary.first_observed_at.clone(),
            last_observed_at: summary.last_observed_at.clone(),
        })
    }

    pub const fn state(&self) -> MeasurementThresholdState {
        self.state
    }

    pub const fn policy(&self) -> MeasurementThresholdPolicy {
        self.policy
    }
}
