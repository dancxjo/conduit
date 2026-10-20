//! Admission and finite storage for fresh and reusable scan child owners.
use super::*;

impl BoundedScanActivationHost {
    #[cfg(feature = "fixture-registry-preparation")]
    pub fn prepare(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, BoundedScanError> {
        if planned.selected_plan_id != planned.selected_plan.plan_id
            || planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || planned.accumulator_input.value_kind != planned.output.value_kind
            || planned.limits.maximum_active != 1
            || planned.limits.maximum_queue_items != 1
            || planned.initial_accumulator.len() > planned.retained_accumulator_bytes as usize
            || planned.terminal_policy
                != PlannedScanTerminalPolicy::DrainThenCloseWithoutExtraEmission
            || planned.abnormal_policy
                != PlannedScanAbnormalPolicy::DiscardAccumulatorAndPropagateExact
            || planned.cancellation_policy
                != PlannedScanCancellationPolicy::DiscardAccumulatorWithoutEmission
            || planned.effect_multiplicity
                != PlannedActivationEffectMultiplicity::OncePerAcceptedInput
            || !front_matches(
                &definition,
                &planned.accumulator_input,
                PortDirection::Input,
            )
            || !front_matches(&definition, &planned.item_input, PortDirection::Input)
            || !front_matches(&definition, &planned.output, PortDirection::Output)
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        let maximum_items = usize::from(planned.limits.maximum_items);
        if maximum_items == 0 {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(BoundedScanError::Refused)?,
            );
        }
        Self::prepare_with_ready(planned, definition, ready)
    }

    pub(crate) fn prepare_with_ready(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedScanError> {
        Self::prepare_pool(planned, definition, ready, false)
    }
    pub(crate) fn prepare_reusable_with_ready(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        ready: Vec<KernelCompositeHost>,
    ) -> Result<Self, BoundedScanError> {
        Self::prepare_pool(planned, definition, ready, true)
    }
    fn prepare_pool(
        planned: &PlannedScanActivation,
        definition: KernelCompositeDefinition,
        mut ready: Vec<KernelCompositeHost>,
        reusable: bool,
    ) -> Result<Self, BoundedScanError> {
        let expected = if reusable {
            1
        } else {
            usize::from(planned.limits.maximum_items)
        };
        if planned.selected_plan.as_ref() != &definition.internal_plan
            || !verify_plan(&planned.selected_plan)
            || ready.len() != expected
            || ready.capacity() != expected
        {
            return Err(BoundedScanError::PlannedContractMismatch);
        }
        let mut accumulator_bytes = Vec::with_capacity(planned.retained_accumulator_bytes as usize);
        accumulator_bytes.extend_from_slice(&planned.initial_accumulator);
        let accumulator = ValuePayload {
            value_kind: planned.accumulator_input.value_kind.clone(),
            encoded: accumulator_bytes,
        };
        let candidate = ValuePayload {
            value_kind: planned.output.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
        };
        let queued = ValuePayload {
            value_kind: planned.item_input.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_item_bytes as usize),
        };
        let admission = ValuePayload {
            value_kind: planned.item_input.value_kind.clone(),
            encoded: Vec::with_capacity(planned.retained_item_bytes as usize),
        };
        let maximum_items = usize::from(planned.limits.maximum_items);
        let reusable = if reusable {
            let host = ready.pop().unwrap();
            ready = Vec::new();
            Some(Box::new(ReusableScanPool::new(
                host,
                planned.limits.maximum_items,
            )?))
        } else {
            None
        };
        let abnormal_buffer = planned
            .output
            .abnormal_kind
            .clone()
            .map(|value_kind| ValuePayload {
                value_kind,
                encoded: Vec::with_capacity(planned.retained_accumulator_bytes as usize),
            });
        Ok(Self {
            planned: planned.clone(),
            ready,
            receipts: Vec::with_capacity(if reusable.is_some() { 0 } else { maximum_items }),
            reusable,
            accumulator,
            queued,
            queued_ready: false,
            admission,
            active: None,
            candidate,
            abnormal_buffer,
            candidate_ready: false,
            output_ready: false,
            closing: false,
            state: BoundedScanState::Idle,
            parent_play: None,
        })
    }
}
