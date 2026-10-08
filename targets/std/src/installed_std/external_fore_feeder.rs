//! Finite, single-Flow external Fore input for an ordinary installed Play.
//! Values stay in caller-owned preparation storage; only one is admitted per
//! loop turn, using the exact same numeric cords and sequence on every branch.

use std::collections::BTreeSet;

use crate::{installed_std::InstalledScheduler, BodyLiveForeQueue, ExternalForeInput};
use conduit_core::{PortDirection, PortTemporal};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_kernel::{CordId, RemoteEndpointId};
use conduit_plan_lowering::lowering::LoweredForePort;

pub(super) const MAX_SEQUENCE_ITEMS: u16 = 64;

pub(super) enum PreparedForeInputs<'a> {
    Single(Vec<SingleInput<'a>>),
    Sequential(SequentialForeFeeder<'a>),
    Live(Box<LiveForeFeeder>),
}

pub(super) struct SingleInput<'a> {
    bytes: &'a [u8],
    targets: Vec<(RemoteEndpointId, CordId)>,
}

impl<'a> PreparedForeInputs<'a> {
    pub(super) fn prepare_live(
        planned: &[&LoweredForePort],
        queue: &BodyLiveForeQueue,
    ) -> Result<Self, String> {
        LiveForeFeeder::prepare(planned, queue).map(|feeder| Self::Live(Box::new(feeder)))
    }
    pub(super) fn prepare(
        planned: &[&LoweredForePort],
        supplied: &'a [ExternalForeInput],
        sequential: bool,
    ) -> Result<Self, String> {
        if sequential {
            return SequentialForeFeeder::prepare(planned, supplied).map(Self::Sequential);
        }
        let planned_keys = planned
            .iter()
            .map(|port| (&port.front_port_id, port.track))
            .collect::<BTreeSet<_>>();
        let supplied_keys = supplied
            .iter()
            .map(|input| (&input.front_port_id, input.track))
            .collect::<BTreeSet<_>>();
        if supplied.len() != planned_keys.len() || supplied_keys != planned_keys {
            return Err("external Fore input set does not match the sealed Plan".into());
        }
        let mut entries = Vec::with_capacity(planned_keys.len());
        for (front_port_id, track) in planned_keys {
            let input = supplied
                .iter()
                .find(|input| input.front_port_id == *front_port_id && input.track == track)
                .ok_or_else(|| {
                    format!(
                        "external Fore input '{}' is missing",
                        front_port_id.as_str()
                    )
                })?;
            let branches = planned
                .iter()
                .copied()
                .filter(|port| port.front_port_id == *front_port_id && port.track == track)
                .collect::<Vec<_>>();
            if branches
                .iter()
                .any(|port| port.validate_value(&input.bytes).is_err())
            {
                return Err(format!(
                    "external Fore input '{}' violates its sealed value contract",
                    front_port_id.as_str()
                ));
            }
            entries.push(SingleInput {
                bytes: &input.bytes,
                targets: branches
                    .iter()
                    .map(|port| (port.endpoint, port.cord))
                    .collect(),
            });
        }
        Ok(Self::Single(entries))
    }

    pub(super) fn input_items(&self) -> u16 {
        match self {
            Self::Single(_) => 1,
            Self::Sequential(feeder) => feeder.inputs.len() as u16,
            Self::Live(feeder) => feeder.queue.maximum_items(),
        }
    }

    pub(super) fn start(&mut self, scheduler: &mut InstalledScheduler) -> Result<(), String> {
        let Self::Single(entries) = self else {
            return Ok(());
        };
        for entry in entries {
            match scheduler
                .admit_remote_input_fanout(&entry.targets, 0, entry.bytes)
                .map_err(|error| format!("admit external Fore input: {error:?}"))?
            {
                RemoteIngressOutcome::Accepted { sequence: 0 } => {}
                outcome => {
                    return Err(format!(
                        "external Fore input was not admitted exactly: {outcome:?}"
                    ))
                }
            }
            for &(endpoint, cord) in &entry.targets {
                scheduler
                    .close_remote_input(endpoint, cord)
                    .map_err(|error| format!("close external Fore input: {error:?}"))?;
            }
        }
        Ok(())
    }

    pub(super) fn feed_next(&mut self, scheduler: &mut InstalledScheduler) -> Result<(), String> {
        match self {
            Self::Sequential(feeder) => feeder.feed_next(scheduler)?,
            Self::Live(feeder) => feeder.feed_next(scheduler)?,
            Self::Single(_) => {}
        }
        Ok(())
    }

    pub(super) fn activity_generation(&self) -> Option<u64> {
        match self {
            Self::Live(feeder) if !feeder.closed => Some(feeder.queue.observed_generation()),
            _ => None,
        }
    }

    pub(super) fn wait_for_activity(&self, observed: u64) -> Result<bool, String> {
        let Self::Live(feeder) = self else {
            return Ok(false);
        };
        if feeder.closed {
            return Ok(false);
        }
        if feeder.full {
            return Err("live Fore input remained pressured without kernel progress".into());
        }
        feeder.queue.wait_if_empty(observed);
        Ok(true)
    }
}

pub(super) struct LiveForeFeeder {
    queue: BodyLiveForeQueue,
    targets: Vec<(RemoteEndpointId, CordId)>,
    closed: bool,
    full: bool,
}

impl LiveForeFeeder {
    fn prepare(planned: &[&LoweredForePort], queue: &BodyLiveForeQueue) -> Result<Self, String> {
        let expected = queue.port();
        if planned.len() != 1
            || planned.iter().any(|port| {
                port.direction != PortDirection::Input
                    || port.front_port_id != expected.front_port_id
                    || port.track != expected.track
                    || port.value_kind != expected.value_kind
                    || port.value_contract != expected.value_contract
                    || port.temporal != expected.temporal
                    || port.byte_capacity != expected.byte_capacity
                    || port.item_capacity != expected.item_capacity
                    || port.selected_line.is_some()
            })
        {
            return Err("live Fore queue differs from the sealed local input Flow".into());
        }
        Ok(Self {
            queue: queue.clone(),
            targets: planned
                .iter()
                .map(|port| (port.endpoint, port.cord))
                .collect(),
            closed: false,
            full: false,
        })
    }

    fn feed_next(&mut self, ingress: &mut impl ForeIngress) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        let (outcome, should_close) = self.queue.with_front(|front, should_close| {
            let outcome = front
                .map(|(sequence, bytes)| ingress.admit(&self.targets, sequence, bytes))
                .transpose()?;
            Ok::<_, String>((outcome, should_close))
        })?;
        self.full = false;
        match outcome {
            Some(RemoteIngressOutcome::Accepted { sequence }) => {
                self.queue.acknowledge(sequence)?;
            }
            Some(RemoteIngressOutcome::Full { sequence }) => {
                let expected = self
                    .queue
                    .with_front(|front, _| front.map(|(next, _)| next));
                if expected != Some(sequence) {
                    return Err("live Fore pressure changed the pending sequence".into());
                }
                self.full = true;
            }
            None if should_close => {
                for &(endpoint, cord) in &self.targets {
                    ingress.close(endpoint, cord)?;
                }
                self.queue.mark_kernel_closed()?;
                self.closed = true;
            }
            None => {}
        }
        Ok(())
    }
}

pub(super) fn remote_sign_items(
    ports: &[LoweredForePort],
    input_items: u16,
) -> Result<u16, String> {
    ports.iter().try_fold(0_u16, |total, port| {
        let events = match port.direction {
            PortDirection::Input => port.item_capacity.max(input_items).checked_add(1),
            PortDirection::Output => port
                .item_capacity
                .max(input_items)
                .checked_mul(3)
                .and_then(|count| count.checked_add(1)),
        }
        .ok_or_else(|| "external Fore lifecycle Sign capacity overflow".to_string())?;
        total
            .checked_add(events)
            .ok_or_else(|| "external Fore lifecycle Sign capacity overflow".to_string())
    })
}

pub(super) struct SequentialForeFeeder<'a> {
    inputs: &'a [ExternalForeInput],
    targets: Vec<(RemoteEndpointId, CordId)>,
    next: usize,
    closed: bool,
}

impl<'a> SequentialForeFeeder<'a> {
    pub(super) fn prepare(
        planned: &[&LoweredForePort],
        inputs: &'a [ExternalForeInput],
    ) -> Result<Self, String> {
        if planned.is_empty() || inputs.is_empty() || inputs.len() > usize::from(MAX_SEQUENCE_ITEMS)
        {
            return Err("external Fore sequence requires one to 64 admitted input values".into());
        }
        let first = planned[0];
        if first.direction != PortDirection::Input
            || !matches!(first.temporal, PortTemporal::Flow { closes: true })
            || planned.iter().any(|port| {
                port.direction != PortDirection::Input
                    || port.front_port_id != first.front_port_id
                    || port.track != first.track
                    || port.temporal != first.temporal
            })
        {
            return Err("external Fore sequence requires one exact input Flow".into());
        }
        for input in inputs {
            if input.front_port_id != first.front_port_id || input.track != first.track {
                return Err("external Fore sequence does not match its sealed input Flow".into());
            }
            if planned
                .iter()
                .any(|port| port.validate_value(&input.bytes).is_err())
            {
                return Err("external Fore sequence violates its sealed value contract".into());
            }
        }
        Ok(Self {
            inputs,
            targets: planned
                .iter()
                .map(|port| (port.endpoint, port.cord))
                .collect(),
            next: 0,
            closed: false,
        })
    }

    #[cfg(test)]
    pub(super) fn is_pending(&self) -> bool {
        !self.closed
    }

    pub(super) fn feed_next(&mut self, ingress: &mut impl ForeIngress) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        let input = &self.inputs[self.next];
        let sequence = u64::try_from(self.next)
            .map_err(|_| "external Fore sequence index overflow".to_string())?;
        match ingress.admit(&self.targets, sequence, &input.bytes)? {
            RemoteIngressOutcome::Accepted { sequence: accepted } if accepted == sequence => {
                self.next += 1;
                if self.next == self.inputs.len() {
                    for &(endpoint, cord) in &self.targets {
                        ingress.close(endpoint, cord)?;
                    }
                    self.closed = true;
                }
            }
            RemoteIngressOutcome::Full { sequence: refused } if refused == sequence => {}
            other => {
                return Err(format!(
                    "external Fore sequence was not admitted: {other:?}"
                ))
            }
        }
        Ok(())
    }
}

pub(super) trait ForeIngress {
    fn admit(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String>;
    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String>;
}

impl ForeIngress for InstalledScheduler {
    fn admit(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        self.admit_remote_input_fanout(targets, sequence, bytes)
            .map_err(|error| format!("admit external Fore sequence: {error:?}"))
    }

    fn close(&mut self, endpoint: RemoteEndpointId, cord: CordId) -> Result<(), String> {
        self.close_remote_input(endpoint, cord)
            .map_err(|error| format!("close external Fore sequence: {error:?}"))
    }
}

#[cfg(test)]
#[path = "external_fore_feeder_tests.rs"]
mod tests;
