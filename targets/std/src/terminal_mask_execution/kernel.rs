//! Finite storage comes from the exact lowered Plan, allocated before execution.
use super::*;
use conduit_kernel::scheduler::FixedScheduler;
use conduit_kernel::{FixedHostCallBindings, FixedRoutes, HostedSignLog, HostedValueStore};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use conduit_presentation::{FACE_INTERACTION_KIND, PRESENTATION_TEE_KIND, RENDERER_KIND};
pub(super) type Scheduler = FixedScheduler<
    backs::MaskBack,
    HostedValueStore,
    HostedSignLog,
    3,
    6,
    PORTS,
    6,
    { 3 * PORTS },
    6,
    2,
    3,
>;

pub(super) fn prepare(
    fragment: &conduit_core::PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Scheduler, TerminalError> {
    let read_only = fragment.placements.iter().any(|placement| {
        placement.implementation_id.as_str() == planning::READ_ONLY_INTERACTION_IMPLEMENTATION
    });
    if fragment.placements.len() != 3
        || lowered.cords.len() != 6
        || lowered.host_calls.len() != if read_only { 1 } else { 2 }
        || lowered.cord_value_slots != 6
    {
        return Err(error("unexpected terminal Mask Plan shape"));
    }
    let mut routes = FixedRoutes::<{ 3 * PORTS }, 6>::new(PORTS as u16);
    for r in &lowered.routes {
        routes
            .install(r.source_node, r.source_port, r.range, &r.targets)
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    let mut bytes = lowered.cord_value_bytes;
    for call in &lowered.host_calls {
        if call.maximum_in_flight != 1
            || call.binding.maximum_input_bytes > MAX_TERMINAL_VALUE_BYTES
        {
            return Err(error("terminal Host Call exceeds admitted bound"));
        }
        let maximum = match call.contract_id.as_str() {
            PRESENT_CALL => MAX_TERMINAL_VALUE_BYTES,
            INTERACTION_CALL => conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32,
            _ => return Err(error("unknown terminal Host Call")),
        };
        if call.binding.maximum_output_bytes > maximum {
            return Err(error("terminal Host Call output bound"));
        }
        bytes = bytes
            .checked_add(call.binding.maximum_output_bytes)
            .ok_or_else(|| error("terminal storage overflow"))?;
        bindings
            .install(call.node, call.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let drivers = fragment
        .placements
        .iter()
        .map(|p| match p.kind_id.as_str() {
            PRESENTATION_TEE_KIND => Ok(backs::MaskBack::Tee),
            RENDERER_KIND => Ok(backs::MaskBack::Renderer {
                pending: false,
                emitted: false,
            }),
            FACE_INTERACTION_KIND => {
                if p.implementation_id.as_str() == planning::READ_ONLY_INTERACTION_IMPLEMENTATION {
                    Ok(backs::MaskBack::InteractionClose { seen_face: false })
                } else {
                    Ok(backs::MaskBack::Interaction {
                        seen_face: false,
                        pending: false,
                    })
                }
            }
            _ => Err(error("unknown terminal back")),
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| error("terminal back count"))?;
    // Six admitted one-item queues plus at most two Host Call result values.
    let values = HostedValueStore::new(8, MAX_TERMINAL_VALUE_BYTES, bytes).map_err(debug_error)?;
    let signs = HostedSignLog::new_with_remote_storage(
        96,
        (96 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        32,
        conduit_kernel::remote_sign_storage_bytes(32)
            .ok_or_else(|| error("terminal sign bound"))?,
    )
    .map_err(debug_error)?;
    Scheduler::new_with_host_calls(
        lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| error("terminal node count"))?,
        lowered
            .cords
            .iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| error("terminal cord count"))?,
        routes,
        bindings,
        drivers,
        values,
        signs,
    )
    .map_err(debug_error)
}
