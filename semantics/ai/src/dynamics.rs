//! Provider-neutral, bounded continuous-state evolution semantics.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{PlannedStateBoundary, QuantityUnit, StateContinuation};
use conduit_data::{
    tensor_content_digest, SampledSignal, SignalCadence, SignalStart, TensorBacking, TensorElement,
    TensorValue,
};

use crate::RandomnessProfile;

pub const MAXIMUM_DYNAMICS_CONTEXTS: usize = 32;
pub const MAXIMUM_DYNAMICS_SAMPLES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicsProfile {
    DeterministicOde,
    Stochastic {
        profile: String,
        randomness: RandomnessProfile,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct IntegrationInterval {
    pub start: i64,
    pub end: i64,
    pub unit: QuantityUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSamplingGrid {
    pub clock_identity: String,
    /// Exact requested observation coordinates. These are independent of
    /// solver-internal adaptive steps.
    pub coordinates: Vec<i64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct IntegrationAccuracy {
    pub absolute_tolerance_millionths: u64,
    pub relative_tolerance_millionths: u64,
    pub maximum_estimated_error_millionths: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct IntegrationResourceEnvelope {
    pub maximum_state_bytes: u64,
    pub maximum_context_bytes: u64,
    pub maximum_output_samples: u32,
    pub maximum_output_bytes: u64,
    pub maximum_internal_steps: u64,
    pub maximum_function_evaluations: u64,
    pub maximum_work_units: u64,
    pub memory_ceiling_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicsState {
    pub identity: String,
    pub schema_version: u32,
    pub generation: u64,
    pub value: TensorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrateContract {
    pub identity: [u8; 32],
    pub vector_field_artifact_identity: [u8; 32],
    pub interval: IntegrationInterval,
    pub sampling: OutputSamplingGrid,
    pub profile: DynamicsProfile,
    pub accuracy: IntegrationAccuracy,
    pub resources: IntegrationResourceEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrateRequest {
    pub expected_generation: u64,
    pub initial_state: DynamicsState,
    pub context: Vec<TensorValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverRealization {
    pub implementation_identity: String,
    pub solver_family: String,
    pub adapter_name: String,
    pub adapter_version: String,
    pub runtime_build_identity: String,
    pub device_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationCandidate {
    pub trajectory: SampledSignal,
    pub final_state: TensorValue,
    pub internal_steps: u64,
    pub function_evaluations: u64,
    pub consumed_work_units: u64,
    pub estimated_error_millionths: u64,
    pub realization: SolverRealization,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IntegrationTerminal {
    WorkLimitExhausted,
    Cancelled,
    Discontinuity,
    ProviderLost,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostIntegrationTerminal {
    Candidate(Box<IntegrationCandidate>),
    NoCommit(IntegrationTerminal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationReceipt {
    pub contract_identity: [u8; 32],
    pub contract_descriptor_identity: [u8; 32],
    pub vector_field_artifact_identity: [u8; 32],
    pub prior_state_identity: String,
    pub prior_generation: u64,
    pub trajectory_identity: [u8; 32],
    pub final_state_identity: [u8; 32],
    pub internal_steps: u64,
    pub function_evaluations: u64,
    pub consumed_work_units: u64,
    pub estimated_error_millionths: u64,
    pub realization: SolverRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedIntegration {
    pub next_state: DynamicsState,
    pub trajectory: SampledSignal,
    pub receipt: IntegrationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationOutcome {
    Completed(Box<CompletedIntegration>),
    NotCommitted {
        terminal: IntegrationTerminal,
        retained_generation: u64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DynamicsRefusal {
    MissingIdentity,
    InvalidTime,
    InvalidSampling,
    UnsupportedStochasticProfile,
    InvalidAccuracy,
    InvalidResources,
    InvalidState,
    StaleState,
    InvalidContext,
    ResourceBoundExceeded,
    InvalidTrajectory,
    InvalidFinalState,
    WorkBoundExceeded,
    InvalidRealization,
}

impl IntegrateContract {
    pub fn validate(&self) -> Result<(), DynamicsRefusal> {
        nonzero(self.identity)?;
        nonzero(self.vector_field_artifact_identity)?;
        self.interval.validate()?;
        self.sampling.validate_for(self.interval, self.resources)?;
        if !matches!(self.profile, DynamicsProfile::DeterministicOde) {
            return Err(DynamicsRefusal::UnsupportedStochasticProfile);
        }
        self.accuracy.validate()?;
        self.resources.validate()
    }

    pub fn planned_state_boundary(
        &self,
        state: &DynamicsState,
    ) -> Result<PlannedStateBoundary, DynamicsRefusal> {
        self.validate()?;
        state.validate(self.resources.maximum_state_bytes)?;
        let maximum_value_bytes = u32::try_from(self.resources.maximum_state_bytes)
            .map_err(|_| DynamicsRefusal::InvalidResources)?;
        Ok(PlannedStateBoundary {
            state_id: state.identity.clone().into(),
            gear_id: "ai/integrate".into(),
            value_kind: "ai/dynamics-state@1".into(),
            initial_value: state.generation.to_le_bytes().to_vec(),
            retained: None,
            maximum_value_bytes,
            continuation: StateContinuation::MaximumTransitions(1),
        })
    }

    pub fn realize(
        &self,
        request: &IntegrateRequest,
        terminal: HostIntegrationTerminal,
    ) -> Result<IntegrationOutcome, DynamicsRefusal> {
        self.validate()?;
        request.validate_for(self)?;
        let candidate = match terminal {
            HostIntegrationTerminal::Candidate(candidate) => candidate,
            HostIntegrationTerminal::NoCommit(terminal) => {
                return Ok(IntegrationOutcome::NotCommitted {
                    terminal,
                    retained_generation: request.initial_state.generation,
                })
            }
        };
        candidate.validate_for(self, request)?;
        let trajectory_identity = candidate
            .trajectory
            .semantic_digest()
            .map_err(|_| DynamicsRefusal::InvalidTrajectory)?;
        let final_state_identity = candidate.final_state.content_digest;
        let next_state = DynamicsState {
            identity: request.initial_state.identity.clone(),
            schema_version: request.initial_state.schema_version,
            generation: request
                .initial_state
                .generation
                .checked_add(1)
                .ok_or(DynamicsRefusal::InvalidState)?,
            value: candidate.final_state,
        };
        let receipt = IntegrationReceipt {
            contract_identity: self.identity,
            contract_descriptor_identity: self.semantic_digest()?,
            vector_field_artifact_identity: self.vector_field_artifact_identity,
            prior_state_identity: request.initial_state.identity.clone(),
            prior_generation: request.initial_state.generation,
            trajectory_identity,
            final_state_identity,
            internal_steps: candidate.internal_steps,
            function_evaluations: candidate.function_evaluations,
            consumed_work_units: candidate.consumed_work_units,
            estimated_error_millionths: candidate.estimated_error_millionths,
            realization: candidate.realization,
        };
        Ok(IntegrationOutcome::Completed(Box::new(
            CompletedIntegration {
                next_state,
                trajectory: candidate.trajectory,
                receipt,
            },
        )))
    }
}

impl IntegrationInterval {
    fn validate(self) -> Result<(), DynamicsRefusal> {
        if self.start >= self.end || !time_unit(self.unit) {
            Err(DynamicsRefusal::InvalidTime)
        } else {
            Ok(())
        }
    }
}

impl OutputSamplingGrid {
    fn validate_for(
        &self,
        interval: IntegrationInterval,
        resources: IntegrationResourceEnvelope,
    ) -> Result<(), DynamicsRefusal> {
        text(&self.clock_identity)?;
        if self.coordinates.len() < 2
            || self.coordinates.len() > MAXIMUM_DYNAMICS_SAMPLES
            || self.coordinates.len() > resources.maximum_output_samples as usize
            || self.coordinates.first() != Some(&interval.start)
            || self.coordinates.last() != Some(&interval.end)
            || self.coordinates.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(DynamicsRefusal::InvalidSampling);
        }
        Ok(())
    }
}

impl IntegrationAccuracy {
    fn validate(self) -> Result<(), DynamicsRefusal> {
        if (self.absolute_tolerance_millionths == 0 && self.relative_tolerance_millionths == 0)
            || self.maximum_estimated_error_millionths == 0
        {
            Err(DynamicsRefusal::InvalidAccuracy)
        } else {
            Ok(())
        }
    }
}

impl IntegrationResourceEnvelope {
    fn validate(self) -> Result<(), DynamicsRefusal> {
        if self.maximum_state_bytes == 0
            || self.maximum_context_bytes == 0
            || self.maximum_output_samples < 2
            || self.maximum_output_samples as usize > MAXIMUM_DYNAMICS_SAMPLES
            || self.maximum_output_bytes == 0
            || self.maximum_internal_steps == 0
            || self.maximum_function_evaluations == 0
            || self.maximum_work_units == 0
            || self.memory_ceiling_bytes == 0
        {
            Err(DynamicsRefusal::InvalidResources)
        } else {
            Ok(())
        }
    }
}

impl DynamicsState {
    fn validate(&self, maximum_bytes: u64) -> Result<(), DynamicsRefusal> {
        text(&self.identity)?;
        self.value
            .validate()
            .map_err(|_| DynamicsRefusal::InvalidState)?;
        if self.schema_version == 0
            || self
                .value
                .byte_count()
                .map_err(|_| DynamicsRefusal::InvalidState)?
                > maximum_bytes
        {
            return Err(DynamicsRefusal::InvalidState);
        }
        Ok(())
    }
}

impl IntegrateRequest {
    fn validate_for(&self, contract: &IntegrateContract) -> Result<(), DynamicsRefusal> {
        self.initial_state
            .validate(contract.resources.maximum_state_bytes)?;
        if self.expected_generation != self.initial_state.generation {
            return Err(DynamicsRefusal::StaleState);
        }
        if self.context.len() > MAXIMUM_DYNAMICS_CONTEXTS {
            return Err(DynamicsRefusal::InvalidContext);
        }
        let mut bytes = 0_u64;
        for value in &self.context {
            value
                .validate()
                .map_err(|_| DynamicsRefusal::InvalidContext)?;
            bytes = bytes
                .checked_add(
                    value
                        .byte_count()
                        .map_err(|_| DynamicsRefusal::InvalidContext)?,
                )
                .ok_or(DynamicsRefusal::ResourceBoundExceeded)?;
        }
        if bytes > contract.resources.maximum_context_bytes {
            return Err(DynamicsRefusal::ResourceBoundExceeded);
        }
        Ok(())
    }
}

impl IntegrationCandidate {
    fn validate_for(
        &self,
        contract: &IntegrateContract,
        request: &IntegrateRequest,
    ) -> Result<(), DynamicsRefusal> {
        self.trajectory
            .validate()
            .map_err(|_| DynamicsRefusal::InvalidTrajectory)?;
        if self.trajectory.clock_identity != contract.sampling.clock_identity
            || self.trajectory.sample_count != contract.sampling.coordinates.len() as u64
            || !matches!(self.trajectory.start, SignalStart::SampleIndex(0))
        {
            return Err(DynamicsRefusal::InvalidTrajectory);
        }
        let SignalCadence::Irregular { coordinates } = &self.trajectory.cadence else {
            return Err(DynamicsRefusal::InvalidTrajectory);
        };
        let expected_coordinates = contract
            .sampling
            .coordinates
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        if coordinates.element != TensorElement::I64
            || coordinates.dimensions != [contract.sampling.coordinates.len() as u64]
            || coordinates.axes[0].unit != Some(contract.interval.unit)
            || coordinates.content_digest != tensor_content_digest(&expected_coordinates)
            || !matches!(&coordinates.backing, TensorBacking::Inline(bytes) if bytes == &expected_coordinates)
        {
            return Err(DynamicsRefusal::InvalidTrajectory);
        }
        let sample_bytes = self
            .trajectory
            .samples
            .byte_count()
            .map_err(|_| DynamicsRefusal::InvalidTrajectory)?;
        let coordinate_bytes = coordinates
            .byte_count()
            .map_err(|_| DynamicsRefusal::InvalidTrajectory)?;
        if self.trajectory.samples.element != request.initial_state.value.element
            || self.trajectory.samples.dimensions[1..] != request.initial_state.value.dimensions
            || self.trajectory.samples.axes[1..] != request.initial_state.value.axes
            || sample_bytes
                .checked_add(coordinate_bytes)
                .is_none_or(|bytes| bytes > contract.resources.maximum_output_bytes)
        {
            return Err(DynamicsRefusal::InvalidTrajectory);
        }
        self.final_state
            .validate()
            .map_err(|_| DynamicsRefusal::InvalidFinalState)?;
        if self.final_state.dimensions != request.initial_state.value.dimensions
            || self.final_state.axes != request.initial_state.value.axes
            || self.final_state.element != request.initial_state.value.element
            || self
                .final_state
                .byte_count()
                .map_err(|_| DynamicsRefusal::InvalidFinalState)?
                > contract.resources.maximum_state_bytes
        {
            return Err(DynamicsRefusal::InvalidFinalState);
        }
        if self.internal_steps == 0
            || self.internal_steps > contract.resources.maximum_internal_steps
            || self.function_evaluations == 0
            || self.function_evaluations > contract.resources.maximum_function_evaluations
            || self.consumed_work_units == 0
            || self.consumed_work_units > contract.resources.maximum_work_units
            || self.estimated_error_millionths
                > contract.accuracy.maximum_estimated_error_millionths
        {
            return Err(DynamicsRefusal::WorkBoundExceeded);
        }
        self.realization.validate()
    }
}

impl SolverRealization {
    fn validate(&self) -> Result<(), DynamicsRefusal> {
        for value in [
            &self.implementation_identity,
            &self.solver_family,
            &self.adapter_name,
            &self.adapter_version,
            &self.runtime_build_identity,
            &self.device_profile,
        ] {
            text(value).map_err(|_| DynamicsRefusal::InvalidRealization)?;
        }
        Ok(())
    }
}

fn time_unit(unit: QuantityUnit) -> bool {
    matches!(
        unit,
        QuantityUnit::Second
            | QuantityUnit::Millisecond
            | QuantityUnit::Microsecond
            | QuantityUnit::Nanosecond
    )
}

fn text(value: &str) -> Result<(), DynamicsRefusal> {
    if value.is_empty() || value.len() > 128 {
        Err(DynamicsRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}

fn nonzero(value: [u8; 32]) -> Result<(), DynamicsRefusal> {
    if value == [0; 32] {
        Err(DynamicsRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}
