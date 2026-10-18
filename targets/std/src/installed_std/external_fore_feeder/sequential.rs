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
    pub(super) stream: Option<crate::ExternalForeStreamStats>,
}
impl<'a> SequentialForeFeeder<'a> {
    pub(in crate::installed_std) fn prepare(
        planned: &[&LoweredForePort],
        inputs: &'a [ExternalForeInput],
    ) -> Result<Self, String> {
        Self::prepare_bounded(planned, inputs, MAX_SEQUENCE_ITEMS, None)
    }
    pub(in crate::installed_std) fn prepare_stream(
        planned: &[&LoweredForePort],
        inputs: &'a [ExternalForeInput],
        config: crate::ExternalForeStreamConfig,
    ) -> Result<Self, String> {
        config.validate()?;
        Self::prepare_bounded(
            planned,
            inputs,
            crate::ExternalForeStreamConfig::MAX_INPUTS,
            Some(crate::ExternalForeStreamStats {
                admission_batch: config.admission_batch,
                ..Default::default()
            }),
        )
    }
    fn prepare_bounded(
        planned: &[&LoweredForePort],
        inputs: &'a [ExternalForeInput],
        maximum: u16,
        stream: Option<crate::ExternalForeStreamStats>,
    ) -> Result<Self, String> {
        if planned.is_empty() || inputs.is_empty() || inputs.len() > usize::from(maximum) {
            return Err(format!(
                "external Fore sequence requires one to {maximum} admitted input values"
            ));
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
            stream,
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
        if self.closed || (self.stream.is_none() && ingress.has_ready_work()) {
            return Ok(());
        }
        // Closure follows drained payloads in both modes. Overlapping ingress
        // changes admission cadence, never the kernel's value-before-close law.
        if self.next == self.inputs.len() {
            if ingress.has_ready_work() {
                return Ok(());
            }
            for flow in &self.flows {
                for &(endpoint, cord) in &flow.targets {
                    ingress.close(endpoint, cord)?;
                }
            }
            self.closed = true;
            return Ok(());
        }
        let batch = if let Some(stats) = &mut self.stream {
            stats.batches += 1;
            stats.admission_batch
        } else {
            1
        };
        for _ in 0..batch {
            if self.next == self.inputs.len() {
                break;
            }
            let input = &self.inputs[self.next];
            let flow = &mut self.flows[self.input_flows[self.next]];
            if let Some(stats) = &mut self.stream {
                stats.attempts += 1;
            }
            match ingress.admit(&flow.targets, flow.sequence, &input.bytes)? {
                RemoteIngressOutcome::Accepted { sequence } if sequence == flow.sequence => {
                    flow.sequence += 1; // Bounded by the admitted finite input lifetime.
                    self.next += 1;
                    if let Some(stats) = &mut self.stream {
                        stats.accepted += 1;
                    }
                }
                RemoteIngressOutcome::Full { sequence } if sequence == flow.sequence => {
                    if let Some(stats) = &mut self.stream {
                        stats.full += 1;
                    }
                    break;
                }
                other => {
                    return Err(format!(
                        "external Fore sequence was not admitted: {other:?}"
                    ))
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod stream_tests;
#[cfg(test)]
#[path = "../external_fore_feeder_tests.rs"]
mod tests;
