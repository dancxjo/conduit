//! Finite renderer-neutral projections for learned and dynamical runtime Watches.

use serde::{Deserialize, Serialize};

pub const MAX_LEARNED_WATCH_PROJECTIONS: usize = 8;
pub const MAX_SIGNAL_POINTS: usize = 96;
pub const MAX_TENSOR_AXES: usize = 8;
pub const MAX_TENSOR_SLICE_VALUES: usize = 32;
pub const MAX_PROBABILISTIC_ALTERNATIVES: usize = 8;
pub const MAX_OBJECTIVE_COMPONENTS: usize = 8;
const MAX_IDENTITY_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearnedWatchProjection {
    pub observation_sequence: u64,
    pub max_updates_per_second: u16,
    pub dropped_updates: u64,
    #[serde(flatten)]
    pub kind: LearnedWatchProjectionKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "projection", content = "detail")]
pub enum LearnedWatchProjectionKind {
    Tensor(TensorWatch),
    Signal(SignalWatch),
    Probabilistic(ProbabilisticWatch),
    State(StateWatch),
    Training(TrainingWatch),
    Dynamics(DynamicsWatch),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TensorAxis {
    pub role: String,
    pub unit: Option<String>,
    pub length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TensorWatch {
    pub dtype: String,
    pub shape: Vec<u32>,
    pub axes: Vec<TensorAxis>,
    pub total_bytes: u64,
    pub resource_identity: Option<String>,
    /// Fixed-point milli-units; statistics exist only when explicitly observed.
    pub statistics_milli: Option<[i64; 4]>,
    pub bounded_slice_milli: Vec<i64>,
    pub slice_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalStreamRole {
    AudioDerived,
    Articulatory,
    Latent,
    Metric,
    Sensor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum ClockAlignment {
    SourceClock,
    Related { relation_evidence: String },
    NotAligned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalContinuity {
    Continuous,
    Discontinuous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbabilisticDisposition {
    Observed,
    Inferred,
    Sampled,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalPoint {
    pub tick: i64,
    pub value_milli: Option<i64>,
    pub lower_milli: Option<i64>,
    pub upper_milli: Option<i64>,
    pub disposition: ProbabilisticDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalWatch {
    pub role: SignalStreamRole,
    pub channel: String,
    pub unit: String,
    pub clock_identity: String,
    pub start_tick: i64,
    pub ticks_per_second: u32,
    pub continuity: SignalContinuity,
    pub alignment: ClockAlignment,
    pub retained_history_bytes: u32,
    pub evicted_points: u64,
    pub points: Vec<SignalPoint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbabilisticAlternative {
    pub label: String,
    pub value_milli: i64,
    pub weight_millionths: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbabilisticWatch {
    pub disposition: ProbabilisticDisposition,
    pub mean_milli: i64,
    pub standard_deviation_milli: u64,
    pub alternatives: Vec<ProbabilisticAlternative>,
    pub sample_count: u32,
    pub seed_profile: String,
    pub approximation: String,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StateTransition {
    Pending,
    Committed,
    Reset,
    Cancelled,
    Refused,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateWatch {
    pub generation: u64,
    pub step: u64,
    pub value_identity: String,
    pub candidate_identity: Option<String>,
    pub transition: StateTransition,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrainingPhase {
    Training,
    Evaluation,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveComponent {
    pub name: String,
    pub value_milli: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingWatch {
    pub phase: TrainingPhase,
    pub split_identity: String,
    pub batch_identity: String,
    pub step: u64,
    pub work_units: u64,
    pub objectives: Vec<ObjectiveComponent>,
    pub total_loss_milli: i64,
    pub checkpoint_event: Option<String>,
    pub pressure: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicsWatch {
    pub clock_identity: String,
    pub start_tick: i64,
    pub end_tick: i64,
    pub initial_state_milli: Vec<i64>,
    pub final_state_milli: Vec<i64>,
    pub trajectory: Vec<SignalPoint>,
    pub solver_work: u64,
    pub tolerance_millionths: u32,
    pub estimated_error_millionths: u32,
    pub truncated: bool,
    pub refusal: Option<String>,
}

impl LearnedWatchProjection {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_updates_per_second == 0 {
            return Err("projection update rate must be bounded and nonzero");
        }
        match &self.kind {
            LearnedWatchProjectionKind::Tensor(value) => value.validate(),
            LearnedWatchProjectionKind::Signal(value) => value.validate(),
            LearnedWatchProjectionKind::Probabilistic(value) => value.validate(),
            LearnedWatchProjectionKind::State(value) => value.validate(),
            LearnedWatchProjectionKind::Training(value) => value.validate(),
            LearnedWatchProjectionKind::Dynamics(value) => value.validate(),
        }
    }

    pub(crate) fn same_slot(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (LearnedWatchProjectionKind::Tensor(_), LearnedWatchProjectionKind::Tensor(_))
            | (
                LearnedWatchProjectionKind::Probabilistic(_),
                LearnedWatchProjectionKind::Probabilistic(_),
            )
            | (LearnedWatchProjectionKind::State(_), LearnedWatchProjectionKind::State(_))
            | (LearnedWatchProjectionKind::Training(_), LearnedWatchProjectionKind::Training(_))
            | (LearnedWatchProjectionKind::Dynamics(_), LearnedWatchProjectionKind::Dynamics(_)) => {
                true
            }
            (
                LearnedWatchProjectionKind::Signal(left),
                LearnedWatchProjectionKind::Signal(right),
            ) => left.role == right.role && left.channel == right.channel,
            _ => false,
        }
    }
}

fn identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_IDENTITY_BYTES
}

impl TensorWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if !identity(&self.dtype)
            || self.shape.is_empty()
            || self.shape.len() > MAX_TENSOR_AXES
            || self.axes.len() != self.shape.len()
            || self.bounded_slice_milli.len() > MAX_TENSOR_SLICE_VALUES
            || self.axes.iter().zip(&self.shape).any(|(axis, length)| {
                !identity(&axis.role)
                    || axis.unit.as_deref().is_some_and(|unit| !identity(unit))
                    || axis.length != *length
            })
            || self
                .resource_identity
                .as_deref()
                .is_some_and(|value| !identity(value))
        {
            return Err("invalid bounded tensor projection");
        }
        Ok(())
    }
}

impl SignalWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if !identity(&self.channel)
            || !identity(&self.unit)
            || !identity(&self.clock_identity)
            || self.ticks_per_second == 0
            || self.points.len() > MAX_SIGNAL_POINTS
            || matches!(&self.alignment, ClockAlignment::Related { relation_evidence } if !identity(relation_evidence))
            || invalid_points(&self.points)
        {
            return Err("invalid bounded signal projection");
        }
        Ok(())
    }
}

impl ProbabilisticWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if matches!(
            self.disposition,
            ProbabilisticDisposition::Observed | ProbabilisticDisposition::Missing
        ) || self.alternatives.len() > MAX_PROBABILISTIC_ALTERNATIVES
            || !identity(&self.seed_profile)
            || !identity(&self.approximation)
            || self
                .alternatives
                .iter()
                .any(|item| !identity(&item.label) || item.weight_millionths > 1_000_000)
        {
            return Err("invalid probabilistic projection");
        }
        Ok(())
    }
}

impl StateWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if !identity(&self.value_identity)
            || !identity(&self.summary)
            || self
                .candidate_identity
                .as_deref()
                .is_some_and(|value| !identity(value))
        {
            return Err("invalid state projection");
        }
        Ok(())
    }
}

impl TrainingWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if !identity(&self.split_identity)
            || !identity(&self.batch_identity)
            || self.objectives.is_empty()
            || self.objectives.len() > MAX_OBJECTIVE_COMPONENTS
            || self.objectives.iter().any(|item| !identity(&item.name))
            || self
                .checkpoint_event
                .as_deref()
                .is_some_and(|value| !identity(value))
            || self
                .pressure
                .as_deref()
                .is_some_and(|value| !identity(value))
        {
            return Err("invalid training projection");
        }
        Ok(())
    }
}

impl DynamicsWatch {
    fn validate(&self) -> Result<(), &'static str> {
        if !identity(&self.clock_identity)
            || self.end_tick < self.start_tick
            || self.initial_state_milli.is_empty()
            || self.initial_state_milli.len() > MAX_TENSOR_SLICE_VALUES
            || self.initial_state_milli.len() != self.final_state_milli.len()
            || self.trajectory.len() > MAX_SIGNAL_POINTS
            || invalid_points(&self.trajectory)
            || self
                .refusal
                .as_deref()
                .is_some_and(|value| !identity(value))
        {
            return Err("invalid dynamics projection");
        }
        Ok(())
    }
}

fn invalid_points(points: &[SignalPoint]) -> bool {
    points.iter().any(|point| {
        point.value_milli.is_none()
            != matches!(point.disposition, ProbabilisticDisposition::Missing)
            || point
                .lower_milli
                .zip(point.upper_milli)
                .is_some_and(|(low, high)| low > high)
            || point.lower_milli.is_some() != point.upper_milli.is_some()
    })
}
