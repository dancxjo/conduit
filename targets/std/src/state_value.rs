//! Typed finite State adapter for the ordinary kernel Step scheduler.
use conduit_core::{
    PlannedGear, PlannedStateBoundary, PreparedStructuredValueValidator, StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    state_delay::{back::StateBack, StateDelay},
    Failure, FailureCode, PortId,
};

pub use crate::host_execution::continuity::StateContinuationRunFailure;
mod continuity;
pub use continuity::{RetainedTypedState, StateContinuityFailure};

/// Finished ordinary execution plus its separately owned retained State cells.
/// The report's disposition remains authoritative; retention is not completion.
pub struct RetainedStdRun {
    pub report: crate::StdRunReport,
    pub states: Vec<RetainedTypedState>,
}

impl<const PORTS: usize> StepOperation<PORTS> for TypedStateBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let port = self.back.next_port();
        if io.input(port).is_some() {
            let Some(canonical) = input_bytes.input(port) else {
                <StateBack<64> as StepOperation<PORTS>>::cancel(&mut self.back);
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 9,
                });
            };
            if let Err(error) = self.validator.validate(canonical) {
                <StateBack<64> as StepOperation<PORTS>>::cancel(&mut self.back);
                let capacity = matches!(
                    error,
                    conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge
                );
                return StepOutcome::Fail(Failure {
                    code: if capacity {
                        FailureCode::StorageExhausted
                    } else {
                        FailureCode::InvalidInput
                    },
                    detail: if capacity { 1 } else { 9 },
                });
            }
        }
        <StateBack<64> as StepOperation<PORTS>>::step(&mut self.back, io, input_bytes)
    }

    fn step_committed(&mut self) {
        <StateBack<64> as StepOperation<PORTS>>::step_committed(&mut self.back);
    }

    fn cancel(&mut self) {
        <StateBack<64> as StepOperation<PORTS>>::cancel(&mut self.back);
    }
}

pub struct TypedStateBack {
    binding: Option<continuity::StateExecutionBinding>,
    back: StateBack<64>,
    validator: PreparedStructuredValueValidator,
}

impl TypedStateBack {
    /// Prepare all owned schema/storage before Play. Numeric identities must be
    /// supplied by the host's exact lowering tables, never selected at runtime.
    pub fn prepare(
        placement: &PlannedGear,
        state: &PlannedStateBoundary,
        slot: u16,
        next: PortId,
        current: PortId,
    ) -> Result<Self, String> {
        if state.retained.is_some() {
            return Err("retained State requires owned continuity admission".into());
        }
        let validator = Self::prepare_validator(placement, state)?;
        let cell = StateDelay::externally_continued(
            slot,
            state.maximum_value_bytes as usize,
            &state.initial_value,
        )
        .map_err(|error| format!("State storage: {error:?}"))?;
        let back = StateBack::new(cell, next, current)
            .map_err(|error| format!("State back: {error:?}"))?;
        Ok(Self {
            binding: None,
            back,
            validator,
        })
    }

    fn prepare_validator(
        placement: &PlannedGear,
        state: &PlannedStateBoundary,
    ) -> Result<PreparedStructuredValueValidator, String> {
        conduit_semantic_catalog::state_value::validate_state_placement(placement, state)
            .map_err(|error| format!("State semantic admission: {error:?}"))?;
        if placement.execution_profile_id.as_str() != conduit_std_offers::STATE_VALUE_STD_PROFILE
            || placement.implementation_id.as_str()
                != conduit_std_offers::STATE_VALUE_STD_IMPLEMENTATION
            || placement.artifact_id.as_str() != conduit_std_offers::STATE_VALUE_STD_ARTIFACT
            || state.maximum_value_bytes > conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES
            || !placement.host_calls.is_empty()
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || !placement.pool_references.is_empty()
        {
            return Err("State placement differs from the installed finite implementation".into());
        }
        let initial = StructuredInfoValue::from_canonical_bytes(&state.initial_value)
            .map_err(|error| format!("State initialization: {error:?}"))?;
        let validator = PreparedStructuredValueValidator::new(
            initial.value_type(),
            state.maximum_value_bytes as usize,
        )
        .map_err(|error| format!("State validator: {error:?}"))?;
        validator
            .validate(&state.initial_value)
            .map_err(|error| format!("State initial shape: {error:?}"))?;
        Ok(validator)
    }

    pub fn current(&self) -> &[u8] {
        self.back.state().current()
    }
    pub fn generation(&self) -> u64 {
        self.back.state().generation()
    }
}
