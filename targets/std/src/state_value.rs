//! Typed finite State adapter for the ordinary kernel Step scheduler.
use conduit_core::{
    KindId, PlannedGear, PlannedStateBoundary, PreparedStructuredValueValidator,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    state_delay::{back::StateBack, StateDelay},
    Failure, FailureCode, PortId, ValueStorage,
};

pub use crate::host_execution::continuity::StateContinuationRunFailure;
mod continuity;
mod durable;
pub use continuity::{RetainedTypedState, StateContinuityFailure};
pub(crate) use durable::InstalledDurableStateHost;
pub use durable::{
    DurableStateBack, DurableStateBinding, DurableStateHost, DurableStateRefusal,
    FileDurableStateResidence, RecoveryDisposition,
};

/// Installed typed wrapper for the Body-durable driver. Validation remains the
/// same semantic obligation as the Play-only Back; durability does not make
/// malformed values acceptable.
pub(crate) struct InstalledDurableStateBack {
    back: DurableStateBack,
    validator: StateValueValidator,
}

impl<const PORTS: usize> StepBack<PORTS> for InstalledDurableStateBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            let Some(encoded) = bytes.input(PortId(0)) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 20,
                });
            };
            if self.validator.validate(encoded).is_err()
                || value.byte_len > conduit_std_offers::STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES
            {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 21,
                });
            }
        }
        if self.back.expects_recovered_value()
            && bytes
                .host_output()
                .is_some_and(|value| self.validator.validate(value).is_err())
        {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 22,
            });
        }
        self.back.step(io, bytes)
    }

    fn cancel(&mut self) {
        <DurableStateBack as StepBack<PORTS>>::cancel(&mut self.back);
    }

    fn retains_host_call_input(
        &self,
        request: conduit_kernel::RequestId,
        value: conduit_kernel::ValueRef,
    ) -> bool {
        <DurableStateBack as StepBack<PORTS>>::retains_host_call_input(&self.back, request, value)
    }
}

/// Finished ordinary execution plus its separately owned retained State cells.
/// The report's disposition remains authoritative; retention is not completion.
pub struct RetainedStdRun {
    pub report: crate::StdRunReport,
    pub states: Vec<RetainedTypedState>,
}

impl<const PORTS: usize> StepBack<PORTS> for TypedStateBack {
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<
            PORTS,
        >>::prepared_output(&self.back, port)
    }

    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let port = self.back.next_port();
        if io.input(port).is_some() {
            let Some(canonical) = input_bytes.input(port) else {
                <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<PORTS>>::cancel(&mut self.back);
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 9,
                });
            };
            if let Err(error) = self.validator.validate(canonical) {
                <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<PORTS>>::cancel(&mut self.back);
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
        <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<
            PORTS,
        >>::step(&mut self.back, io, input_bytes)
    }

    fn step_committed(&mut self) {
        <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<
            PORTS,
        >>::step_committed(&mut self.back);
    }

    fn cancel(&mut self) {
        <StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }> as StepBack<
            PORTS,
        >>::cancel(&mut self.back);
    }
}

pub struct TypedStateBack {
    binding: Option<continuity::StateExecutionBinding>,
    back: StateBack<{ conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES as usize }>,
    validator: StateValueValidator,
}

enum StateValueValidator {
    Primitive(KindId),
    Structured(PreparedStructuredValueValidator),
}

impl StateValueValidator {
    fn validate(&self, value: &[u8]) -> Result<(), conduit_core::StructuredInfoRefusal> {
        match self {
            Self::Primitive(kind) => conduit_core::validate_primitive_info(kind.as_str(), value)
                .map_err(conduit_core::StructuredInfoRefusal::InvalidPrimitiveLeaf),
            Self::Structured(validator) => validator.validate(value),
        }
    }
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
        let initial = state
            .initial_value
            .as_deref()
            .ok_or("installed State Back requires an initialized keep")?;
        let validator = Self::prepare_validator(placement, state)?;
        let cell =
            StateDelay::externally_continued(slot, state.maximum_value_bytes as usize, initial)
                .map_err(|error| format!("State storage: {error:?}"))?;
        let back = StateBack::new_prepared(cell, next, current)
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
    ) -> Result<StateValueValidator, String> {
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
        Self::prepare_value_validator(placement, state)
    }

    fn prepare_value_validator(
        placement: &PlannedGear,
        state: &PlannedStateBoundary,
    ) -> Result<StateValueValidator, String> {
        let initial_bytes = state
            .initial_value
            .as_deref()
            .ok_or("installed State Back requires an initialized keep")?;
        let configured = placement
            .configuration
            .iter()
            .find(|entry| entry.key == "initial")
            .and_then(|entry| match &entry.value {
                conduit_core::ConfigurationValue::Structured(value) => Some(value),
                _ => None,
            })
            .ok_or("installed State Back requires exact typed initialization")?;
        let initial = StructuredInfoValue::from_canonical_bytes(configured.canonical_value())
            .map_err(|error| format!("State initialization: {error:?}"))?;
        if let StructuredInfoTypeShape::Leaf(kind) = initial.value_type().shape() {
            conduit_core::validate_primitive_info(kind.as_str(), initial_bytes)
                .map_err(|error| format!("State initial leaf: {error:?}"))?;
            return Ok(StateValueValidator::Primitive(kind.clone()));
        }
        let validator = PreparedStructuredValueValidator::new(
            initial.value_type(),
            state.maximum_value_bytes as usize,
        )
        .map_err(|error| format!("State validator: {error:?}"))?;
        validator
            .validate(initial_bytes)
            .map_err(|error| format!("State initial shape: {error:?}"))?;
        Ok(StateValueValidator::Structured(validator))
    }

    pub fn current(&self) -> &[u8] {
        self.back.state().current()
    }
    pub fn generation(&self) -> u64 {
        self.back.state().generation()
    }
}

pub(crate) fn prepare_durable_state(
    fragment: &conduit_core::PlanFragment,
    state: &conduit_plan_lowering::lowering::LoweredState,
    play: &conduit_core::ActivePlayIdentity,
    body: &str,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledDurableStateBack, String> {
    let (placement, _) = continuity::bind(fragment, state, play)?;
    if placement.execution_profile_id.as_str()
        != conduit_std_offers::STATE_VALUE_DURABLE_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::STATE_VALUE_DURABLE_STD_ARTIFACT
        || placement.host_calls.len() != 2
        || placement.host_calls[0].contract_id.as_str()
            != conduit_std_offers::STATE_VALUE_DURABLE_COMMIT_HOST_CALL
        || placement.host_calls[1].contract_id.as_str()
            != conduit_std_offers::STATE_VALUE_DURABLE_RECOVER_HOST_CALL
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS
        || state.contract.maximum_value_bytes
            > conduit_std_offers::STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES
        || body.is_empty()
    {
        return Err("durable State placement differs from its sealed implementation".into());
    }
    let initial = state
        .contract
        .initial_value
        .as_deref()
        .ok_or("durable State requires exact typed initialization")?;
    conduit_semantic_catalog::state_value::validate_state_placement(placement, &state.contract)
        .map_err(|error| format!("durable State semantic admission: {error:?}"))?;
    let validator = TypedStateBack::prepare_value_validator(placement, &state.contract)?;
    let initial_value = values
        .store(initial)
        .map_err(|error| format!("store durable State initial value: {error:?}"))?;
    let recovery_metadata_probe = values
        .store(&[1])
        .map_err(|error| format!("store durable State recovery metadata probe: {error:?}"))?;
    let binding = DurableStateBinding {
        body: body.into(),
        state: state.contract.state_id.clone(),
        value_kind: state.contract.value_kind.clone(),
        maximum_value_bytes: state.contract.maximum_value_bytes,
    };
    let back = DurableStateBack::new(binding, initial_value, recovery_metadata_probe)
        .map_err(|error| format!("prepare durable State: {error:?}"))?;
    Ok(InstalledDurableStateBack { back, validator })
}
