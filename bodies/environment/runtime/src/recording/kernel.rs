//! Finite single-generation recovery preparation. No alternate scheduler,
//! storage executor, Host identity, authority or Body start is created here.
use super::*;
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

pub type RecordingKernel = FixedScheduler<
    RecordingBack,
    k::FixedValueStore<4, 4096>,
    k::FixedSignLog<128>,
    4,
    3,
    1,
    3,
    4,
    3,
    4,
    4,
>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordingRoles {
    pub generation: k::NodeId,
    pub read: k::NodeId,
    pub singleton: k::NodeId,
    pub recovery: k::NodeId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecordingKernelRefusal {
    Identity,
    Lowering,
    Shape,
    Storage,
    Tables,
    Software(RecoveryInstallationRefusal),
}
pub struct PreparedRecordingKernel {
    pub kernel: RecordingKernel,
    pub roles: RecordingRoles,
}
impl PreparedRecordingKernel {
    /// Root retains the selected snapshot executor and independently admits this
    /// complete memory/work closure before a real Body start. A generic Host
    /// Call driver is not a storage provider or proof of durable residence.
    pub fn prepare(
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        generations: &mut ReadyRecordingGenerations,
        singleton: &mut ReadyRecordingSingleton,
        recovery: &mut ReadyRecordingRecovery,
    ) -> Result<Self, RecordingKernelRefusal> {
        use RecordingKernelRefusal as E;
        if !c::verify_plan_fragment(fragment) {
            return Err(E::Identity);
        }
        if lower_plan_fragment(fragment).map_err(|_| E::Lowering)? != *lowered {
            return Err(E::Lowering);
        }
        if lowered.nodes.len() != 4
            || lowered.node_specs.len() != 4
            || lowered.cords.len() != 3
            || lowered.host_calls.len() != 1
            || lowered.resources.len() != 1
            || !lowered.remote_endpoints.is_empty()
            || !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
            || !lowered.fore_ports.is_empty()
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
        if lowered.cord_value_slots > 4
            || lowered.cord_value_bytes > 4 * 4096
            || lowered.sign_items > 128
            || lowered.sign_bytes > (128 * core::mem::size_of::<k::KernelEvent>()) as u32
        {
            return Err(E::Storage);
        }
        let find = |kind: &str| {
            let mut gears = fragment
                .placements
                .iter()
                .filter(|g| g.kind_id.as_str() == kind);
            let placement = &gears.next().ok_or(E::Shape)?.placement_id;
            if gears.next().is_some() {
                return Err(E::Shape);
            }
            let node = lowered
                .identity
                .node_for_placement(placement)
                .ok_or(E::Shape)?;
            Ok((node, placement))
        };
        let (generation, generation_placement) = find("environment/recording-generation")?;
        let (read, _) = find(conduit_semantic_catalog::SNAPSHOT_READ_KIND)?;
        let (singleton_node, singleton_placement) = find("flow/singleton")?;
        let (recovery_node, recovery_placement) = find("environment/recover-recording")?;
        if lowered.resources[0].node != read {
            return Err(E::Shape);
        }
        let call = &lowered.host_calls[0];
        if call.node != read
            || call.call != k::HostCallId(0)
            || call.maximum_in_flight != 1
            || call.contract_id.as_str() != "conduit.host/resource-snapshot-read@1"
            || call.target_kind.as_ref().map(|k| k.as_str())
                != Some(conduit_semantic_catalog::SNAPSHOT_READ_KIND)
            || call.binding.maximum_input_bytes != 512
            || call.binding.maximum_output_bytes != 4096
        {
            return Err(E::Shape);
        }
        generations
            .check_selected(fragment, generation_placement)
            .map_err(E::Software)?;
        singleton
            .check_selected(fragment, singleton_placement)
            .map_err(E::Software)?;
        recovery
            .check_selected(fragment, recovery_placement)
            .map_err(E::Software)?;
        let mut routes = k::FixedRoutes::<4, 3>::new(1);
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
        calls
            .install(call.node, call.binding)
            .map_err(|_| E::Tables)?;
        calls.seal().map_err(|_| E::Tables)?;
        let values = k::FixedValueStore::<4, 4096>::new(4 * 4096).map_err(|_| E::Storage)?;
        let signs =
            k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32)
                .map_err(|_| E::Storage)?;
        // All owner selections and table bounds are checked before any transfer.
        let mut source = Some(
            generations
                .take_selected(fragment, generation_placement)
                .map_err(E::Software)?,
        );
        let mut bridge = Some(
            singleton
                .take_selected(fragment, singleton_placement)
                .map_err(E::Software)?,
        );
        let mut history = Some(Box::new(
            recovery
                .take_selected(fragment, recovery_placement)
                .map_err(E::Software)?,
        ));
        let drivers = core::array::from_fn(|i| {
            if i == usize::from(generation.0) {
                RecordingBack::Source(source.take().expect("unique generation"))
            } else if i == usize::from(singleton_node.0) {
                RecordingBack::Singleton(bridge.take().expect("unique singleton"))
            } else if i == usize::from(recovery_node.0) {
                RecordingBack::Recovery(history.take().expect("unique recovery"))
            } else {
                RecordingBack::Read(HostCallBack::new(512))
            }
        });
        let kernel = RecordingKernel::new_with_host_calls(
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
            roles: RecordingRoles {
                generation,
                read,
                singleton: singleton_node,
                recovery: recovery_node,
            },
        })
    }
}
