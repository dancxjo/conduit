//! Consuming handoff of typed State between exact prepared executions.
use super::TypedStateOperation;
use conduit_core::{
    bind_active_play, verify_plan_fragment, ActivePlayIdentity, FormIdentity, PlanFragment,
    PlannedGear, RetainedStateProvenance,
};
use conduit_kernel::state_delay::{operation::StateOperation, StateDelay};
use conduit_plan_lowering::lowering::LoweredState;

pub(super) struct StateExecutionBinding {
    form: FormIdentity,
    play: ActivePlayIdentity,
    state: conduit_core::StateId,
    value_kind: conduit_core::KindId,
    initial_value: Vec<u8>,
}

/// Private owned cell, never a cloneable serialized checkpoint.
pub struct RetainedTypedState {
    cell: StateDelay<64>,
    provenance: RetainedStateProvenance,
    initial_value: Vec<u8>,
}

/// Refusal preserves ownership; the lifecycle owner decides what happens next.
pub struct StateContinuityFailure<T> {
    pub reason: String,
    pub source: T,
}

impl RetainedTypedState {
    pub fn provenance(&self) -> &RetainedStateProvenance {
        &self.provenance
    }
}

impl TypedStateOperation {
    pub fn prepare_for_play(
        fragment: &PlanFragment,
        state: &LoweredState,
        play: &ActivePlayIdentity,
    ) -> Result<Self, String> {
        let (placement, binding) = bind(fragment, state, play)?;
        let mut operation = Self::prepare(
            placement,
            &state.contract,
            state.slot,
            state.next,
            state.current,
        )?;
        operation.binding = Some(binding);
        Ok(operation)
    }

    /// Only a consumed, terminal, bound operation can yield retained ownership.
    /// An active scheduler exposes borrowed drivers, so it must first retire.
    pub fn try_retire(self) -> Result<RetainedTypedState, Box<StateContinuityFailure<Self>>> {
        if !self.operation.is_terminal() || self.binding.is_none() {
            return Err(Box::new(StateContinuityFailure {
                reason: "State is not terminal or lacks exact execution binding".into(),
                source: self,
            }));
        }
        let binding = self
            .binding
            .expect("binding checked before consuming State");
        let cell = self.operation.into_state();
        let provenance = RetainedStateProvenance {
            source_form: binding.form,
            source_play: binding.play,
            source_state: binding.state,
            value_kind: binding.value_kind,
            generation: cell.generation(),
            current_value: cell.current().to_vec(),
        };
        Ok(RetainedTypedState {
            cell,
            provenance,
            initial_value: binding.initial_value,
        })
    }

    pub(crate) fn validate_continuation(
        fragment: &PlanFragment,
        state: &LoweredState,
        play: &ActivePlayIdentity,
        source: &RetainedTypedState,
    ) -> Result<(), String> {
        Self::admit_continuation(fragment, state, play, source).map(|_| ())
    }

    fn admit_continuation(
        fragment: &PlanFragment,
        state: &LoweredState,
        play: &ActivePlayIdentity,
        source: &RetainedTypedState,
    ) -> Result<
        (
            StateExecutionBinding,
            conduit_core::PreparedStructuredValueValidator,
        ),
        String,
    > {
        let (placement, binding) = bind(fragment, state, play)?;
        if binding.form != source.provenance.source_form
            || state.contract.initial_value != source.initial_value
            || state.contract.retained.as_ref() != Some(&source.provenance)
            || source.provenance.source_play.active_play_id == play.active_play_id
        {
            return Err("owned State differs from sealed continuity obligation".into());
        }
        let validator = Self::prepare_validator(placement, &state.contract)?;
        validator
            .validate(source.cell.current())
            .map_err(|error| format!("retained State shape: {error:?}"))?;
        Ok((binding, validator))
    }

    /// Validate the sealed destination obligation against actual owned source
    /// State, then move the cell without renewing generation or transition fuel.
    /// All schema/binding allocation happens before the replacement Play starts.
    pub fn prepare_continued(
        fragment: &PlanFragment,
        state: &LoweredState,
        play: &ActivePlayIdentity,
        source: RetainedTypedState,
    ) -> Result<Self, Box<StateContinuityFailure<RetainedTypedState>>> {
        let admitted = Self::admit_continuation(fragment, state, play, &source);
        let (binding, validator) = match admitted {
            Ok(admitted) => admitted,
            Err(reason) => return Err(Box::new(StateContinuityFailure { reason, source })),
        };
        let (cell, _) = match source
            .cell
            .try_transfer::<64>(state.slot, state.contract.maximum_value_bytes as usize)
        {
            Ok(transferred) => transferred,
            Err(refused) => {
                return Err(Box::new(StateContinuityFailure {
                    reason: format!("retained State storage: {:?}", refused.reason),
                    source: RetainedTypedState {
                        cell: refused.source,
                        provenance: source.provenance,
                        initial_value: source.initial_value,
                    },
                }));
            }
        };
        Ok(Self {
            binding: Some(binding),
            operation: StateOperation::new(cell, state.next, state.current)
                .expect("validated std State capacity fits the kernel envelope"),
            validator,
        })
    }
}

fn bind<'a>(
    fragment: &'a PlanFragment,
    state: &LoweredState,
    play: &ActivePlayIdentity,
) -> Result<(&'a PlannedGear, StateExecutionBinding), String> {
    if !verify_plan_fragment(fragment)
        || play.plan_id != fragment.plan_id
        || play.host_id != fragment.host_id
        || play.boot_id != fragment.boot_id
        || *play
            != bind_active_play(
                &play.plan_id,
                &play.host_id,
                &play.boot_id,
                play.play_sequence,
            )
        || fragment.states.get(usize::from(state.slot)) != Some(&state.contract)
        || state.next != conduit_kernel::PortId(0)
        || state.current != conduit_kernel::PortId(0)
    {
        return Err("State execution identity differs from its sealed fragment".into());
    }
    let placement = fragment
        .placements
        .get(usize::from(state.node.0))
        .filter(|placement| placement.gear_id == state.contract.gear_id)
        .ok_or_else(|| "State numeric owner differs from its placement".to_string())?;
    Ok((
        placement,
        StateExecutionBinding {
            form: FormIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_form_id: fragment.checked_form_id.clone(),
                expanded_form_id: fragment.expanded_form_id.clone(),
            },
            play: play.clone(),
            state: state.contract.state_id.clone(),
            value_kind: state.contract.value_kind.clone(),
            initial_value: state.contract.initial_value.clone(),
        },
    ))
}
