//! Bounded execution of one ordinary Mask Plot through its plan-sealed Fore.

#[cfg(any(test, feature = "native-compositor"))]
mod backs;
#[cfg(any(test, feature = "native-compositor"))]
mod interaction;
#[cfg(any(test, feature = "native-compositor"))]
mod prepared;
#[cfg(test)]
mod tests;
#[cfg(any(test, feature = "native-compositor"))]
use backs::MaskBack;
#[cfg(any(test, feature = "native-compositor"))]
pub use interaction::NativeMaskInteractionSession;
#[cfg(any(test, feature = "native-compositor"))]
pub use prepared::PreparedNativeMaskPlay;

#[cfg(any(test, feature = "native-compositor"))]
use alloc::{string::String, vec::Vec};
#[cfg(any(test, feature = "native-compositor"))]
use conduit_kernel::scheduler::{CordSpec, FixedScheduler};
#[cfg(any(test, feature = "native-compositor"))]
use conduit_kernel::{FixedHostCallBindings, FixedRoutes, FixedSignLog, HostedValueStore};
#[cfg(any(test, feature = "native-compositor"))]
use conduit_plan_lowering::lowering::lower_plan_fragment;
#[cfg(any(test, feature = "native-compositor"))]
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};
#[cfg(any(test, feature = "native-compositor"))]
use conduit_presentation::{PlannedMaskPlot, Presentation};
#[cfg(any(test, feature = "native-compositor"))]
use serde::Serialize;
#[cfg(any(test, feature = "native-compositor"))]
use sha2::{Digest, Sha256};

// The current complete resident Patchbay Face is about 21 KiB encoded. This
// finite native profile admits it whole; larger Faces require a new admission.
pub const MAX_MASK_VALUE_BYTES: usize = 32 * 1024;
#[cfg(any(test, feature = "native-compositor"))]
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
#[cfg(any(test, feature = "native-compositor"))]
const NODES: usize = 4;
#[cfg(any(test, feature = "native-compositor"))]
const CORDS: usize = 6;
#[cfg(any(test, feature = "native-compositor"))]
const ROUTES: usize = NODES * PORTS;
#[cfg(any(test, feature = "native-compositor"))]
const HOST_BINDINGS: usize = 4;
// One value per fixed Cord queue slot plus one result per pending Host Call.
#[cfg(any(test, feature = "native-compositor"))]
const VALUES: usize = CORDS + NODES;
#[cfg(any(test, feature = "native-compositor"))]
const INTERACTION_VALUE_BYTES: usize = conduit_presentation::MAX_FACE_INTERACTION_BYTES;
#[cfg(any(test, feature = "native-compositor"))]
const STORAGE_VALUE_BYTES: usize = MAX_MASK_VALUE_BYTES;
// Five Face/Show queues, one interaction queue, and the two Host Call results.
#[cfg(any(test, feature = "native-compositor"))]
const VALUE_BYTES: usize = 6 * MAX_MASK_VALUE_BYTES + 2 * INTERACTION_VALUE_BYTES;
#[cfg(any(test, feature = "native-compositor"))]
const SIGNS: usize = 96;

#[cfg(any(test, feature = "native-compositor"))]
type Scheduler = FixedScheduler<
    MaskBack,
    HostedValueStore,
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

#[cfg(any(test, feature = "native-compositor"))]
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

#[cfg(any(test, feature = "native-compositor"))]
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
    Interaction(conduit_presentation::FaceInteractionRefusal),
    Pressure,
    Cancelled,
}

#[cfg(any(test, feature = "native-compositor"))]
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
#[cfg(test)]
pub fn run(
    _planned: &PlannedMaskPlot,
    _presentation: &Presentation,
    _play_sequence: u64,
) -> Result<NativeMaskPlayReceipt, NativeMaskPlayError> {
    Err(NativeMaskPlayError::PendingRenderer)
}

#[cfg(any(test, feature = "native-compositor"))]
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

#[cfg(any(test, feature = "native-compositor"))]
fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Scheduler, NativeMaskPlayError> {
    if !matches!(lowered.nodes.len(), 3 | NODES) || lowered.cords.len() != CORDS {
        return Err(NativeMaskPlayError::Shape);
    }
    let value_bytes = admitted_value_bytes(lowered)?;
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
            conduit_presentation::FACE_INTERACTION_KIND => Ok(MaskBack::Interaction {
                seen_face: false,
                pending: false,
            }),
            _ => Err(NativeMaskPlayError::Shape),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if drivers.len() == 3 {
        drivers.push(MaskBack::ResourceSource);
    }
    let drivers = drivers.try_into().map_err(|_| NativeMaskPlayError::Shape)?;
    let values = HostedValueStore::new(VALUES as u16, STORAGE_VALUE_BYTES as u32, value_bytes)
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

/// The preallocated store reserves 10 finite value slots at preparation; its live
/// byte budget follows the admitted Cord queues plus the two bounded Host Call
/// results. Native rendering narrows its larger semantic offer to 16 KiB;
/// interaction retains its declared 8192-byte bound. No budget grows during Play.
#[cfg(any(test, feature = "native-compositor"))]
fn admitted_value_bytes(lowered: &LoweredPlanFragment) -> Result<u32, NativeMaskPlayError> {
    if usize::from(lowered.cord_value_slots) > CORDS || lowered.host_calls.len() != 2 {
        return Err(NativeMaskPlayError::Shape);
    }
    let mut bytes = lowered.cord_value_bytes;
    for call in &lowered.host_calls {
        if call.maximum_in_flight != 1 {
            return Err(NativeMaskPlayError::Shape);
        }
        let bound = match call.contract_id.as_str() {
            "conduit.host/present@1" => MAX_MASK_VALUE_BYTES,
            "conduit.host/presentation-interaction@1" => INTERACTION_VALUE_BYTES,
            _ => return Err(NativeMaskPlayError::Shape),
        } as u32;
        if call.binding.maximum_output_bytes > bound {
            return Err(NativeMaskPlayError::Pressure);
        }
        bytes = bytes
            .checked_add(call.binding.maximum_output_bytes)
            .ok_or(NativeMaskPlayError::Pressure)?;
    }
    if bytes as usize > VALUE_BYTES {
        return Err(NativeMaskPlayError::Pressure);
    }
    Ok(bytes)
}
