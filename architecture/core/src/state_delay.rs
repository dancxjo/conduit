use serde::{Deserialize, Serialize};

use alloc::string::String;
use alloc::vec::Vec;

use crate::{GearId, KindId, SignStorageBudget};

/// Exact semantic identity of one retained computational state occurrence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateId(String);

impl StateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StateId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for StateId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Finite continuation demand. An externally continued state still admits one
/// transition at a time; the external authority is not invented by the state.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateContinuation {
    MaximumTransitions(u64),
    ExternallyBounded,
}

/// Immutable Plan truth for one explicit delay boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStateBoundary {
    pub state_id: StateId,
    pub gear_id: GearId,
    pub value_kind: KindId,
    pub initial_value: Vec<u8>,
    pub maximum_value_bytes: u32,
    pub continuation: StateContinuation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateResourceBudget {
    pub instance_capacity: u16,
    /// Current plus at most one candidate for every instance.
    pub retained_value_bytes: u64,
    pub retained_value_slots: u32,
    pub sign_storage: SignStorageBudget,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StatePlanError {
    EmptyIdentity,
    ZeroValueBound,
    ZeroTransitionBound,
    DuplicateState,
    ResourceOverflow,
}

pub fn state_resource_budget(
    states: &[PlannedStateBoundary],
) -> Result<StateResourceBudget, StatePlanError> {
    let instance_capacity =
        u16::try_from(states.len()).map_err(|_| StatePlanError::ResourceOverflow)?;
    let mut retained_value_bytes = 0u64;
    for (index, state) in states.iter().enumerate() {
        if state.state_id.as_str().is_empty()
            || state.gear_id.as_str().is_empty()
            || state.value_kind.as_str().is_empty()
        {
            return Err(StatePlanError::EmptyIdentity);
        }
        if state.maximum_value_bytes == 0 {
            return Err(StatePlanError::ZeroValueBound);
        }
        if state.initial_value.len() > state.maximum_value_bytes as usize {
            return Err(StatePlanError::ResourceOverflow);
        }
        if matches!(state.continuation, StateContinuation::MaximumTransitions(0)) {
            return Err(StatePlanError::ZeroTransitionBound);
        }
        if states[..index]
            .iter()
            .any(|prior| prior.state_id == state.state_id || prior.gear_id == state.gear_id)
        {
            return Err(StatePlanError::DuplicateState);
        }
        retained_value_bytes = retained_value_bytes
            .checked_add(
                u64::from(state.maximum_value_bytes)
                    .checked_mul(2)
                    .ok_or(StatePlanError::ResourceOverflow)?,
            )
            .ok_or(StatePlanError::ResourceOverflow)?;
    }
    let retained_value_slots = u32::from(instance_capacity)
        .checked_mul(2)
        .ok_or(StatePlanError::ResourceOverflow)?;
    // Initialized plus one terminal transition per state must always fit.
    let sign_items = u32::from(instance_capacity)
        .checked_mul(2)
        .ok_or(StatePlanError::ResourceOverflow)?;
    Ok(StateResourceBudget {
        instance_capacity,
        retained_value_bytes,
        retained_value_slots,
        sign_storage: SignStorageBudget {
            item_capacity: u16::try_from(sign_items)
                .map_err(|_| StatePlanError::ResourceOverflow)?,
            byte_capacity: sign_items
                .checked_mul(32)
                .ok_or(StatePlanError::ResourceOverflow)?,
        },
    })
}

/// State is an immutable fragment commitment, not an unsealed side table.
pub(crate) fn push_canonical_state(bytes: &mut Vec<u8>, states: &[PlannedStateBoundary]) {
    if states.is_empty() {
        return;
    }
    crate::push_string(bytes, "conduit/fragment-state@1");
    crate::push_u64(bytes, states.len() as u64);
    for state in states {
        crate::push_string(bytes, state.state_id.as_str());
        crate::push_string(bytes, state.gear_id.as_str());
        crate::push_string(bytes, state.value_kind.as_str());
        crate::push_u64(bytes, state.initial_value.len() as u64);
        bytes.extend_from_slice(&state.initial_value);
        crate::push_u32(bytes, state.maximum_value_bytes);
        match state.continuation {
            StateContinuation::MaximumTransitions(count) => {
                bytes.push(0);
                crate::push_u64(bytes, count);
            }
            StateContinuation::ExternallyBounded => bytes.push(1),
        }
    }
}

pub(crate) fn verify_plan_states(plan: &crate::Plan) -> bool {
    let mut state_ids = alloc::collections::BTreeSet::new();
    let mut gear_ids = alloc::collections::BTreeSet::new();
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.states)
        .all(|state| state_ids.insert(&state.state_id) && gear_ids.insert(&state.gear_id))
}

pub(crate) fn verify_fragment_state(fragment: &crate::PlanFragment) -> bool {
    if fragment.states.is_empty() {
        return true;
    }
    let Ok(budget) = state_resource_budget(&fragment.states) else {
        return false;
    };
    let Some(base_signs) = crate::mandatory_sign_storage_requirement(&fragment.expected_sign)
    else {
        return false;
    };
    if !fragment.states.is_empty()
        && (base_signs
            .item_capacity
            .checked_add(budget.sign_storage.item_capacity)
            .is_none_or(|required| required > fragment.sign_storage_budget.item_capacity)
            || base_signs
                .byte_capacity
                .checked_add(budget.sign_storage.byte_capacity)
                .is_none_or(|required| required > fragment.sign_storage_budget.byte_capacity))
    {
        return false;
    }
    fragment.states.iter().all(|state| {
        let mut placements = fragment
            .placements
            .iter()
            .filter(|gear| gear.gear_id == state.gear_id);
        let Some(gear) = placements.next() else {
            return false;
        };
        placements.next().is_none()
            && gear.host_id == fragment.host_id
            && gear.boot_id == fragment.boot_id
            && gear.inputs.len() == 1
            && gear.outputs.len() == 1
            && gear.inputs[0].direction == crate::PortDirection::Input
            && gear.outputs[0].direction == crate::PortDirection::Output
            && gear.inputs[0].value_kind == state.value_kind
            && gear.outputs[0].value_kind == state.value_kind
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn state(id: &str, bytes: u32) -> PlannedStateBoundary {
        PlannedStateBoundary {
            state_id: StateId::from(id),
            gear_id: GearId::from(id),
            value_kind: KindId::from("number/u32@1"),
            initial_value: 0u32.to_le_bytes().to_vec(),
            maximum_value_bytes: bytes,
            continuation: StateContinuation::MaximumTransitions(3),
        }
    }

    #[test]
    fn admission_accounts_current_candidate_and_mandatory_evidence() {
        let budget = state_resource_budget(&[state("a", 4), state("b", 8)]).unwrap();
        assert_eq!(budget.instance_capacity, 2);
        assert_eq!(budget.retained_value_slots, 4);
        assert_eq!(budget.retained_value_bytes, 24);
        assert_eq!(budget.sign_storage.item_capacity, 4);
    }

    #[test]
    fn duplicate_unbounded_and_oversized_initial_state_refuse() {
        assert_eq!(
            state_resource_budget(&[state("same", 4), state("same", 4)]),
            Err(StatePlanError::DuplicateState)
        );
        let mut zero = state("zero", 4);
        zero.continuation = StateContinuation::MaximumTransitions(0);
        assert_eq!(
            state_resource_budget(&[zero]),
            Err(StatePlanError::ZeroTransitionBound)
        );
        let mut oversized = state("large", 2);
        oversized.initial_value = vec![0; 3];
        assert_eq!(
            state_resource_budget(&[oversized]),
            Err(StatePlanError::ResourceOverflow)
        );
    }

    #[test]
    fn architecture_contract_keeps_state_explicit_and_non_authoritative() {
        let contract = include_str!("../../../docs/architecture/explicit-state-delay.md");
        for required in [
            "Conduit preserves acyclic ordinary dataflow",
            "current[n+1] = next[n]",
            "Multiple candidates refuse",
            "Failure or",
            "cancellation discards the candidate",
            "never an",
            "input to commitment or scheduling",
        ] {
            assert!(contract.contains(required), "missing {required}");
        }
    }
}
