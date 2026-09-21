//! Browser production-kernel half of the exact split Text Lab Plan.

use crate::presentation_nucleus::uppercase_utf8;
use conduit_kernel::scheduler::{
    FixedScheduler, SchedulerStatus, StepInputBytes, StepIo, StepOperation, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostCallBindings, FixedRoutes,
    HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, PortId,
    RemoteEndpointId, RequestId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_semantic_catalog::{
    exact_text_lab_split_plan, TEXT_LAB_BROWSER_HOST, TEXT_LAB_MAXIMUM_VALUES,
};
use conduit_text::{MAX_TEXT_BYTES, TEXT_UPPER_KIND};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGN_ITEMS: u16 = 128;

type BrowserTextLabScheduler =
    FixedScheduler<UpperBack, HostedValueStore, HostedSignLog, 1, 2, PORTS, 2, 1, 1, 1, 1>;

struct UpperBack {
    pending: Option<RequestId>,
    next: u32,
}

impl UpperBack {
    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl StepOperation<PORTS> for UpperBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(expected) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != expected
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return Self::fail(3);
            }
            let Some(output) = outcome.output else {
                return Self::fail(2);
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() || io.send(PortId(0), output.value).is_err() {
                return Self::fail(3);
            }
            self.pending = None;
            self.next += 1;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.next >= TEXT_LAB_MAXIMUM_VALUES as u32 {
                return Self::fail(3);
            }
            let request = RequestId(self.next);
            let input = match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                Ok(value) => value,
                Err(_) => return Self::fail(1),
            };
            if io.consume(PortId(0)).is_err()
                || io.request_host_call(request, HostCallId(0), input).is_err()
            {
                return Self::fail(3);
            }
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return Self::fail(3);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

pub struct BrowserTextLabFragment {
    scheduler: BrowserTextLabScheduler,
    lowered: LoweredPlanFragment,
}

pub struct BrowserTextOffer {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

impl BrowserTextLabFragment {
    pub fn prepare(base_instance: &str) -> Result<Self, String> {
        let exact = exact_text_lab_split_plan(
            base_instance,
            &crate::presentation_nucleus::browser_text_upper_offer(),
        )?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id.as_str() == TEXT_LAB_BROWSER_HOST)
            .ok_or_else(|| "split Text Lab browser fragment is missing".to_string())?;
        let lowered = lower_plan_fragment(fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 2
            || lowered.remote_endpoints.len() != 2
            || lowered.host_calls.len() != 1
            || fragment.placements[0].kind_id.as_str() != TEXT_UPPER_KIND
        {
            return Err("split Text Lab browser fragment has the wrong exact shape".into());
        }
        let directions = lowered
            .remote_endpoints
            .iter()
            .map(|endpoint| endpoint.direction)
            .collect::<Vec<_>>();
        if !directions.contains(&RemoteCordDirection::Ingress)
            || !directions.contains(&RemoteCordDirection::Egress)
        {
            return Err("split Text Lab browser fragment lacks both Line directions".into());
        }
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|error| format!("{error:?}"))?;
        }
        routes.seal().map_err(|error| format!("{error:?}"))?;
        let mut bindings = FixedHostCallBindings::<1>::new(1);
        bindings
            .install(lowered.host_calls[0].node, lowered.host_calls[0].binding)
            .map_err(|error| format!("{error:?}"))?;
        bindings.seal().map_err(|error| format!("{error:?}"))?;
        let values = HostedValueStore::new(2, MAX_TEXT_BYTES, MAX_TEXT_BYTES * 2)
            .map_err(|error| format!("{error:?}"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "split Text Lab browser Sign budget overflow".to_string())?;
        let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(SIGN_ITEMS)
            .ok_or_else(|| "split Text Lab browser remote Sign budget overflow".to_string())?;
        let signs = HostedSignLog::new_with_remote_storage(
            SIGN_ITEMS,
            sign_bytes,
            SIGN_ITEMS,
            remote_sign_bytes,
        )
        .map_err(|error| format!("{error:?}"))?;
        let back = UpperBack {
            pending: None,
            next: 0,
        };
        let scheduler = BrowserTextLabScheduler::new_with_host_calls(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "split Text Lab browser node width".to_string())?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "split Text Lab browser Cord width".to_string())?,
            routes,
            bindings,
            [back],
            values,
            signs,
        )
        .map_err(|error| format!("{error:?}"))?;
        Ok(Self { scheduler, lowered })
    }

    fn endpoint(&self, direction: RemoteCordDirection) -> (RemoteEndpointId, CordId) {
        let endpoint = self
            .lowered
            .remote_endpoints
            .iter()
            .find(|endpoint| endpoint.direction == direction)
            .expect("exact split Text Lab direction was checked");
        (endpoint.endpoint, endpoint.cord)
    }

    pub fn admit_text(&mut self, sequence: u64, input: &[u8]) -> Result<(), String> {
        let (ingress, ingress_cord) = self.endpoint(RemoteCordDirection::Ingress);
        self.scheduler
            .admit_remote_input(ingress, ingress_cord, sequence, input)
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    pub fn next_upper_offer(&mut self) -> Result<BrowserTextOffer, String> {
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                let input = self
                    .scheduler
                    .host_value(request.input.value)
                    .map_err(|error| format!("{error:?}"))?
                    .to_vec();
                let output = uppercase_utf8(&input)?;
                let value = self
                    .scheduler
                    .store_host_value(&output)
                    .map_err(|error| format!("{error:?}"))?;
                self.scheduler
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(value, MAX_TEXT_BYTES)
                                    .map_err(|error| format!("{error:?}"))?,
                            ),
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("{error:?}"))?;
                continue;
            }
            let (egress, egress_cord) = self.endpoint(RemoteCordDirection::Egress);
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(egress, egress_cord)
                .map_err(|error| format!("{error:?}"))?
            {
                let bytes = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?
                    .to_vec();
                return Ok(BrowserTextOffer {
                    sequence: offer.sequence,
                    bytes,
                });
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("split Text Lab browser kernel became idle".into())
                }
                SchedulerStatus::Drained => {
                    return Err("split Text Lab browser kernel completed before output".into())
                }
                SchedulerStatus::Cancelled => {
                    return Err("split Text Lab browser kernel cancelled".into())
                }
            }
        }
    }

    pub fn accept_upper(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn deliver_upper(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_delivered(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn execute_value(&mut self, sequence: u64, input: &[u8]) -> Result<Vec<u8>, String> {
        self.admit_text(sequence, input)?;
        let offer = self.next_upper_offer()?;
        self.accept_upper(offer.sequence)?;
        self.deliver_upper(offer.sequence)?;
        Ok(offer.bytes)
    }

    pub fn close_text_input(&mut self) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Ingress);
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish(&mut self) -> Result<(), String> {
        loop {
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("split Text Lab browser became idle at finish".into())
                }
                SchedulerStatus::Drained => break,
                SchedulerStatus::Cancelled => return Err("split Text Lab browser cancelled".into()),
            }
        }
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
        if !self
            .scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(|error| format!("{error:?}"))?
        {
            return Err("split Text Lab browser return Cord is not terminal".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_browser_fragment_uppercases_five_values_through_the_production_kernel() {
        let mut browser = BrowserTextLabFragment::prepare("ws://127.0.0.1:1/conduit").unwrap();
        let inputs = ["h", "e", "l", "l", "o"];
        let mut output = String::new();
        for (sequence, input) in inputs.into_iter().enumerate() {
            let value = browser
                .execute_value(sequence as u64, input.as_bytes())
                .unwrap();
            output.push_str(core::str::from_utf8(&value).unwrap());
        }
        assert_eq!(output, "HELLO");
    }
}
