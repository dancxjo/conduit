//! Exact, finite local Fore boundary for one installed Body Play.
use crate::{
    body_execution::BodyForeOutputAdapter, BodyLiveForeQueue, ExternalForeDelivery,
    ExternalForeInput,
};
use conduit_core::PortDirection;
use conduit_plan_lowering::lowering::{LoweredForePort, LoweredPlanFragment};

use super::super::{
    external_fore_feeder::{self, PreparedForeInputs},
    InstalledScheduler,
};

pub(super) struct BodyForeRoute<'a> {
    inputs: PreparedForeInputs<'a>,
    ports: Vec<LoweredForePort>,
    output_slots: Vec<OutputSlots>,
    deliveries: Vec<ExternalForeDelivery>,
    started: bool,
}

struct OutputSlots {
    port: LoweredForePort,
    available: Vec<ExternalForeDelivery>,
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
            for _ in 0..count {
                available.push(ExternalForeDelivery {
                    front_port_id: port.front_port_id.clone(),
                    track: port.track,
                    value_kind: port.value_kind.clone(),
                    sequence: 0,
                    bytes: Vec::with_capacity(port.byte_capacity as usize),
                });
            }
            output_slots.push(OutputSlots {
                port: port.clone(),
                available,
            });
        }
        Ok(Self {
            inputs,
            ports,
            output_slots,
            deliveries: Vec::with_capacity(delivery_capacity),
            started: false,
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
    ) -> Result<(), String> {
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
                let mut delivery = slots
                    .available
                    .pop()
                    .ok_or("Body Fore delivery slots exhausted")?;
                delivery.sequence = offer.sequence;
                delivery.bytes.clear();
                delivery.bytes.extend_from_slice(bytes);
                output
                    .as_deref_mut()
                    .ok_or("Body Fore output adapter lost")?
                    .deliver(&delivery)?;
                scheduler
                    .remote_egress_accept(planned.endpoint, planned.cord, offer.sequence)
                    .map_err(|error| format!("Body Fore accept: {error:?}"))?;
                scheduler
                    .remote_egress_delivered(planned.endpoint, planned.cord, offer.sequence)
                    .map_err(|error| format!("Body Fore acknowledge: {error:?}"))?;
                self.deliveries.push(delivery);
            }
        }
        Ok(())
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

    pub(super) fn into_deliveries(self) -> Vec<ExternalForeDelivery> {
        self.deliveries
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
