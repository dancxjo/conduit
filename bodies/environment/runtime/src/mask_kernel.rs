//! Bounded realization of the ordinary read-only Face/renderer/interaction Plot.
//! This prepares shared-kernel tables, not a provider, Body membership or effect
//! authority. The selected renderer Host Call still needs an admitted service.
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredForePort};
pub type Kernel =
    FixedScheduler<MaskBack, k::HostedValueStore, k::FixedSignLog<64>, 3, 6, 2, 6, 6, 6, 3, 3>;
pub struct PreparedMaskKernel {
    pub kernel: Box<Kernel>,
    pub face: LoweredForePort,
    pub show: LoweredForePort,
    pub interaction: LoweredForePort,
}
impl PreparedMaskKernel {
    /// Storage uses the sealed Plan ceilings. Full canonical Face/Show values
    /// stay in the ordinary
    /// kernel store; no local handle is substituted for their semantic Types.
    pub fn prepare(planned: &f::PlannedMaskPlot) -> Result<Self, &'static str> {
        f::PlannedMaskPlot::admit(&planned.mask, &planned.plan).map_err(|_| "mask-plan")?;
        let [fragment] = planned.plan.fragments.as_slice() else {
            return Err("mask-fragments");
        };
        let lowered = lower_plan_fragment(fragment).map_err(|_| "mask-lowering")?;
        if fragment.placements.len() != 3
            || lowered.cords.len() != 6
            || lowered.host_calls.len() != 1
            || lowered.routes.iter().any(|r| r.source_port.0 >= 2)
        {
            return Err("mask-shape");
        }
        let fore = |name: &str| {
            lowered
                .fore_ports
                .iter()
                .find(|p| {
                    p.front_port_id.as_str() == name && p.track == c::ConnectionTrack::Payload
                })
                .cloned()
                .ok_or("mask-fore")
        };
        let face = fore("face")?;
        let show = fore("show")?;
        let interaction = fore("interaction")?;
        if face.byte_capacity == 0
            || show.byte_capacity == 0
            || face.byte_capacity > 8192
            || show.byte_capacity > 8192
            || lowered
                .cords
                .iter()
                .any(|cord| cord.spec.item_capacity != 1)
        {
            return Err("mask-bound");
        }
        let mut backs = Vec::with_capacity(3);
        let mut roles = 0;
        for placement in &fragment.placements {
            c::face_data_label(&placement.configuration).map_err(|_| "mask-configuration")?;
            let role = match placement.kind_id.as_str() {
                f::FACE_TEE_KIND => 1,
                f::RENDERER_KIND => 2,
                f::FACE_INTERACTION_KIND => 4,
                _ => return Err("mask-back"),
            };
            if roles & role != 0 {
                return Err("mask-back");
            }
            roles |= role;
            let (inputs, outputs, calls) = match role {
                1 => (f::face_tee_inputs(), f::face_tee_outputs(), 0),
                2 => (f::renderer_inputs(), f::renderer_outputs(), 1),
                _ => (f::interaction_inputs(), f::interaction_outputs(), 0),
            };
            if placement.inputs != inputs
                || placement.outputs != outputs
                || placement.host_calls.len() != calls
            {
                return Err("mask-back-contract");
            }
            let back = match placement.kind_id.as_str() {
                f::FACE_TEE_KIND => MaskBack::Tee,
                f::RENDERER_KIND => MaskBack::Renderer {
                    state: 0,
                    next_request: 0,
                    maximum_input_bytes: face.byte_capacity,
                },
                f::FACE_INTERACTION_KIND if placement.host_calls.is_empty() => {
                    MaskBack::ReadOnly { face: false }
                }
                _ => return Err("mask-back"),
            };
            backs.push(back);
        }
        if roles != 7
            || lowered
                .nodes
                .iter()
                .any(|n| n.inputs.len() > 2 || n.outputs.len() > 2)
        {
            return Err("mask-shape");
        }
        let nodes = core::array::from_fn(|i| NodeSpec {
            input_cords: core::array::from_fn(|p| lowered.node_specs[i].input_cords[p]),
            maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel,
        });
        let mut routes = k::FixedRoutes::<6, 6>::new(2);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|_| "mask-routes")?;
        }
        routes.seal().map_err(|_| "mask-routes")?;
        let mut calls = k::FixedHostCallBindings::<3>::new(1);
        let call = &lowered.host_calls[0];
        if call.maximum_in_flight != 1
            || call.binding.maximum_input_bytes > face.byte_capacity
            || call.binding.maximum_output_bytes > show.byte_capacity
        {
            return Err("mask-call-bound");
        }
        calls
            .install(call.node, call.binding)
            .map_err(|_| "mask-call")?;
        calls.seal().map_err(|_| "mask-call")?;
        // Face fan-out shares its ValueRef. A pending call retains that Face
        // until completion; the result needs one separate Show slot. Egress and
        // interaction share the same Show. No next Face enters this one-shot Play.
        let values = k::HostedValueStore::new_with_slot_capacities(
            &[face.byte_capacity, show.byte_capacity],
            face.byte_capacity + show.byte_capacity,
        )
        .map_err(|_| "mask-values")?;
        let signs = k::FixedSignLog::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).ok_or("mask-signs")?,
        )
        .map_err(|_| "mask-signs")?;
        let kernel = Kernel::new_with_host_calls(
            nodes,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "mask-cords")?,
            routes,
            calls,
            backs.try_into().map_err(|_| "mask-backs")?,
            values,
            signs,
        )
        .map_err(|_| "mask-kernel")?;
        Ok(Self {
            kernel: Box::new(kernel),
            face,
            show,
            interaction,
        })
    }
}
/// Workload-specific Backs. Routing, fan-out retention, pressure, Host Calls and
/// terminal propagation are exclusively performed by the shared scheduler.
pub enum MaskBack {
    Tee,
    Renderer {
        state: u8,
        maximum_input_bytes: u32,
        next_request: u32,
    },
    ReadOnly {
        face: bool,
    },
}
impl StepBack<2> for MaskBack {
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Tee => {
                if let Some(value) = io.input(k::PortId(0)) {
                    if !io.output_ready(k::PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.consume(k::PortId(0)).is_err() || io.send(k::PortId(0), value).is_err() {
                        return failed();
                    }
                    StepOutcome::Progress
                } else if io.input_closed(k::PortId(0)) {
                    if io.consume_closed(k::PortId(0)).is_err() {
                        return failed();
                    }
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::ReadOnly { face } => {
                if !*face && io.input(k::PortId(0)).is_some() {
                    if io.consume(k::PortId(0)).is_err() {
                        return failed();
                    }
                    *face = true;
                    StepOutcome::Progress
                } else if *face && io.input(k::PortId(1)).is_some() {
                    if io.consume(k::PortId(1)).is_err() {
                        return failed();
                    }
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Renderer {
                state,
                maximum_input_bytes,
                next_request,
            } => match *state {
                0 => {
                    let Some(value) = io.input(k::PortId(0)) else {
                        return StepOutcome::Await;
                    };
                    let Ok(input) = k::BoundedValueRef::new(value, *maximum_input_bytes) else {
                        return failed();
                    };
                    if io.consume(k::PortId(0)).is_err()
                        || io
                            .request_host_call(k::RequestId(*next_request), k::HostCallId(0), input)
                            .is_err()
                    {
                        return failed();
                    }
                    *state = 1;
                    StepOutcome::Progress
                }
                1 => {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(output) = outcome.output else {
                        return failed();
                    };
                    if !io.output_ready(k::PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if request != k::RequestId(*next_request)
                        || outcome.disposition != k::HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                        || io.send(k::PortId(0), output.value).is_err()
                    {
                        return failed();
                    }
                    let Some(next) = next_request.checked_add(1) else {
                        return failed();
                    };
                    *next_request = next;
                    *state = 2;
                    StepOutcome::Progress
                }
                _ => StepOutcome::Complete,
            },
        }
    }
    fn cancel(&mut self) {}
}
fn failed() -> StepOutcome {
    StepOutcome::Fail(k::Failure {
        code: k::FailureCode::InvalidLifecycle,
        detail: 60,
    })
}

// Only the pure routing/closure state is rearmed. Renderer request generations
// survive. The session owner separately requires drained queues, accepted output,
// transferred evidence, and unchanged exact Plan/provider custody.
impl prepared_session::ReusableStepBack<2> for MaskBack {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn k::ValueStorage) -> bool {
        match self {
            Self::Tee => true,
            Self::Renderer {
                state,
                next_request,
                ..
            } => *state == 2 && *next_request < u32::MAX,
            Self::ReadOnly { face } => *face,
        }
    }
    fn begin_next(&mut self, _: ()) {
        match self {
            Self::Tee => (),
            Self::Renderer { state, .. } => *state = 0,
            Self::ReadOnly { face } => *face = false,
        }
    }
}
