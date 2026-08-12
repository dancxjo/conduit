//! Installed finite audio render-demand source.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{AudioRenderDemand, ConfigurationValue, PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    RequestId, ValueRef, ValueStorage,
};

pub(super) static AUDIO_RENDER_DEMAND_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::AUDIO_RENDER_DEMAND_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct AudioRenderDemandOperation {
    demands: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl AudioRenderDemandOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.request_wait().unwrap_or(OperationAction::Complete)
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.demands.get(self.next).copied().map_or_else(
                    || InstalledOperation::fail(44),
                    |value| OperationAction::Emit {
                        port: conduit_kernel::PortId(0),
                        value,
                    },
                )
            }
            _ => InstalledOperation::fail(45),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        self.request_wait().unwrap_or(OperationAction::Complete)
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.demands.capacity() + self.waits.capacity()
    }

    fn request_wait(&mut self) -> Option<OperationAction> {
        let wait = self.waits.get(self.next).copied()?;
        let request = RequestId(u32::try_from(self.next).ok()?);
        self.pending = Some(request);
        Some(OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(wait, 8).expect("render wait is exactly eight bytes"),
        })
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    let blocks = conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS;
    let value_items = blocks
        .checked_mul(2)
        .ok_or_else(|| "audio render-demand value item budget overflow".to_string())?;
    let value_bytes = u32::from(blocks)
        .checked_mul((conduit_core::AUDIO_RENDER_DEMAND_ENCODED_LEN + 8) as u32)
        .ok_or_else(|| "audio render-demand value byte budget overflow".to_string())?;
    let sign_items = blocks
        .checked_mul(15)
        .and_then(|items| items.checked_add(64))
        .ok_or_else(|| "audio render-demand Sign budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items,
        value_bytes,
        host_requests: usize::from(blocks),
        sign_items,
        maximum_value_bytes: conduit_core::AUDIO_RENDER_DEMAND_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let blocks = usize::from(conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS);
    let mut demands = Vec::with_capacity(blocks);
    let mut waits = Vec::with_capacity(blocks);
    for sequence in 0..conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS {
        let demand = AudioRenderDemand::new(
            conduit_std_catalog::AUDIO_RENDER_CLOCK_ID,
            u64::from(sequence) * u64::from(conduit_std_catalog::AUDIO_RENDER_BLOCK_FRAMES),
            conduit_std_catalog::AUDIO_RENDER_BLOCK_FRAMES,
            u32::from(sequence),
        )
        .map_err(|error| format!("construct audio render demand: {error:?}"))?;
        demands.push(
            values
                .store(&demand.encode())
                .map_err(|error| format!("store audio render demand: {error:?}"))?,
        );
        waits.push(
            values
                .store(&conduit_std_catalog::AUDIO_RENDER_PERIOD_MILLIS.to_le_bytes())
                .map_err(|error| format!("store audio render wait: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::AudioRenderDemand(
        AudioRenderDemandOperation {
            demands,
            waits,
            next: 0,
            pending: None,
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    let configuration_is_exact = placement.configuration.len() == 2
        && placement.configuration.iter().any(|entry| {
            entry.key == conduit_std_catalog::AUDIO_RENDER_BLOCK_FRAMES_KEY
                && entry.value
                    == ConfigurationValue::U64(u64::from(
                        conduit_std_catalog::AUDIO_RENDER_BLOCK_FRAMES,
                    ))
        })
        && placement.configuration.iter().any(|entry| {
            entry.key == conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS_KEY
                && entry.value
                    == ConfigurationValue::U64(u64::from(
                        conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS,
                    ))
        });
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || placement.outputs.len() != 1
        || placement.outputs[0].port_id.as_str() != "demand"
        || placement.outputs[0].direction != PortDirection::Output
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str()
            != conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || !placement.authority.is_empty()
        || !configuration_is_exact
    {
        return Err("planned audio/render-demand identity does not match installation".into());
    }
    Ok(())
}

pub(crate) fn offer() -> conduit_core::CapabilityOffer {
    conduit_std_catalog::audio_render_demand_offer()
}
