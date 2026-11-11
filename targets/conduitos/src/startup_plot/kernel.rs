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
    Chime(conduit_semantic_catalog::StartupChimeBack),
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, _inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
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
            Self::Chime(back) => back.step(io, _inputs),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Wake(v) => *v = None,
            Self::Chime(back) => {
                <conduit_semantic_catalog::StartupChimeBack as StepBack<PORTS>>::cancel(back)
            }
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
            super::plan::CHIME => Back::Chime(conduit_semantic_catalog::StartupChimeBack::new()),
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
