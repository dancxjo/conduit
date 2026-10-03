//! Consume exact pre-admitted child pools and finite activation storage.
use super::*;
use crate::KernelCompositeDefinition;
#[cfg(feature = "fixture-registry-preparation")]
use crate::KernelOperationRegistry;
#[cfg(feature = "fixture-registry-preparation")]
use conduit_core::verify_plan;

impl BoundedActivationHost {
    #[cfg(feature = "fixture-registry-preparation")]
    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        input_port: PortId,
        output_port: PortId,
        maximum_items: u16,
    ) -> Result<Self, BoundedActivationError> {
        let contract = BoundedActivationContract::for_definition(
            &definition,
            input_port,
            output_port,
            maximum_items,
        )?;
        Self::prepare_with_contract(definition, registry, contract)
    }

    /// Fixture-only registry preparation from exact Plan truth. Production
    /// enters through `PreparedActivationChildPool`, which consumes subordinate
    /// preparation receipts. The selected
    /// subplan, Value fronts, and finite activation limits must be identical
    /// to the executable composite definition; no runtime lookup or widening
    /// may repair a disagreement.
    #[cfg(feature = "fixture-registry-preparation")]
    pub fn prepare_planned(
        planned: &PlannedActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, BoundedActivationError> {
        if planned.selected_plan_id != planned.selected_plan.plan_id
            || planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        let contract = BoundedActivationContract::for_definition(
            &definition,
            planned.input.front_port_id.clone(),
            planned.output.front_port_id.clone(),
            planned.limits.maximum_items,
        )?;
        if contract.selected_plan_id != planned.selected_plan_id
            || contract.input_value_kind != planned.input.value_kind
            || contract.input_abnormal_kind != planned.input.abnormal_kind
            || contract.output_value_kind != planned.output.value_kind
            || contract.output_abnormal_kind != planned.output.abnormal_kind
            || contract.maximum_active != planned.limits.maximum_active
            || contract.maximum_queue_items != planned.limits.maximum_queue_items
            || contract.maximum_queue_bytes != planned.limits.maximum_queue_bytes
            || contract.maximum_items != planned.limits.maximum_items
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        Self::prepare_with_contract(definition, registry, contract)
    }

    #[cfg(feature = "fixture-registry-preparation")]
    fn prepare_with_contract(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        contract: BoundedActivationContract,
    ) -> Result<Self, BoundedActivationError> {
        // Refuse an unavailable or over-budget exact subgraph before any input
        // can become owed work. Each activation is prepared afresh below.
        let maximum_items = usize::from(contract.maximum_items);
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(BoundedActivationError::Refused)?,
            );
        }
        Self::prepare_with_ready(contract, ready)
    }

    pub(crate) fn prepare_planned_with_ready(
        planned: &PlannedActivation,
        definition: &KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedActivationError> {
        let contract = BoundedActivationContract::for_definition(
            definition,
            planned.input.front_port_id.clone(),
            planned.output.front_port_id.clone(),
            planned.limits.maximum_items,
        )?;
        if planned.selected_plan.as_ref() != &definition.internal_plan
            || contract.selected_plan_id != planned.selected_plan_id
            || contract.input_value_kind != planned.input.value_kind
            || contract.output_value_kind != planned.output.value_kind
        {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        Self::prepare_with_ready(contract, ready)
    }

    fn prepare_with_ready(
        contract: BoundedActivationContract,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedActivationError> {
        let maximum_items = usize::from(contract.maximum_items);
        if ready.len() != maximum_items || ready.capacity() != maximum_items {
            return Err(BoundedActivationError::PlannedContractMismatch);
        }
        let output_buffer = ValuePayload {
            value_kind: contract.output_value_kind.clone(),
            encoded: Vec::with_capacity(contract.maximum_queue_bytes as usize),
        };
        let abnormal_buffer =
            contract
                .output_abnormal_kind
                .clone()
                .map(|value_kind| ValuePayload {
                    value_kind,
                    encoded: Vec::with_capacity(contract.maximum_queue_bytes as usize),
                });
        Ok(Self {
            contract,
            ready,
            receipts: Vec::with_capacity(maximum_items),
            active: None,
            input_close_pending: false,
            output_completed: false,
            output_sequence: None,
            output_buffer,
            abnormal_buffer,
            pending_input_terminal: None,
            state: BoundedActivationState::Idle,
        })
    }
}
