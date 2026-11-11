//! Prepared routing of ordinary local-model calls in the shared Body kernel.
//! Provider custody stays with StdHost; this adds no scheduler or retry policy.
use super::*;
use crate::hosted_local_model::HostedLocalModelAdapter;
use conduit_core::PlannedGear;
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) struct ModelRoute<'a> {
    placements: Vec<(NodeId, PlannedGear)>,
    provider: Option<&'a mut Box<dyn HostedLocalModelAdapter>>,
    output: Vec<u8>,
}

pub(super) fn supports(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_ai::LOCAL_MODEL_OPERATION
}

impl<'a> ModelRoute<'a> {
    pub(super) fn prepare(
        fragments: &[&PlanFragment],
        lowered: &[LoweredPlanFragment],
    ) -> Result<Self, String> {
        if fragments.len() != lowered.len() {
            return Err("Body model partition mismatch".into());
        }
        let mut placements = Vec::new();
        let mut maximum = 0;
        for (fragment, partition) in fragments.iter().zip(lowered) {
            for operation in partition.host_calls.iter().filter(|op| supports(op)) {
                let placement_id = partition
                    .identity
                    .placement_for_node(operation.node)
                    .ok_or("Body model operation has no exact placement identity")?;
                let placement = fragment
                    .placements
                    .iter()
                    .find(|placement| &placement.placement_id == placement_id)
                    .ok_or("Body model operation has no exact planned gear")?;
                super::super::local_model_back::validate(placement)?;
                // Streaming has a separate retained-input/cancellation lifecycle.
                if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
                    return Err("streaming local-model Body calls are not installed".into());
                }
                maximum = maximum.max(operation.binding.maximum_output_bytes as usize);
                placements.push((operation.node, placement.clone()));
            }
        }
        Ok(Self {
            placements,
            provider: None,
            output: Vec::with_capacity(maximum),
        })
    }

    pub(super) fn attach(
        &mut self,
        provider: Option<&'a mut Box<dyn HostedLocalModelAdapter>>,
    ) -> Result<(), String> {
        if self.placements.is_empty() {
            return Ok(());
        }
        let provider = provider.ok_or("Body model has no admitted provider")?;
        let offers = provider
            .offer()
            .capability_offers()
            .map_err(|error| format!("Body model provider offer: {error:?}"))?;
        if self.placements.iter().any(|(_, placement)| {
            !offers.iter().any(|offer| {
                offer.kind_id == placement.kind_id
                    && offer.implementation.artifact_id == placement.artifact_id
            })
        }) {
            return Err("Body model provider differs from planned artifact".into());
        }
        self.provider = Some(provider);
        Ok(())
    }

    pub(super) fn require_supported(&self) -> Result<(), String> {
        if !self.placements.is_empty() && self.provider.is_none() {
            return Err("Body model has no admitted provider".into());
        }
        Ok(())
    }

    pub(super) fn complete(
        &mut self,
        operation: &LoweredHostCall,
        request: HostCallRequest,
        scheduler: &mut InstalledScheduler,
    ) -> Result<bool, String> {
        if !supports(operation) {
            return Ok(false);
        }
        let placement = self
            .placements
            .iter()
            .find(|(node, _)| *node == request.node)
            .map(|(_, placement)| placement)
            .ok_or("Body model request has no prepared exact placement")?;
        let input = scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("Body model request value: {error:?}"))?;
        let completion = super::super::model_host::execute(
            operation.contract_id.as_str(),
            placement,
            input,
            self.provider.as_mut().map(|provider| &mut ***provider),
            &mut self.output,
        )?;
        let output = if completion.has_output() {
            let value = scheduler
                .store_host_value(&self.output)
                .map_err(|error| format!("store Body model output: {error:?}"))?;
            Some(
                BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                    .map_err(|error| format!("bound Body model output: {error:?}"))?,
            )
        } else {
            None
        };
        scheduler
            .complete_host_call(request.node, request.request, completion.outcome(output))
            .map_err(|error| format!("complete Body model operation: {error:?}"))?;
        Ok(true)
    }
}
