//! Browser production-kernel half of the exact split Text Lab Plan.

use crate::presentation_nucleus::uppercase_utf8;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RemoteEndpointId,
    RequestId,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection, MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_std_catalog::{
    exact_text_lab_split_plan, MAX_TEXT_BYTES, TEXT_LAB_BROWSER_HOST, TEXT_LAB_MAXIMUM_VALUES,
    TEXT_UPPER_KIND,
};

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const SIGN_ITEMS: u16 = 128;

type BrowserTextLabScheduler = FixedScheduler<
    OperationDriver<UpperOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    1,
    2,
    PORTS,
    2,
    1,
    1,
    1,
    1,
>;

struct UpperOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl UpperOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for UpperOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < TEXT_LAB_MAXIMUM_VALUES as u32 => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(value) => value,
                        Err(_) => return Self::fail(1),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return Self::fail(2);
                };
                self.pending = None;
                self.next += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => Self::fail(3),
        }
    }
}

pub struct BrowserTextLabFragment {
    scheduler: BrowserTextLabScheduler,
    lowered: LoweredPlanFragment,
}

impl BrowserTextLabFragment {
    pub fn prepare(base_instance: &str) -> Result<Self, String> {
        let exact = exact_text_lab_split_plan(base_instance)?;
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
            || lowered.host_operations.len() != 1
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
        let mut bindings = FixedHostOperationBindings::<1>::new(1);
        bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|error| format!("{error:?}"))?;
        bindings.seal().map_err(|error| format!("{error:?}"))?;
        let values = HostedValueStore::new(2, MAX_TEXT_BYTES, MAX_TEXT_BYTES * 2)
            .map_err(|error| format!("{error:?}"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "split Text Lab browser Sign budget overflow".to_string())?;
        let signs =
            HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|error| format!("{error:?}"))?;
        let driver = OperationDriver::new(UpperOperation {
            pending: None,
            next: 0,
        })
        .map_err(|error| format!("{error:?}"))?;
        let scheduler = BrowserTextLabScheduler::new_with_host_operations(
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
            [driver],
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

    pub fn execute_value(&mut self, sequence: u64, input: &[u8]) -> Result<Vec<u8>, String> {
        let (ingress, ingress_cord) = self.endpoint(RemoteCordDirection::Ingress);
        self.scheduler
            .admit_remote_input(ingress, ingress_cord, sequence, input)
            .map_err(|error| format!("{error:?}"))?;
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
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
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
                let output = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?
                    .to_vec();
                self.scheduler
                    .remote_egress_accept(egress, egress_cord, offer.sequence)
                    .map_err(|error| format!("{error:?}"))?;
                self.scheduler
                    .remote_egress_delivered(egress, egress_cord, offer.sequence)
                    .map_err(|error| format!("{error:?}"))?;
                return Ok(output);
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
                SchedulerStatus::Complete => {
                    return Err("split Text Lab browser kernel completed before output".into())
                }
                SchedulerStatus::Cancelled => {
                    return Err("split Text Lab browser kernel cancelled".into())
                }
            }
        }
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
