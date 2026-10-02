//! Bounded execution of one ordinary Mask Plot through its plan-sealed Fore.

mod prepared;
#[cfg(test)]
mod tests;
pub use prepared::PreparedNativeMaskPlay;

use alloc::{string::String, vec::Vec};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallId, PortId, RequestId,
};
use conduit_plan_lowering::lowering::{
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment, lower_plan_fragment,
};
use conduit_presentation::{PlannedMaskPlot, Presentation};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_MASK_VALUE_BYTES: usize = 4 * 1024;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 4;
const CORDS: usize = 6;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = 4;
const VALUES: usize = 10;
const VALUE_BYTES: usize = VALUES * MAX_MASK_VALUE_BYTES;
const SIGNS: usize = 96;

type Scheduler = FixedScheduler<
    MaskBack,
    FixedValueStore<VALUES, MAX_MASK_VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    HOST_BINDINGS,
    NODES,
>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeMaskPlayReceipt {
    pub mask_plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub show_value_id: String,
    pub scanout_frame_sequence: u64,
    pub scanout_pixels_written: u32,
    pub display_base_id: conduit_core::HostBaseId,
    pub surface_id: String,
    pub kernel_signs: u16,
    pub fore_endpoints: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMaskPlayError {
    Presentation,
    Plan,
    Shape,
    Kernel,
    SchedulerCreate,
    ForeAdmit,
    ForeClose,
    HostComplete,
    ForeOutput,
    Value,
    PendingRenderer,
    RendererMismatch,
    RendererFailed,
}

enum MaskBack {
    ResourceSource,
    Tee,
    Renderer { pending: bool, emitted: bool },
    Interaction { seen: u8 },
}

impl StepBack<PORTS> for MaskBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::ResourceSource => StepOutcome::Complete,
            Self::Tee => pass_one(io),
            Self::Renderer { pending, emitted } => render(pending, emitted, io),
            Self::Interaction { seen } => correlate(seen, io),
        }
    }
}

fn pass_one(io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send(PortId(0), value).is_err() {
            return failure();
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return failure();
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn render(pending: &mut bool, emitted: &mut bool, io: &mut StepIo<PORTS>) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        let Some(output) = outcome.output else {
            return failure();
        };
        if request != RequestId(0)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
            || !io.output_ready(PortId(0))
            || io.consume_host_completion().is_err()
            || io.send(PortId(0), output.value).is_err()
        {
            return failure();
        }
        *pending = false;
        *emitted = true;
        return StepOutcome::Progress;
    }
    if !*emitted && let Some(value) = io.input(PortId(0)) {
        let Ok(value) = BoundedValueRef::new(value, MAX_MASK_VALUE_BYTES as u32) else {
            return failure();
        };
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(RequestId(0), HostCallId(0), value)
                .is_err()
        {
            return failure();
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if *emitted {
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn correlate(seen: &mut u8, io: &mut StepIo<PORTS>) -> StepOutcome {
    for port in 0..2 {
        let port = PortId(port);
        if *seen & (1 << port.0) == 0 && io.input(port).is_some() {
            if io.consume(port).is_err() {
                return failure();
            }
            *seen |= 1 << port.0;
            return StepOutcome::Progress;
        }
    }
    if *seen == 0b11 {
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn failure() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail: 1,
    })
}

#[derive(Serialize)]
struct ShowValue<'a> {
    schema: &'static str,
    mask_plan_id: &'a str,
    active_play_id: &'a str,
    presentation_id: &'a str,
    presentation_revision: u64,
    show_value_id: &'a str,
}

/// An unacknowledged renderer cannot produce a Show. Call `PreparedNativeMaskPlay`
/// and complete it with compositor-owned scanout evidence instead.
pub fn run(
    _planned: &PlannedMaskPlot,
    _presentation: &Presentation,
    _play_sequence: u64,
) -> Result<NativeMaskPlayReceipt, NativeMaskPlayError> {
    Err(NativeMaskPlayError::PendingRenderer)
}

fn show_value_id(
    planned: &PlannedMaskPlot,
    presentation: &Presentation,
    active: &conduit_core::ActivePlayIdentity,
) -> String {
    let digest = Sha256::digest(
        alloc::format!(
            "native-mask-show@1\n{}\n{}\n{}\n{}\n",
            planned.plan.plan_id.as_str(),
            active.active_play_id.as_str(),
            presentation.identity.as_str(),
            presentation.revision
        )
        .as_bytes(),
    );
    let mut value = String::from("show-value/");
    for byte in digest {
        value.push_str(&alloc::format!("{byte:02x}"));
    }
    value
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Scheduler, NativeMaskPlayError> {
    if !matches!(lowered.nodes.len(), 3 | NODES) || lowered.cords.len() != CORDS {
        return Err(NativeMaskPlayError::Shape);
    }
    let mut node_specs = lowered.node_specs.clone();
    if node_specs.len() == 3 {
        node_specs.push(conduit_kernel::scheduler::NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 1,
        });
    }
    let nodes = node_specs
        .as_slice()
        .try_into()
        .map_err(|_| NativeMaskPlayError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| NativeMaskPlayError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| NativeMaskPlayError::Kernel)?;
    }
    routes.seal().map_err(|_| NativeMaskPlayError::Kernel)?;
    let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(1);
    for call in &lowered.host_calls {
        bindings
            .install(call.node, call.binding)
            .map_err(|_| NativeMaskPlayError::Kernel)?;
    }
    bindings.seal().map_err(|_| NativeMaskPlayError::Kernel)?;
    let mut drivers = fragment
        .placements
        .iter()
        .map(|placement| match placement.kind_id.as_str() {
            conduit_presentation::SHOW_RESOURCE_SOURCE_KIND => Ok(MaskBack::ResourceSource),
            conduit_presentation::PRESENTATION_TEE_KIND => Ok(MaskBack::Tee),
            conduit_presentation::RENDERER_KIND | conduit_presentation::RESOURCE_RENDERER_KIND => {
                Ok(MaskBack::Renderer {
                    pending: false,
                    emitted: false,
                })
            }
            conduit_presentation::FACE_INTERACTION_KIND => Ok(MaskBack::Interaction { seen: 0 }),
            _ => Err(NativeMaskPlayError::Shape),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if drivers.len() == 3 {
        drivers.push(MaskBack::ResourceSource);
    }
    let drivers = drivers.try_into().map_err(|_| NativeMaskPlayError::Shape)?;
    let values = FixedValueStore::<VALUES, MAX_MASK_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| NativeMaskPlayError::Value)?;
    let signs = FixedSignLog::<SIGNS>::new_with_remote_storage(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
        32,
        conduit_kernel::remote_sign_storage_bytes(32).ok_or(NativeMaskPlayError::Kernel)?,
    )
    .map_err(|_| NativeMaskPlayError::Kernel)?;
    FixedScheduler::new_with_host_calls(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(|_| NativeMaskPlayError::SchedulerCreate)
}
