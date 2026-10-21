//! Exact, finite local Fore boundary for one installed Body Play.
use crate::{
    body_execution::BodyForeOutputAdapter, BodyLiveForeQueue, ExternalForeDelivery,
    ExternalForeInput,
};
use conduit_core::PortDirection;
use conduit_plan_lowering::lowering::{LoweredForePort, LoweredPlanFragment};

use super::super::keyboard_input_completion::{CompletedInputEvent, MAX_COMPLETED_INPUT_EVENTS};
use std::collections::VecDeque;

mod custody;
use crate::body_execution::BodyForePendingOutput;

use super::super::{
    external_fore_feeder::{self, PreparedForeInputs},
    InstalledScheduler,
};

pub(super) struct BodyForeRoute<'a> {
    inputs: PreparedForeInputs<'a>,
    ports: Vec<LoweredForePort>,
    output_slots: Vec<OutputSlots>,
    deliveries: Vec<ExternalForeDelivery>,
    pending_output: Option<BodyForePendingOutput>,
    started: bool,
    acknowledged_inputs: VecDeque<AcknowledgedInput>,
}

pub(super) struct AcknowledgedInput {
    pub(super) event: CompletedInputEvent,
    delivery_index: usize,
}

struct OutputSlots {
    port: LoweredForePort,
    source_node: Option<conduit_kernel::NodeId>,
    available: Vec<ExternalForeDelivery>,
    input_origin: Option<(
        conduit_kernel::NodeId,
        conduit_kernel::HostCallId,
        super::super::keyboard_input_binding::InputRealization,
    )>,
    acknowledged_descriptor: Option<usize>,
    acknowledged_physical_event: Option<u64>,
}

impl<'a> BodyForeRoute<'a> {
    pub(super) fn prepare(
        partitions: &[LoweredPlanFragment],
        supplied: &'a [ExternalForeInput],
        sequential: bool,
        has_output: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_live(partitions, supplied, sequential, has_output, None)
    }

    pub(super) fn prepare_live(
        partitions: &[LoweredPlanFragment],
        queue: &BodyLiveForeQueue,
    ) -> Result<Self, String> {
        Self::prepare_with_live(partitions, &[], false, true, Some(queue))
    }

    fn prepare_with_live(
        partitions: &[LoweredPlanFragment],
        supplied: &'a [ExternalForeInput],
        sequential: bool,
        has_output: bool,
        live: Option<&BodyLiveForeQueue>,
    ) -> Result<Self, String> {
        let planned_inputs = partitions
            .iter()
            .flat_map(|part| &part.fore_ports)
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let inputs = if let Some(queue) = live {
            PreparedForeInputs::prepare_live(&planned_inputs, queue)?
        } else {
            PreparedForeInputs::prepare(&planned_inputs, supplied, sequential)?
        };
        // An external Fore input or delivery has no Plot discriminator. Refuse
        // names shared by distinct resident Plots instead of inventing fan-out.
        let mut seen = Vec::new();
        for (index, part) in partitions.iter().enumerate() {
            for port in &part.fore_ports {
                if seen
                    .iter()
                    .any(|(prior, earlier): &(usize, &LoweredForePort)| {
                        *prior != index
                            && earlier.front_port_id == port.front_port_id
                            && earlier.direction == port.direction
                            && earlier.track == port.track
                    })
                {
                    return Err("Body Fore port identity is ambiguous across Plots".into());
                }
                seen.push((index, port));
            }
        }
        let ports = partitions
            .iter()
            .flat_map(|part| &part.fore_ports)
            .cloned()
            .collect::<Vec<_>>();
        if !has_output
            && ports
                .iter()
                .any(|port| port.direction == PortDirection::Output)
        {
            return Err("sealed Body Fore output has no acknowledging adapter".into());
        }
        let mut delivery_capacity = 0_usize;
        let mut output_bytes = 0_usize;
        let mut output_slots = Vec::new();
        for port in ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
        {
            let count = usize::from(port.item_capacity.max(inputs.input_items()));
            delivery_capacity = delivery_capacity
                .checked_add(count)
                .ok_or("Body Fore delivery capacity overflow")?;
            output_bytes = output_bytes
                .checked_add(
                    count
                        .checked_mul(port.byte_capacity as usize)
                        .ok_or("Body Fore delivery byte capacity overflow")?,
                )
                .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                .ok_or("Body Fore delivery byte capacity exceeded")?;
            let mut available = Vec::with_capacity(count);
            let identity = std::sync::Arc::new(crate::ExternalForeIdentity {
                front_port_id: port.front_port_id.clone(),
                track: port.track,
                value_kind: port.value_kind.clone(),
            });
            for _ in 0..count {
                available.push(ExternalForeDelivery::prepared(
                    identity.clone(),
                    port.byte_capacity as usize,
                ));
            }
            output_slots.push(OutputSlots {
                port: port.clone(),
                source_node: partitions.iter().find_map(|part| {
                    let cord = part.cords.iter().find(|cord| cord.spec.cord == port.cord)?;
                    match cord.spec.source {
                        conduit_kernel::CordEndpoint::Local { node, .. } => Some(node),
                        conduit_kernel::CordEndpoint::Remote(_) => None,
                    }
                }),
                available,
                input_origin: partitions.iter().find_map(|part| {
                    let cord = part.cords.iter().find(|cord| cord.spec.cord == port.cord)?;
                    let conduit_kernel::CordEndpoint::Local { node, .. } = cord.spec.source else {
                        return None;
                    };
                    part.host_calls.iter().find_map(|call| {
                        (call.node == node).then(||
                            super::super::keyboard_input_binding::InputRealization::from_contract(
                                call.contract_id.as_str()).map(|realization| (node, call.call, realization)))
                            .flatten()
                    })
                }),
                acknowledged_descriptor: None,
                acknowledged_physical_event: None,
            });
        }
        Ok(Self {
            inputs,
            ports,
            output_slots,
            deliveries: Vec::with_capacity(delivery_capacity),
            pending_output: None,
            started: false,
            acknowledged_inputs: VecDeque::with_capacity(MAX_COMPLETED_INPUT_EVENTS),
        })
    }

    pub(super) fn input_ports(&self) -> impl Iterator<Item = &LoweredForePort> {
        self.ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
    }

    pub(super) fn sign_items(&self) -> Result<u16, String> {
        external_fore_feeder::remote_sign_items(&self.ports, self.inputs.input_items())
    }

    pub(super) fn has_ports(&self) -> bool {
        !self.ports.is_empty()
    }

    pub(super) fn start_and_feed(
        &mut self,
        scheduler: &mut InstalledScheduler,
    ) -> Result<(), String> {
        if !self.started {
            // Ingress lifecycle Signs belong to the admitted Body Play.
            self.inputs.start(scheduler)?;
            self.started = true;
        }
        self.inputs.feed_next(scheduler)
    }

    pub(super) fn activity_generation(&self) -> Option<u64> {
        self.inputs.activity_generation()
    }

    pub(super) fn wait_for_activity(&self, observed: u64) -> Result<bool, String> {
        self.inputs.wait_for_activity(observed)
    }

    pub(super) fn drain(
        &mut self,
        scheduler: &mut InstalledScheduler,
        output: &mut Option<&mut dyn BodyForeOutputAdapter>,
        mut input: Option<&mut super::super::keyboard_input_host::KeyboardInputHost<'_>>,
        mut map: Option<&mut super::parsec_acknowledged_map::ParsecAcknowledgedMap>,
    ) -> Result<(), String> {
        if self.pending_output.is_some() {
            return Err("Body Fore stopped output cannot be retried".into());
        }
        for slots in &mut self.output_slots {
            let planned = &slots.port;
            while let Some(offer) = scheduler
                .remote_egress_offer(planned.endpoint, planned.cord)
                .map_err(|error| format!("Body Fore offer: {error:?}"))?
            {
                if self.deliveries.len() == self.deliveries.capacity() {
                    return Err("Body Fore delivery capacity exceeded".into());
                }
                let bytes = scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("Body Fore value: {error:?}"))?;
                planned
                    .validate_value(bytes)
                    .map_err(|error| format!("Body Fore output contract: {error:?}"))?;
                let completed = if let (Some(input), Some((node, call, realization))) =
                    (input.as_deref_mut(), slots.input_origin)
                {
                    let event = input.claim_completed_event(node, call, offer.value)?;
                    if event.realization != realization
                        || slots
                            .acknowledged_descriptor
                            .is_some_and(|index| index != event.descriptor)
                        || slots
                            .acknowledged_physical_event
                            .is_some_and(|prior| prior >= event.physical_event)
                    {
                        return Err(
                            "Body input observation differs from its completed source custody"
                                .into(),
                        );
                    }
                    if self.acknowledged_inputs.len() == MAX_COMPLETED_INPUT_EVENTS {
                        return Err("acknowledged input staging bound exceeded".into());
                    }
                    Some((node, call, event.descriptor, event.physical_event))
                } else {
                    None
                };
                let mut delivery = slots
                    .available
                    .pop()
                    .ok_or("Body Fore delivery slots exhausted")?;
                delivery.fill_prepared(offer.sequence, bytes)?;
                let delivery = match custody::deliver(scheduler, output, planned, delivery) {
                    Ok(delivery) => delivery,
                    Err((pending, error)) => {
                        self.pending_output = Some(pending);
                        return Err(error);
                    }
                };
                // Retain confirmed delivery even if a subsequent source/map
                // acknowledgement fails. Those are distinct custody facts.
                let delivery_index = self.deliveries.len();
                self.deliveries.push(delivery);
                let delivery = &self.deliveries[delivery_index];
                if slots.input_origin.is_none() {
                    if let Some(map) = map.as_deref_mut() {
                        map.acknowledge_lowered(
                            planned,
                            slots
                                .source_node
                                .ok_or("Parsec states Fore lost its local source")?,
                            offer.sequence,
                            &delivery.bytes,
                        )?;
                    }
                }
                if let Some((node, call, descriptor, physical_event)) = completed {
                    let event = input
                        .as_deref_mut()
                        .ok_or("Body input observation lost its custody")?
                        .acknowledge_completed_event(node, call, offer.value)?;
                    self.acknowledged_inputs.push_back(AcknowledgedInput {
                        event,
                        delivery_index,
                    });
                    slots.acknowledged_descriptor = Some(descriptor);
                    slots.acknowledged_physical_event = Some(physical_event);
                }
            }
        }
        Ok(())
    }

    /// Called only after the complete Fore drain, including state acknowledgements.
    pub(super) fn take_acknowledged_input(&mut self) -> Option<AcknowledgedInput> {
        self.acknowledged_inputs.pop_front()
    }

    pub(super) fn acknowledged_input_bytes(&self, input: &AcknowledgedInput) -> &[u8] {
        // Delivery history is retained for the whole Play; the index is minted
        // only after acknowledgement and before that exact delivery is appended.
        &self.deliveries[input.delivery_index].bytes
    }

    pub(super) fn require_normal_terminal(
        &self,
        scheduler: &InstalledScheduler,
    ) -> Result<(), String> {
        for slots in &self.output_slots {
            let planned = &slots.port;
            if !scheduler
                .remote_egress_terminal(planned.endpoint, planned.cord)
                .map_err(|error| format!("Body Fore terminal: {error:?}"))?
            {
                return Err(format!(
                    "Body Fore output '{}' did not reach an observed normal terminal",
                    planned.front_port_id.as_str(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn into_custody(self) -> (Vec<ExternalForeDelivery>, Option<BodyForePendingOutput>) {
        (self.deliveries, self.pending_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unplanned() -> [ExternalForeInput; 1] {
        [ExternalForeInput {
            front_port_id: conduit_core::PortId::from("unplanned"),
            track: conduit_core::ConnectionTrack::Payload,
            bytes: vec![1],
        }]
    }

    #[test]
    fn unplanned_value_refuses_in_fore_preparation() {
        let supplied = unplanned();
        let refusal = BodyForeRoute::prepare(&[], &supplied, false, false)
            .err()
            .expect("unplanned Fore value must refuse");
        assert!(refusal.contains("does not match the sealed Plan"));
    }

    #[test]
    fn unplanned_sequence_refuses_in_fore_preparation() {
        let supplied = unplanned();
        let refusal = BodyForeRoute::prepare(&[], &supplied, true, false)
            .err()
            .expect("unplanned Fore Flow must refuse");
        assert!(refusal.contains("requires one to 64 admitted input values"));
    }
}
