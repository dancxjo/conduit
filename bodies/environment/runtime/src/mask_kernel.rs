//! Bounded realization of the ordinary read-only Face/renderer/interaction Plot.
//! This prepares shared-kernel tables, not a provider, Body membership or effect
//! authority. The selected renderer Host Call still needs an admitted service.
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredForePort};
pub type Kernel<V = k::HostedValueStore> =
    FixedScheduler<MaskBack, V, k::FixedSignLog<64, 12>, 3, 6, 2, 6, 6, 6, 3, 3>;
#[path = "mask_assigned.rs"]
mod assigned;
pub use assigned::*;
#[cfg(feature = "local-planning")]
#[path = "mask_local.rs"]
mod local;
#[cfg(feature = "local-planning")]
pub use local::LocalMaskInput;
pub struct PreparedMaskKernel<V: k::ValueStorage = k::HostedValueStore> {
    pub kernel: Box<Kernel<V>>,
    pub face: LoweredForePort,
    pub show: LoweredForePort,
    pub interaction: LoweredForePort,
}
/// Private canonical storage reserved before other heap owners fragment memory.
/// This is uninitialized value capacity, never a Plan or execution permission.
pub struct PreparedMaskValueBacking {
    values: k::HostedValueStore,
    face_bytes: u32,
    show_bytes: u32,
}
impl PreparedMaskValueBacking {
    pub fn prepare(face_bytes: u32, show_bytes: u32) -> Result<Self, &'static str> {
        if face_bytes == 0 || show_bytes == 0 || face_bytes > 8192 || show_bytes > 8192 {
            return Err("mask-values");
        }
        let values = k::HostedValueStore::new_with_slot_capacities(
            &[face_bytes, show_bytes],
            face_bytes + show_bytes,
        )
        .map_err(|_| "mask-values")?;
        Ok(Self {
            values,
            face_bytes,
            show_bytes,
        })
    }
}
/// Owned, checked dormant assembly material. It retains neither Plan nor
/// lowering scratch and cannot be constructed from received numeric tables.
/// Finishing allocates canonical value backing; it grants no execution authority.
pub struct PreparedMaskKernelParts {
    nodes: [NodeSpec<2>; 3],
    cords: [CordSpec; 6],
    routes: k::FixedRoutes<6, 6>,
    calls: k::FixedHostCallBindings<3>,
    backs: [MaskBack; 3],
    face: LoweredForePort,
    show: LoweredForePort,
    interaction: LoweredForePort,
}
impl PreparedMaskKernel {
    /// Storage uses the sealed Plan ceilings. Full canonical Face/Show values
    /// stay in the ordinary
    /// kernel store; no local handle is substituted for their semantic Types.
    pub fn prepare(planned: &f::PlannedMaskPlot) -> Result<Self, &'static str> {
        Self::prepare_parts(planned)?.finish()
    }
    /// Complete ordinary admission, retaining only owned assembly material.
    pub fn prepare_parts(
        planned: &f::PlannedMaskPlot,
    ) -> Result<PreparedMaskKernelParts, &'static str> {
        planned.validate().map_err(|_| "mask-plan")?;
        let [fragment] = planned.plan.fragments.as_slice() else {
            return Err("mask-fragments");
        };
        let lowered = lower_plan_fragment(fragment).map_err(|_| "mask-lowering")?;
        Self::prepare_lowered_parts(fragment, &lowered, |placement| {
            c::face_data_label(&placement.configuration)
                .map(|_| ())
                .map_err(|_| "mask-configuration")
        })
    }
    fn prepare_lowered_parts(
        fragment: &c::PlanFragment,
        lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
        mut configuration: impl FnMut(&c::PlannedGear) -> Result<(), &'static str>,
    ) -> Result<PreparedMaskKernelParts, &'static str> {
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
            configuration(placement)?;
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
        let cords = core::array::from_fn(|index| lowered.cords[index].spec);
        Ok(PreparedMaskKernelParts {
            nodes,
            cords,
            routes,
            calls,
            backs: backs.try_into().map_err(|_| "mask-backs")?,
            face,
            show,
            interaction,
        })
    }
}
impl PreparedMaskKernelParts {
    /// Exact canonical slot backing plus boxed scheduler. Allocator overhead,
    /// the parts wrapper, remote Sign storage and owner control reserves remain
    /// separate admission responsibilities.
    pub fn required_heap_bytes(&self) -> Result<usize, &'static str> {
        k::HostedValueStore::canonical_storage_bytes([
            self.face.byte_capacity,
            self.show.byte_capacity,
        ])
        .map_err(|_| "mask-values")?
        .checked_add(core::mem::size_of::<Kernel>())
        .ok_or("mask-values")
    }
    /// Consume checked owned parts after retiring Plan/lowering preparation.
    /// No Step, provider possession or native effect occurs here.
    pub fn finish(self) -> Result<PreparedMaskKernel, &'static str> {
        let backing =
            PreparedMaskValueBacking::prepare(self.face.byte_capacity, self.show.byte_capacity)?;
        self.finish_with_backing(backing)
    }
    /// Attach only exact, fresh canonical storage reserved by the private
    /// backing constructor. Mismatched capacity cannot narrow the checked Plan.
    pub fn finish_with_backing(
        self,
        backing: PreparedMaskValueBacking,
    ) -> Result<PreparedMaskKernel, &'static str> {
        if backing.face_bytes != self.face.byte_capacity
            || backing.show_bytes != self.show.byte_capacity
        {
            return Err("mask-values");
        }
        self.finish_with_values(backing.values)
    }
    /// Attach exclusive canonical fixed backing. This reserves the same two
    /// full 8192-byte slots in target static storage instead of fragmented heap.
    pub fn finish_with_fixed_storage(
        self,
        values: &mut k::FixedValueStore<2, 8192>,
    ) -> Result<PreparedMaskKernel<&mut k::FixedValueStore<2, 8192>>, &'static str> {
        use k::ValueStorage;
        if self.face.byte_capacity != 8192
            || self.show.byte_capacity != 8192
            || values.byte_capacity() != 16384
            || values.used_items() != 0
            || values.used_bytes() != 0
        {
            return Err("mask-values");
        }
        self.finish_with_values(values)
    }
    /// Transfer the actual exclusive fixed store only after every fallible
    /// topology, Sign budget and allocation preflight. Refusal retains both
    /// the complete checked material and the original storage reference.
    #[allow(clippy::result_large_err, clippy::type_complexity)]
    pub fn finish_with_fixed_storage_retained<'a>(
        self,
        values: &'a mut k::FixedValueStore<2, 8192>,
    ) -> Result<
        PreparedMaskKernel<&'a mut k::FixedValueStore<2, 8192>>,
        (Self, &'a mut k::FixedValueStore<2, 8192>, &'static str),
    > {
        use k::ValueStorage;
        if self.face.byte_capacity != 8192
            || self.show.byte_capacity != 8192
            || values.byte_capacity() != 16384
            || values.used_items() != 0
            || values.used_bytes() != 0
        {
            return Err((self, values, "mask-values"));
        }
        if Kernel::<&mut k::FixedValueStore<2, 8192>>::check_boxed_active_topology(
            3,
            6,
            &self.nodes,
            &self.cords,
            &self.routes,
            &self.calls,
        )
        .is_err()
        {
            return Err((self, values, "mask-kernel"));
        }
        let remote_bytes = match k::remote_sign_storage_bytes(12) {
            Some(bytes) => bytes,
            None => return Err((self, values, "mask-signs")),
        };
        let signs = match k::FixedSignLog::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            remote_bytes,
        ) {
            Ok(signs) => signs,
            Err(_) => return Err((self, values, "mask-signs")),
        };
        let layout = core::alloc::Layout::new::<Kernel<&mut k::FixedValueStore<2, 8192>>>();
        // SAFETY: reserve the exact scheduler layout before transferring the
        // sole store reference; no initialized value is read from this memory.
        let pointer = unsafe { alloc::alloc::alloc(layout) }
            .cast::<core::mem::MaybeUninit<Kernel<&mut k::FixedValueStore<2, 8192>>>>();
        if pointer.is_null() {
            return Err((self, values, "mask-kernel-capacity"));
        }
        // SAFETY: nonnull exact-layout uniquely owned allocation from above.
        let backing = unsafe { Box::from_raw(pointer) };
        let Self {
            nodes,
            cords,
            routes,
            calls,
            backs,
            face,
            show,
            interaction,
        } = self;
        // Exact tables were preflighted above and have remained exclusively
        // owned/unchanged. Supplied backing makes construction allocation-free.
        let kernel = Kernel::new_boxed_with_storage_and_active_counts_and_host_calls(
            Some(backing),
            3,
            6,
            nodes,
            cords,
            routes,
            calls,
            backs,
            values,
            signs,
        )
        .unwrap_or_else(|_| unreachable!("unchanged preflighted Mask topology"));
        Ok(PreparedMaskKernel {
            kernel,
            face,
            show,
            interaction,
        })
    }
    pub fn required_fixed_heap_bytes(&self) -> usize {
        core::mem::size_of::<Kernel<&mut k::FixedValueStore<2, 8192>>>()
    }
    // Keep fixed SignLog/scheduler construction out of callers that also plan.
    #[inline(never)]
    fn finish_with_values<V: k::ValueStorage>(
        self,
        values: V,
    ) -> Result<PreparedMaskKernel<V>, &'static str> {
        let Self {
            nodes,
            cords,
            routes,
            calls,
            backs,
            face,
            show,
            interaction,
        } = self;
        // Face fan-out shares its ValueRef, while Show occupies its own full slot.
        let signs = k::FixedSignLog::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).ok_or("mask-signs")?,
        )
        .map_err(|_| "mask-signs")?;
        let kernel = Kernel::<V>::new_boxed_with_active_counts_and_host_calls(
            3, 6, nodes, cords, routes, calls, backs, values, signs,
        )
        .map_err(|_| "mask-kernel")?;
        Ok(PreparedMaskKernel {
            kernel,
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
