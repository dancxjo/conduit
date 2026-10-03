use super::*;

const MASK_PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub(crate) type MaskScheduler =
    FixedScheduler<MaskBack, HostedValueStore, HostedSignLog, 1, 3, MASK_PORTS, 12, 48, 3, 1, 1>;

pub(crate) struct MaskBack {
    pending: bool,
}

impl StepBack<MASK_PORTS> for MaskBack {
    fn step(
        &mut self,
        io: &mut StepIo<MASK_PORTS>,
        _inputs: &StepInputBytes<'_, MASK_PORTS>,
    ) -> StepOutcome {
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return failure(1);
            }
            let Some(output) = outcome.output else {
                return failure(2);
            };
            let output_port = if request == RequestId(0) { 1 } else { 0 };
            if !io.output_ready(KernelPortId(output_port)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err()
                || io.send(KernelPortId(output_port), output.value).is_err()
            {
                return failure(3);
            }
            if request == RequestId(1) {
                self.pending = false;
                return StepOutcome::Complete;
            }
            let input = match BoundedValueRef::new(output.value, MASK_BYTES) {
                Ok(input) => input,
                Err(_) => return failure(6),
            };
            if io
                .request_host_call(RequestId(1), HostCallId(0), input)
                .is_err()
            {
                return failure(7);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(KernelPortId(0)) {
            let input = match BoundedValueRef::new(value, MASK_BYTES) {
                Ok(input) => input,
                Err(_) => return failure(4),
            };
            if io.consume(KernelPortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
            {
                return failure(5);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}

fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

pub(crate) fn exact_boundary<'a>(
    ports: &'a [LoweredForePort],
    name: &str,
    direction: PortDirection,
) -> Result<&'a LoweredForePort, String> {
    let mut matches = ports
        .iter()
        .filter(|port| port.front_port_id.as_str() == name && port.direction == direction);
    let port = matches
        .next()
        .ok_or_else(|| format!("browser Mask lacks sealed Fore port '{name}'"))?;
    if matches.next().is_some() {
        return Err(format!("browser Mask Fore port '{name}' is ambiguous"));
    }
    Ok(port)
}

pub(crate) fn mask_scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<MaskScheduler, String> {
    if fragment.placements.len() != 1 || lowered.cords.len() != 3 {
        return Err("browser Mask Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser Mask nodes")?;
    let cords = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "browser Mask cords")?;
    let mut routes = FixedRoutes::<48, 3>::new(MASK_PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install browser Mask route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal browser Mask routes: {error:?}"))?;
    let mut bindings = FixedHostCallBindings::<1>::new(1);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install browser Mask host call: {error:?}"))?;
    }
    bindings
        .seal()
        .map_err(|error| format!("seal browser Mask host calls: {error:?}"))?;
    let values = HostedValueStore::new(12, MASK_BYTES, MASK_BYTES * 4)
        .map_err(|error| format!("prepare browser Mask values: {error:?}"))?;
    const SIGN_ITEMS: u16 = 256;
    let sign_bytes = u32::from(SIGN_ITEMS)
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
        .ok_or("browser Mask Sign budget overflow")?;
    let remote_items = lowered
        .fore_ports
        .iter()
        .try_fold(0u16, |total, port| {
            let multiplier = if port.direction == PortDirection::Input {
                1
            } else {
                3
            };
            port.item_capacity
                .checked_mul(multiplier)
                .and_then(|items| items.checked_add(1))
                .and_then(|items| total.checked_add(items))
        })
        .ok_or("browser Mask remote Sign item budget overflow")?;
    let remote_bytes = conduit_kernel::remote_sign_storage_bytes(remote_items)
        .ok_or("browser Mask remote Sign budget overflow")?;
    let signs =
        HostedSignLog::new_with_remote_storage(SIGN_ITEMS, sign_bytes, remote_items, remote_bytes)
            .map_err(|error| format!("prepare browser Mask signs: {error:?}"))?;
    MaskScheduler::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [MaskBack { pending: false }],
        values,
        signs,
    )
    .map_err(|error| format!("prepare browser Mask scheduler: {error:?}"))
}

pub(super) fn drive_mask_to_terminal(
    scheduler: &mut MaskScheduler,
    boundaries: [&LoweredForePort; 2],
) -> Result<(), String> {
    for _ in 0..32 {
        if boundaries.iter().all(|port| {
            scheduler
                .remote_egress_terminal(port.endpoint, port.cord)
                .unwrap_or(false)
        }) {
            return Ok(());
        }
        for port in boundaries {
            if scheduler
                .remote_egress_offer(port.endpoint, port.cord)
                .map_err(|error| format!("inspect browser Mask Fore output: {error:?}"))?
                .is_some()
                && port.front_port_id.as_str() != "show"
            {
                return Err("browser Mask emitted an unexpected interaction".into());
            }
        }
        match scheduler
            .step()
            .map_err(|error| format!("drain browser Mask Play: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } | SchedulerStatus::Drained | SchedulerStatus::Idle => {
            }
            SchedulerStatus::Cancelled => return Err("browser Mask Play was cancelled".into()),
        }
    }
    Err("browser Mask Fore outputs did not become terminal".into())
}
