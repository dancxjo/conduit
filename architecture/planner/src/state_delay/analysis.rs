//! Finite retained-value representation analysis for admitted State graphs.
//!
//! This is not a whole-Form termination or reachability proof. Kind validity,
//! transition restrictions, external effects, and runtime metadata may change
//! the reachable state space. The exact representation capacities remain useful
//! even when a semantic transition-system proof is absent.

use alloc::vec::Vec;
use conduit_core::{
    state_resource_budget, FormIdentity, GearId, KindId, StateContinuation, StateId,
    StatePlanError, StateResourceBudget,
};

use super::AdmittedStateGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateValueDomain {
    pub state_id: StateId,
    pub gear_id: GearId,
    pub value_kind: KindId,
    pub maximum_value_bytes: u32,
    pub continuation: StateContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationEnumeration {
    /// Exact count of all raw current/candidate byte representations, including
    /// empty current, empty candidate, and absent candidate as separate values.
    WithinBudget { representations: u64 },
    /// The complete raw representation set exceeds this analysis allowance.
    /// This does not prove that semantic reachability is impractical: a Kind
    /// may restrict that set or admit a stronger symbolic proof.
    ExceedsBudget { maximum_representations: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCapacityAnalysis {
    pub form_identity: FormIdentity,
    pub domains: Vec<StateValueDomain>,
    pub resources: StateResourceBudget,
    pub enumeration: RepresentationEnumeration,
}

impl AdmittedStateGraph {
    /// Analyze finite storage without enumerating values or executing a step.
    /// Runtime work/fuel and externally driven lifetime do not become semantic
    /// termination claims. No allocation is proportional to represented bytes.
    pub fn analyze_value_storage(
        &self,
        maximum_representations: u64,
    ) -> Result<StateCapacityAnalysis, StatePlanError> {
        let resources = state_resource_budget(&self.states)?;
        let domains = self
            .states
            .iter()
            .map(|state| StateValueDomain {
                state_id: state.state_id.clone(),
                gear_id: state.gear_id.clone(),
                value_kind: state.value_kind.clone(),
                maximum_value_bytes: state.maximum_value_bytes,
                continuation: state.continuation,
            })
            .collect::<Vec<_>>();
        let count = representation_count(&domains, maximum_representations);
        Ok(StateCapacityAnalysis {
            form_identity: self.form_identity.clone(),
            domains,
            resources,
            enumeration: match count {
                Some(representations) => {
                    RepresentationEnumeration::WithinBudget { representations }
                }
                None => RepresentationEnumeration::ExceedsBudget {
                    maximum_representations,
                },
            },
        })
    }
}

fn representation_count(domains: &[StateValueDomain], budget: u64) -> Option<u64> {
    let mut product = 1u64;
    if budget == 0 {
        return None;
    }
    for domain in domains {
        // S(B) = 1 + 256 + ... + 256^B canonical raw byte strings by length.
        // The recurrence overflows u64 after at most eight iterations even for
        // multi-gigabyte declared capacities: no capacity-sized loop or buffer.
        let mut strings = 1u64;
        for _ in 0..domain.maximum_value_bytes {
            strings = strings.checked_mul(256)?.checked_add(1)?;
            if strings > budget {
                return None;
            }
        }
        let current_and_candidate = strings.checked_mul(strings.checked_add(1)?)?;
        product = product.checked_mul(current_and_candidate)?;
        if product > budget {
            return None;
        }
    }
    Some(product)
}

#[cfg(test)]
mod tests;
