//! Presentation-independent typed threshold and hysteresis decisions.

use crate::{
    MeasurementSummary, MeasurementThresholdDecision, MeasurementThresholdPolicy,
    MeasurementThresholdRefusal, MeasurementThresholdState, MeasurementThresholdTransition,
};

pub const MEASUREMENT_THRESHOLD_POLICY_INFO_ID: &str = "data/measurement-threshold-policy@1";
pub const MEASUREMENT_HYSTERESIS_PROFILE_INFO_ID: &str = "data/measurement-hysteresis-profile@1";
pub const MEASUREMENT_THRESHOLD_DECISION_INFO_ID: &str = "data/measurement-threshold-decision@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementHysteresis {
    policy: MeasurementThresholdPolicy,
    state: MeasurementThresholdState,
}

impl MeasurementHysteresis {
    pub fn new(
        policy: MeasurementThresholdPolicy,
        initial_state: MeasurementThresholdState,
    ) -> Result<Self, MeasurementThresholdRefusal> {
        if !policy.lower().unit().same_physical_definition(policy.upper().unit()) {
            return Err(MeasurementThresholdRefusal::PolicyUnitMismatch);
        }
        if policy
            .lower()
            .as_ref().compare_borrowed(policy.upper().as_ref())
            .map_err(|_| MeasurementThresholdRefusal::InvalidPolicyOrder)?
            .ok_or(MeasurementThresholdRefusal::InvalidPolicyOrder)? != core::cmp::Ordering::Less
        {
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
        if !summary.mean.unit().same_physical_definition(self.policy.lower().unit()) {
            return Err(MeasurementThresholdRefusal::SummaryUnitMismatch);
        }
        let transition = match self.state {
            MeasurementThresholdState::Below
                if summary
                    .mean
                    .as_ref().compare_borrowed(self.policy.upper().as_ref())
                    .map_err(|_| MeasurementThresholdRefusal::SummaryUnitMismatch)?
                    .ok_or(MeasurementThresholdRefusal::InvalidPolicyOrder)? != core::cmp::Ordering::Less =>
            {
                self.state = MeasurementThresholdState::Above;
                Some(MeasurementThresholdTransition::RoseAbove)
            }
            MeasurementThresholdState::Above
                if summary
                    .mean
                    .as_ref().compare_borrowed(self.policy.lower().as_ref())
                    .map_err(|_| MeasurementThresholdRefusal::SummaryUnitMismatch)?
                    .ok_or(MeasurementThresholdRefusal::SummaryUnitMismatch)? != core::cmp::Ordering::Greater =>
            {
                self.state = MeasurementThresholdState::Below;
                Some(MeasurementThresholdTransition::FellBelow)
            }
            _ => None,
        };
        Ok(MeasurementThresholdDecision {
            state: self.state,
            transition,
            evaluated_value: summary.mean.clone(),
            first_observed_at: summary.first_observed_at.clone(),
            last_observed_at: summary.last_observed_at.clone(),
        })
    }

    pub const fn state(&self) -> MeasurementThresholdState {
        self.state
    }

    pub const fn policy(&self) -> &MeasurementThresholdPolicy {
        &self.policy
    }
}
