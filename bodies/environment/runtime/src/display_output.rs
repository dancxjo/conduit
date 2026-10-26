//! Bounded kernel construction for the checked output Plot. Root separately
//! admits its complete memory/work, actual output capability and Body start.
use crate::{
    appliance::{
        software::{ReadySoftwareBacks, SoftwareRefusal},
        ApplianceBack,
    },
    PreparedCaptureSelector,
};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_from_plan, LoweredForePort, LoweredPlanFragment,
};
use conduit_tm1637_wire::WriteBack;
#[cfg(all(feature = "local-planning", feature = "display-output"))]
#[path = "display_output_local.rs"]
mod local;
#[cfg(all(feature = "local-planning", feature = "display-output"))]
pub use local::LocalDisplayInput;

#[path = "display_output_assigned.rs"]
mod assigned;
pub use assigned::{
    AssignedDisplayInput, AssignedDisplayProfile, AssignedDisplayRefusal, AssignedDisplayScope,
    OutputSoftware, PreparedAssignedDisplayKernel,
};
#[path = "display_output_validation.rs"]
mod validation;
pub use validation::ValidatedAssignedDisplay;
#[path = "display_signs.rs"]
mod signs;
pub use signs::{DisplaySignBudget, SignBudgetRefusal};
#[path = "display_values.rs"]
mod values;
pub use values::DisplayValueBudget;
pub enum DisplayBack {
    Software(ApplianceBack),
    Write(WriteBack),
}
impl<const P: usize> StepBack<P> for DisplayBack {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Software(b) => b.step(io, input),
            Self::Write(b) => b.step(io, input),
        }
    }
    fn prepared_output(&self, port: k::PortId) -> Option<&[u8]> {
        match self {
            Self::Software(b) => <ApplianceBack as StepBack<P>>::prepared_output(b, port),
            Self::Write(_) => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Software(b) => <ApplianceBack as StepBack<P>>::step_committed(b),
            Self::Write(b) => <WriteBack as StepBack<P>>::step_committed(b),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Software(b) => <ApplianceBack as StepBack<P>>::cancel(b),
            Self::Write(b) => <WriteBack as StepBack<P>>::cancel(b),
        }
    }
}
pub type DisplayKernel<S = k::FixedValueStore<12, 1024>, E = k::FixedSignLog<256>> =
    FixedScheduler<DisplayBack, S, E, 4, 5, 1, 5, 5, 5, 4, 4>;
#[derive(Debug, PartialEq, Eq)]
pub enum DisplayPreparationRefusal {
    Identity,
    Lowering,
    Shape,
    Bounds,
    Tables,
    Software(SoftwareRefusal),
}
pub struct PreparedDisplayKernel<
    S: k::ValueStorage = k::FixedValueStore<12, 1024>,
    E: k::SignSink = k::FixedSignLog<256>,
> {
    pub kernel: Box<DisplayKernel<S, E>>,
    pub selectors: [Option<PreparedCaptureSelector>; 4],
    pub write: k::NodeId,
    pub input: LoweredForePort,
    pub refusal: LoweredForePort,
}
impl PreparedDisplayKernel {
    /// No Step or GPIO effect occurs here. The caller must retain a separately
    /// authorized output owner for the exact Plan/Play and service only the
    /// kernel's original Host Call requests. Fore access is local ownership,
    /// not an admitted network Line or proof of Body membership.
    pub fn prepare(
        plan: &c::Plan,
        lowered: &LoweredPlanFragment,
        software: &mut ReadySoftwareBacks,
        maximum_frames: u32,
    ) -> Result<Self, DisplayPreparationRefusal> {
        let values = k::FixedValueStore::<12, 1024>::new(12 * 1024)
            .map_err(|_| DisplayPreparationRefusal::Bounds)?;
        let signs = k::FixedSignLog::<256>::new_with_remote_storage(
            256 * core::mem::size_of::<k::KernelEvent>() as u32,
            256,
            k::remote_sign_storage_bytes(256).ok_or(DisplayPreparationRefusal::Bounds)?,
        )
        .map_err(|_| DisplayPreparationRefusal::Bounds)?;
        Self::prepare_with_storage(plan, lowered, software, maximum_frames, values, signs)
    }
}
impl<S: k::ValueStorage, L: k::SignSink> PreparedDisplayKernel<S, L> {
    /// Root admits both stores and construction headroom before calling this.
    /// Storage must be empty and cover this profile's complete finite bounds.
    /// All checks precede software transfer; no Step or native call runs here.
    pub fn prepare_with_storage(
        plan: &c::Plan,
        lowered: &LoweredPlanFragment,
        software: &mut ReadySoftwareBacks,
        maximum_frames: u32,
        values: S,
        signs: L,
    ) -> Result<Self, DisplayPreparationRefusal> {
        use DisplayPreparationRefusal as E;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::Shape);
        };
        if !c::verify_plan(plan) {
            return Err(E::Identity);
        }
        let (expected, activations) =
            lower_plan_fragment_from_plan(plan, &fragment.fragment_id).map_err(|_| E::Lowering)?;
        if expected != *lowered {
            return Err(E::Lowering);
        }
        if !activations.entries.is_empty() {
            return Err(E::Shape);
        }
        if lowered.nodes.len() != 4
            || lowered.node_specs.len() != 4
            || lowered.cords.len() != 5
            || lowered.fore_ports.len() != 2
            || lowered.host_calls.len() != 3
            || lowered.resources.len() != 1
            || !lowered.remote_endpoints.is_empty()
            || !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
            || !lowered.resource_cords.is_empty()
            || !lowered.shared_pools.is_empty()
            || !fragment.execution_regions.is_empty()
            || lowered
                .nodes
                .iter()
                .enumerate()
                .any(|(i, n)| usize::from(n.node.0) != i || !n.terminal_transductions.is_empty())
            || lowered
                .node_specs
                .iter()
                .any(|n| n.input_cords[1..].iter().any(Option::is_some))
        {
            return Err(E::Shape);
        }
        if maximum_frames == 0
            || lowered.cord_value_slots > 5
            || lowered.cord_value_bytes > 5 * 1024
            || lowered.sign_items > 256
            || lowered.sign_bytes > 256 * core::mem::size_of::<k::KernelEvent>() as u32
            || lowered
                .cords
                .iter()
                .any(|c| c.spec.item_capacity != 1 || c.spec.byte_capacity > 1024)
        {
            return Err(E::Bounds);
        }
        let sign_budget =
            DisplaySignBudget::derive(maximum_frames, lowered.sign_items, lowered.sign_bytes)
                .map_err(|_| E::Bounds)?;
        if values.item_capacity() < 12
            || values.byte_capacity() < 12 * 1024
            || values.used_items() != 0
            || values.used_bytes() != 0
            || signs.item_capacity() < sign_budget.ordinary_items()
            || signs.byte_capacity() < sign_budget.ordinary_bytes()
            || !signs.is_empty()
            || signs.used_bytes() != 0
            || signs
                .ensure_remote_capacity(sign_budget.remote_items())
                .is_err()
        {
            return Err(E::Bounds);
        }
        let fore = |name: &str, direction| {
            lowered
                .fore_ports
                .iter()
                .find(|f| f.front_port_id.as_str() == name && f.direction == direction)
                .filter(|f| {
                    f.temporal == c::PortTemporal::Flow { closes: true }
                        && f.track == c::ConnectionTrack::Payload
                        && f.selected_line.is_none()
                })
                .cloned()
                .ok_or(E::Shape)
        };
        let input = fore("frame", c::PortDirection::Input)?;
        let refusal = fore("invalid_brightness", c::PortDirection::Output)?;
        let mut write = None;
        for node in &lowered.nodes {
            let gear = fragment
                .placements
                .iter()
                .find(|p| p.placement_id == node.placement_id)
                .ok_or(E::Shape)?;
            let calls: alloc::vec::Vec<_> = lowered
                .host_calls
                .iter()
                .filter(|call| call.node == node.node)
                .collect();
            if gear.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND {
                if write.replace(node.node).is_some()
                    || calls.len() != 1
                    || calls[0].contract_id.as_str() != conduit_tm1637_wire::WRITE_CALL
                    || calls[0].call != k::HostCallId(0)
                    || calls[0].maximum_in_flight != 1
                    || calls[0].binding.maximum_input_bytes != 1024
                    || calls[0].binding.maximum_output_bytes != 0
                    || lowered.resources[0].node != node.node
                {
                    return Err(E::Shape);
                }
            } else {
                software
                    .check_selected(fragment, &node.placement_id)
                    .map_err(E::Software)?;
                if gear
                    .configuration
                    .iter()
                    .any(|entry| entry.key == "selector")
                {
                    if calls.len() != 1
                        || calls[0].contract_id.as_str() != "conduit.host/structured-selector@1"
                        || calls[0].call != k::HostCallId(0)
                        || calls[0].maximum_in_flight != 1
                        || calls[0].binding.maximum_input_bytes != 1024
                        || calls[0].binding.maximum_output_bytes != 1024
                    {
                        return Err(E::Shape);
                    }
                } else if !calls.is_empty()
                    || !gear
                        .configuration
                        .iter()
                        .any(|entry| entry.key == "program")
                {
                    return Err(E::Shape);
                }
            }
        }
        let write = write.ok_or(E::Shape)?;
        // Ask the same kernel constructor to validate the numeric tables before
        // consuming a software owner. The unpolled shell has no callable backs
        // or effects; its one-byte store is never used for execution.
        let (routes, calls) = tables(lowered)?;
        type TableCheck = FixedScheduler<
            NoStep,
            k::FixedValueStore<1, 1>,
            k::FixedSignLog<1>,
            4,
            5,
            1,
            5,
            5,
            5,
            4,
            4,
        >;
        TableCheck::new_with_host_calls(
            core::array::from_fn(|i| NodeSpec {
                input_cords: [lowered.node_specs[i].input_cords[0]],
                maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel,
            }),
            core::array::from_fn(|i| lowered.cords[i].spec),
            routes,
            calls,
            core::array::from_fn(|_| NoStep),
            k::FixedValueStore::new(1).map_err(|_| E::Bounds)?,
            k::FixedSignLog::new(core::mem::size_of::<k::KernelEvent>() as u32)
                .map_err(|_| E::Bounds)?,
        )
        .map_err(|_| E::Tables)?;
        let (routes, calls) = tables(lowered)?;
        let mut selectors = core::array::from_fn(|_| None);
        // Tables and every software selection are checked before the first
        // transfer. Exclusive mutable custody prevents a competing selection.
        let drivers = core::array::from_fn(|i| {
            if k::NodeId(i as u16) == write {
                DisplayBack::Write(WriteBack::new(maximum_frames))
            } else {
                let operation = software
                    .take_selected(fragment, &lowered.nodes[i].placement_id)
                    .expect("preflighted exclusive software owner");
                selectors[i] = operation.selector;
                DisplayBack::Software(operation.driver)
            }
        });
        let kernel = DisplayKernel::new_boxed_with_active_counts_and_host_calls(
            4,
            5,
            core::array::from_fn(|i| NodeSpec {
                input_cords: [lowered.node_specs[i].input_cords[0]],
                maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel,
            }),
            core::array::from_fn(|i| lowered.cords[i].spec),
            routes,
            calls,
            drivers,
            values,
            signs,
        )
        .map_err(|_| E::Tables)?;
        Ok(Self {
            kernel,
            selectors,
            write,
            input,
            refusal,
        })
    }
}

struct NoStep;
impl StepBack<1> for NoStep {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        unreachable!("numeric table preflight is never stepped")
    }
}
type Tables = (k::FixedRoutes<5, 5>, k::FixedHostCallBindings<4>);
fn tables(lowered: &LoweredPlanFragment) -> Result<Tables, DisplayPreparationRefusal> {
    use DisplayPreparationRefusal as E;
    let mut routes = k::FixedRoutes::<5, 5>::new(1);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| E::Tables)?;
    }
    routes.seal().map_err(|_| E::Tables)?;
    let mut calls = k::FixedHostCallBindings::<4>::new(1);
    for call in &lowered.host_calls {
        calls
            .install(call.node, call.binding)
            .map_err(|_| E::Tables)?;
    }
    calls.seal().map_err(|_| E::Tables)?;
    Ok((routes, calls))
}
