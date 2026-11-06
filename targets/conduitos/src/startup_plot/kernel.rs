//! Numeric admission into the shared production fixed scheduler.
use conduit_core::PlanFragment;
use conduit_kernel::{scheduler::*, *};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTES: usize = 2 * PORTS;
pub(super) type Kernel = FixedScheduler<
    Back,
    FixedValueStore<4, 128>,
    FixedSignLog<64>,
    2,
    1,
    PORTS,
    1,
    ROUTES,
    1,
    2,
    1,
>;
pub(super) enum Back {
    Wake(Option<ValueRef>),
    Chime {
        pending: Option<RequestId>,
        next: u32,
    },
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Wake(value) => {
                let Some(current) = *value else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), current).is_err() {
                    return invalid();
                }
                *value = None;
                StepOutcome::Complete
            }
            Self::Chime { pending, next } => {
                if let Some((request, outcome)) = io.host_completion() {
                    if Some(request) != *pending
                        || outcome.output.is_some()
                        || !match outcome.disposition {
                            HostCallDisposition::Completed => outcome.failure.is_none(),
                            HostCallDisposition::Denied => outcome
                                .failure
                                .is_some_and(|f| f.code == FailureCode::HostCallDenied),
                            HostCallDisposition::Failed => outcome
                                .failure
                                .is_some_and(|f| f.code == FailureCode::HostCallFailed),
                            HostCallDisposition::Cancelled => false,
                        }
                    {
                        return invalid();
                    }
                    if io.consume_host_completion().is_err() {
                        return invalid();
                    }
                    *pending = None;
                    let Some(value) = next.checked_add(1) else {
                        return invalid();
                    };
                    *next = value;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if pending.is_some() {
                        return invalid();
                    }
                    let Ok(input) =
                        BoundedValueRef::new(value, conduit_core::BOOL_ENCODED_LEN as u32)
                    else {
                        return invalid();
                    };
                    if io.consume(PortId(0)).is_err()
                        || io
                            .request_host_call(RequestId(*next), HostCallId(0), input)
                            .is_err()
                    {
                        return invalid();
                    }
                    *pending = Some(RequestId(*next));
                    return StepOutcome::Progress;
                }
                if pending.is_none() && io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return invalid();
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Wake(v) => *v = None,
            Self::Chime { pending, .. } => *pending = None,
        }
    }
}
fn invalid() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 1,
    })
}
pub(super) fn prepare(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    eligible: bool,
) -> Result<Kernel, &'static str> {
    let mut values = FixedValueStore::<4, 128>::new(128).map_err(|_| "startup-values")?;
    let pulse = if eligible {
        Some(
            values
                .store(&conduit_core::InfoBool::new(true).encode())
                .map_err(|_| "startup-pulse")?,
        )
    } else {
        None
    };
    let mut backs = [None, None];
    for (index, placement) in fragment.placements.iter().enumerate() {
        backs[index] = Some(match placement.implementation_id.as_str() {
            super::plan::WAKE => Back::Wake(pulse),
            super::plan::CHIME => Back::Chime {
                pending: None,
                next: 0,
            },
            _ => return Err("startup-back"),
        });
    }
    let [Some(first), Some(second)] = backs else {
        return Err("startup-backs");
    };
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "startup-node-bounds")?;
    let mut routes = FixedRoutes::<ROUTES, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| "startup-route")?;
    }
    routes.seal().map_err(|_| "startup-routes")?;
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| "startup-host-binding")?;
    }
    bindings.seal().map_err(|_| "startup-host-bindings")?;
    let signs = FixedSignLog::<64>::new((64 * core::mem::size_of::<KernelEvent>()) as u32)
        .map_err(|_| "startup-signs")?;
    FixedScheduler::new_with_host_calls(
        nodes,
        [lowered.cords[0].spec],
        routes,
        bindings,
        [first, second],
        values,
        signs,
    )
    .map_err(|_| "startup-kernel")
}
