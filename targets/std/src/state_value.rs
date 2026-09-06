//! Typed finite State adapter for the ordinary kernel Operation driver.
use conduit_core::{
    PlannedGear, PlannedStateBoundary, PreparedStructuredValueValidator, StructuredInfoValue,
};
use conduit_kernel::{
    state_delay::{operation::StateOperation, StateDelay},
    Failure, FailureCode, Operation, OperationAction, OperationInput, PortId, ValueRef,
};

mod continuity;
pub use continuity::{RetainedTypedState, StateContinuityFailure};

pub struct TypedStateOperation {
    binding: Option<continuity::StateExecutionBinding>,
    operation: StateOperation<64>,
    validator: PreparedStructuredValueValidator,
}

impl TypedStateOperation {
    /// Prepare all owned schema/storage before Play. Numeric identities must be
    /// supplied by the Host's exact lowering tables, never selected at runtime.
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
        let operation = StateOperation::new(cell, next, current)
            .map_err(|error| format!("State operation: {error:?}"))?;
        Ok(Self {
            binding: None,
            operation,
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
            || !placement.host_operations.is_empty()
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
        self.operation.state().current()
    }
    pub fn generation(&self) -> u64 {
        self.operation.state().generation()
    }
}

impl Operation for TypedStateOperation {
    fn step_committed(&mut self) {
        self.operation.step_committed();
    }
    fn start(&mut self) -> OperationAction {
        self.operation.start()
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.operation.resume(input)
    }
    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        if let Err(error) = self.validator.validate(canonical) {
            self.operation.cancel();
            let capacity = matches!(
                error,
                conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge
            );
            return OperationAction::Fail(Failure {
                code: if capacity {
                    FailureCode::StorageExhausted
                } else {
                    FailureCode::InvalidInput
                },
                detail: if capacity { 1 } else { 9 },
            });
        }
        self.operation.resume_value(port, value, canonical)
    }
    fn advance(&mut self) -> OperationAction {
        self.operation.advance()
    }
    fn cancel(&mut self) {
        self.operation.cancel();
    }
}
