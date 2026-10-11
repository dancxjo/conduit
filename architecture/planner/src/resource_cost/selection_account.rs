//! Bounded historical accounts for the ordinary cost selector.
//! These records retain attribution, not live authority or executable choices.
use super::{CostEvidenceRefusal as Refusal, CostedForecast};
use crate::{PerformanceIntent, PlannerError, RealizationSelection};
use alloc::vec::Vec;
use conduit_core::{CapabilityId, ExactRational, HostId, MonotonicInstant};

#[derive(Debug)]
pub enum CostAvailability<'a> {
    Known(&'a CostedForecast<'a>),
    Missing,
    Unusable(Refusal),
}
pub struct CostCandidateEvidence<'a> {
    pub host_id: &'a HostId,
    pub capability_id: &'a CapabilityId,
    pub availability: CostAvailability<'a>,
    /// Retain attribution even when freshness/profile validation refuses use.
    /// This is historical custody, not proof that the sample remains usable.
    pub submitted_source: Option<&'a super::CostEvidenceSource<'a>>,
    pub submitted_calibration_source: Option<&'a super::CostEvidenceSource<'a>>,
    pub evaluated_at: &'a MonotonicInstant,
    pub current_execution: Option<&'a super::CostExecutionObservation<'a>>,
    pub submitted_execution_assumption: Option<&'a super::CostEvidenceSource<'a>>,
    pub expected_execution_condition: Option<super::CostExecutionCondition>,
    pub selected: bool,
}
impl core::fmt::Debug for CostCandidateEvidence<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CostCandidateEvidence")
            .field("host", &self.host_id)
            .field("capability", &self.capability_id)
            .field("availability", &self.availability)
            .field(
                "submitted_sign",
                &self.submitted_source.map(|source| source.sign()),
            )
            .field(
                "submitted_calibration_sign",
                &self
                    .submitted_calibration_source
                    .map(|source| source.sign()),
            )
            .field("evaluated_at", &self.evaluated_at)
            .field("selected", &self.selected)
            .finish()
    }
}
#[derive(Debug)]
pub struct CostRealizationSelection<'a> {
    /// All original hard refusals are retained. Exactly one admitted record is
    /// selected after cost preference; costs never change a rejected record.
    pub realization: RealizationSelection,
    pub intent: PerformanceIntent,
    pub costs: Vec<CostCandidateEvidence<'a>>,
    pub used_cost_preference: bool,
    /// Conservative admitted limb work, not empirical scheduling wall time.
    pub arithmetic_work_spent: u64,
}
#[derive(Debug)]
pub struct UnavailableCostEvidence {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
    /// None denotes missing evidence, not a numeric zero.
    pub reason: Option<Refusal>,
    pub submitted: Option<super::CostSourceAttribution>,
    pub submitted_calibration: Option<super::CostSourceAttribution>,
    pub current_execution: Option<(super::CostSourceAttribution, super::CostExecutionCondition)>,
    pub submitted_execution_assumption: Option<super::CostSourceAttribution>,
    pub expected_execution_condition: Option<super::CostExecutionCondition>,
    pub evaluated_at: MonotonicInstant,
}
#[derive(Debug)]
pub enum CostSelectionError {
    Planner(PlannerError),
    Evidence(Refusal),
    HardAdmission(crate::RealizationSelectionRefusal),
    Policy(alloc::boxed::Box<CostPolicyRefusal>),
    Comparison(alloc::boxed::Box<CostComparisonRefusal>),
    NoUsableForecast {
        realization: RealizationSelection,
        costs: Vec<UnavailableCostEvidence>,
    },
}
impl From<PlannerError> for CostSelectionError {
    fn from(value: PlannerError) -> Self {
        Self::Planner(value)
    }
}
impl From<Refusal> for CostSelectionError {
    fn from(value: Refusal) -> Self {
        Self::Evidence(value)
    }
}

/// The two decisive finite model coordinates at a refused exact comparison.
/// Source metadata is owned without retaining inputs or Host inventories.
#[derive(Debug)]
pub struct CostComparisonOperand {
    pub source: super::CostSourceAttribution,
    pub calibration: Option<super::CostSourceAttribution>,
    pub upper_seconds: ExactRational,
    pub evaluated_at: MonotonicInstant,
    pub current_execution: Option<(super::CostSourceAttribution, super::CostExecutionCondition)>,
    pub execution_assumption: Option<super::CostSourceAttribution>,
    pub expected_condition: Option<super::CostExecutionCondition>,
}
impl CostComparisonOperand {
    pub(super) fn retain(
        candidate: &CostCandidateEvidence<'_>,
        forecast: &CostedForecast<'_>,
    ) -> Self {
        Self {
            source: super::CostSourceAttribution::from_source(forecast.source()),
            calibration: candidate
                .submitted_calibration_source
                .map(super::CostSourceAttribution::from_source),
            upper_seconds: forecast.upper_seconds(),
            evaluated_at: candidate.evaluated_at.clone(),
            current_execution: candidate.current_execution.map(|sample| {
                (
                    super::CostSourceAttribution::from_source(sample.source()),
                    sample.condition(),
                )
            }),
            execution_assumption: candidate
                .submitted_execution_assumption
                .map(super::CostSourceAttribution::from_source),
            expected_condition: candidate.expected_execution_condition,
        }
    }
}
#[derive(Debug)]
pub struct CostComparisonRefusal {
    /// The ordinary admission account; its tentative choice did not survive
    /// cost comparison and is never an executable cost-policy decision.
    pub realization: RealizationSelection,
    pub reason: Refusal,
    pub intent: PerformanceIntent,
    pub operands: [CostComparisonOperand; 2],
    pub arithmetic_work_limit: u64,
    pub arithmetic_work_spent: u64,
    pub arithmetic_work_remaining: u64,
}

/// Ordinary hard-admission evidence retained when no cost policy can finish.
/// Its tentative ordinary choice is not an executable cost-policy winner.
#[derive(Debug)]
pub struct CostPolicyRefusal {
    pub realization: RealizationSelection,
    pub intent: PerformanceIntent,
    pub reason: Refusal,
}
