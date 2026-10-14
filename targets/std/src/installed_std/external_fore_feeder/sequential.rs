//! Bounded multi-Flow ingress through the existing installed Kernel loop.
use super::*;
use conduit_core::{ConnectionTrack, PortId};
struct SequenceFlow {
    front: PortId,
    track: ConnectionTrack,
    targets: Vec<(RemoteEndpointId, CordId)>,
    sequence: u64,
}
pub(in crate::installed_std) struct SequentialForeFeeder<'a> {
    inputs: &'a [ExternalForeInput],
    flows: Vec<SequenceFlow>,
    input_flows: Vec<usize>,
    next: usize,
    closed: bool,
}
impl<'a> SequentialForeFeeder<'a> {
    pub(in crate::installed_std) fn prepare(
        planned: &[&LoweredForePort],
        inputs: &'a [ExternalForeInput],
    ) -> Result<Self, String> {
        if planned.is_empty() || inputs.is_empty() || inputs.len() > usize::from(MAX_SEQUENCE_ITEMS)
        {
            return Err("external Fore sequence requires one to 64 admitted input values".into());
        }
        if planned.iter().any(|port| {
            port.direction != PortDirection::Input
                || !matches!(port.temporal, PortTemporal::Flow { closes: true })
        }) {
            return Err("external Fore sequence requires exact closing input Flows".into());
        }
        let mut flows: Vec<SequenceFlow> = Vec::new();
        for port in planned {
            if let Some(flow) = flows
                .iter_mut()
                .find(|flow| flow.front == port.front_port_id && flow.track == port.track)
            {
                flow.targets.push((port.endpoint, port.cord));
            } else {
                if flows.len() == usize::from(MAX_SEQUENCE_ITEMS) {
                    return Err("external Fore sequence exceeds its input Flow bound".into());
                }
                flows.push(SequenceFlow {
                    front: port.front_port_id.clone(),
                    track: port.track,
                    targets: vec![(port.endpoint, port.cord)],
                    sequence: 0,
                });
            }
        }
        let mut input_flows = Vec::with_capacity(inputs.len());
        for input in inputs {
            let index = flows
                .iter()
                .position(|flow| flow.front == input.front_port_id && flow.track == input.track)
                .ok_or("external Fore sequence does not match its sealed input Flow")?;
            if planned
                .iter()
                .filter(|port| {
                    port.front_port_id == input.front_port_id && port.track == input.track
                })
                .any(|port| port.validate_value(&input.bytes).is_err())
            {
                return Err("external Fore sequence violates its sealed value contract".into());
            }
            input_flows.push(index);
        }
        Ok(Self {
            inputs,
            flows,
            input_flows,
            next: 0,
            closed: false,
        })
    }
    pub(in crate::installed_std) fn input_items(&self) -> u16 {
        self.inputs.len() as u16
    }
    #[cfg(test)]
    pub(in crate::installed_std) fn is_pending(&self) -> bool {
        !self.closed
    }
    pub(in crate::installed_std) fn feed_next(
        &mut self,
        ingress: &mut impl ForeIngress,
    ) -> Result<(), String> {
        if self.closed || ingress.has_ready_work() {
            return Ok(());
        }
        // The caller invokes this only after draining current Kernel work.
        // Last payload admission is not closure: let every stage consume it.
        if self.next == self.inputs.len() {
            for flow in &self.flows {
                for &(endpoint, cord) in &flow.targets {
                    ingress.close(endpoint, cord)?;
                }
            }
            self.closed = true;
            return Ok(());
        }
        let input = &self.inputs[self.next];
        let flow = &mut self.flows[self.input_flows[self.next]];
        match ingress.admit(&flow.targets, flow.sequence, &input.bytes)? {
            RemoteIngressOutcome::Accepted { sequence } if sequence == flow.sequence => {
                flow.sequence += 1; // At most 64 admitted values in this prepared run.
                self.next += 1;
            }
            RemoteIngressOutcome::Full { sequence } if sequence == flow.sequence => {}
            other => {
                return Err(format!(
                    "external Fore sequence was not admitted: {other:?}"
                ))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../external_fore_feeder_tests.rs"]
mod tests;
