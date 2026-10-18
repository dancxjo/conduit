//! Exact initialized retained-state boundary, including heterogeneous updates.
use crate::{
    CheckedValueContract, KindSemanticContract, KindSemanticLaw, PortDescriptor, PortDirection,
    PortId, PortTemporal, TemporalStateBehavior,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Emit this exact initial value before awaiting the declared update input.
/// The selected Back must implement the law; a kind name cannot create a cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializedStateLaw {
    pub update_port_id: PortId,
    pub current_port_id: PortId,
    pub current: CheckedValueContract,
    pub initial: Vec<u8>,
}

pub fn initialized_state_boundary<'a>(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    semantic: &'a KindSemanticContract,
) -> Result<Option<&'a InitializedStateLaw>, &'static str> {
    let mut laws = semantic.laws.iter().filter_map(|law| match law {
        KindSemanticLaw::InitializedState(value) => Some(value),
        _ => None,
    });
    let Some(law) = laws.next() else {
        return Ok(None);
    };
    if laws.next().is_some() || inputs.len() != 1 || outputs.len() != 1 {
        return Err("initialized state requires one exact update/current boundary");
    }
    let mut state = semantic
        .laws
        .iter()
        .filter(|law| matches!(law, KindSemanticLaw::TemporalState(_)));
    if state.next()
        != Some(&KindSemanticLaw::TemporalState(
            TemporalStateBehavior::Retained,
        ))
        || state.next().is_some()
    {
        return Err("initialized state must declare retained temporal semantics");
    }
    let input = &inputs[0];
    let output = &outputs[0];
    if input.port_id != law.update_port_id
        || input.direction != PortDirection::Input
        || input.abnormal_kind.is_some()
        || input.temporal != (PortTemporal::Flow { closes: false })
        || output.port_id != law.current_port_id
        || output.direction != PortDirection::Output
        || output.abnormal_kind.is_some()
        || !matches!(
            output.temporal,
            PortTemporal::Current | PortTemporal::Flow { closes: false }
        )
        || output.value_kind != law.current.value_kind
        || law.current.maximum_bytes == 0
        || law.current.validate_definition().is_err()
        || law.current.validate(&law.initial).is_err()
    {
        return Err("initialized state has invalid ports, initial value or finite bound");
    }
    Ok(Some(law))
}

pub(crate) fn validate_kind(kind: &crate::Kind) -> Result<(), crate::KindValidationError> {
    // Checker projections carry placeholder realization limits. Check meaning
    // here; the planner separately checks the selected Back's finite storage.
    initialized_state_boundary(&kind.inputs, &kind.outputs, &kind.semantic_contract())
        .map(|_| ())
        .map_err(|_| crate::KindValidationError::InvalidInitializedState)
}

#[cfg(test)]
#[path = "initialized_state/tests.rs"]
mod tests;
