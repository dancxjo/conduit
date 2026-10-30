//! Finite batch preparation for the same production fixed kernel.
//! Root separately owns the actual read executors, admission and Body start.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};
pub type RecordingBatchKernel<const N: usize, const C: usize> =
    FixedScheduler<RecordingBack, k::HostedValueStore, k::HostedSignLog, N, C, 2, C, 128, C, N, N>;
pub struct PreparedRecordingBatchKernel<const N: usize, const C: usize> {
    pub kernel: Box<RecordingBatchKernel<N, C>>,
    pub read_nodes: Vec<k::NodeId>,
    pub recovery_node: k::NodeId,
}
#[derive(Clone, Copy)]
enum Role {
    Generation(usize),
    Read,
    Singleton(usize),
    Concat(usize),
    Recovery,
}
impl<const N: usize, const C: usize> PreparedRecordingBatchKernel<N, C> {
    /// All objects and canonical value/Sign backing are prepared before Play.
    /// This does not admit complete system memory/work, execute a store, issue
    /// authority or correlate a real Body start. Owners must be refreshed after
    /// transfer; their historical descriptors are not continuing availability.
    pub fn prepare(
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        generations: &mut [ReadyRecordingGenerations],
        singletons: &mut [ReadyRecordingSingleton],
        concats: &mut [ReadyRecordingConcat],
        recovery: &mut ReadyRecordingRecovery,
    ) -> Result<Self, RecordingKernelRefusal> {
        use RecordingKernelRefusal as E;
        if !c::verify_plan_fragment(fragment) {
            return Err(E::Identity);
        }
        if lower_plan_fragment(fragment).map_err(|_| E::Lowering)? != *lowered {
            return Err(E::Lowering);
        }
        let count = generations.len();
        if count == 0
            || count > MAXIMUM_CATALOG_GENERATIONS
            || N != count * 4
            || C + 1 != N
            || singletons.len() != count
            || concats.len() + 1 != count
            || lowered.nodes.len() != N
            || lowered.node_specs.len() != N
            || lowered.cords.len() != C
            || lowered.host_calls.len() != count
            || lowered.resources.len() != count
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
                .any(|(i, n)| usize::from(n.node.0) != i)
            || lowered
                .node_specs
                .iter()
                .any(|n| n.maximum_step_fuel == 0 || n.input_cords[2..].iter().any(Option::is_some))
        {
            return Err(E::Shape);
        }
        if lowered.cord_value_slots > C as u16
            || lowered.cord_value_bytes > C as u32 * 4096
            || lowered.sign_items == 0
            || lowered.sign_items > 4096
            || lowered.sign_bytes == 0
            || lowered.sign_bytes > 4096 * core::mem::size_of::<k::KernelEvent>() as u32
            || lowered
                .cords
                .iter()
                .any(|cord| cord.spec.item_capacity != 1)
        {
            return Err(E::Storage);
        }
        let mut roles = [Role::Read; N];
        let mut read_nodes = Vec::with_capacity(count);
        let mut recovery_node = None;
        let mut role_counts = [0usize; 5];
        // Resolve and preflight every exact owner before consuming any of them.
        for node in &lowered.nodes {
            let gear = fragment
                .placements
                .iter()
                .find(|g| g.placement_id == node.placement_id)
                .ok_or(E::Shape)?;
            let (role, index) = match gear.kind_id.as_str() {
                "environment/recording-generation" => {
                    let index = owner_index(generations, &gear.gear_id)?;
                    generations[index]
                        .check_selected(fragment, &gear.placement_id)
                        .map_err(E::Software)?;
                    (Role::Generation(index), 0)
                }
                "flow/singleton" => {
                    let index = owner_index(singletons, &gear.gear_id)?;
                    singletons[index]
                        .check_selected(fragment, &gear.placement_id)
                        .map_err(E::Software)?;
                    (Role::Singleton(index), 1)
                }
                "flow/concat/finite" => {
                    let index = owner_index(concats, &gear.gear_id)?;
                    concats[index]
                        .check_selected(fragment, &gear.placement_id)
                        .map_err(E::Software)?;
                    (Role::Concat(index), 2)
                }
                "environment/recover-recording" => {
                    recovery
                        .check_selected(fragment, &gear.placement_id)
                        .map_err(E::Software)?;
                    if recovery_node.replace(node.node).is_some() {
                        return Err(E::Shape);
                    }
                    (Role::Recovery, 3)
                }
                conduit_semantic_catalog::SNAPSHOT_READ_KIND => {
                    let calls: Vec<_> = lowered
                        .host_calls
                        .iter()
                        .filter(|call| call.node == node.node)
                        .collect();
                    let [call] = calls.as_slice() else {
                        return Err(E::Shape);
                    };
                    if call.call != k::HostCallId(0)
                        || call.maximum_in_flight != 1
                        || call.contract_id.as_str() != "conduit.host/resource-snapshot-read@1"
                        || call.target_kind.as_ref() != Some(&gear.kind_id)
                        || call.binding.maximum_input_bytes != 512
                        || call.binding.maximum_output_bytes != 4096
                        || lowered
                            .resources
                            .iter()
                            .filter(|r| r.node == node.node)
                            .count()
                            != 1
                    {
                        return Err(E::Shape);
                    }
                    read_nodes.push(node.node);
                    (Role::Read, 4)
                }
                _ => return Err(E::Shape),
            };
            if !matches!(role, Role::Concat(_)) && !node.terminal_transductions.is_empty() {
                return Err(E::Shape);
            }
            role_counts[index] += 1;
            roles[usize::from(node.node.0)] = role;
        }
        if role_counts != [count, count, count - 1, 1, count] {
            return Err(E::Shape);
        }
        let mut routes = k::FixedRoutes::<128, C>::new(2);
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
        let mut calls = k::FixedHostCallBindings::<N>::new(1);
        for call in &lowered.host_calls {
            calls
                .install(call.node, call.binding)
                .map_err(|_| E::Tables)?;
        }
        calls.seal().map_err(|_| E::Tables)?;
        // Reserve one slot for every cord and two per in-flight read, plus one
        // staging slot. Every physical slot is large enough for a full chunk.
        let slots = u16::try_from(C + 2 * count + 1).map_err(|_| E::Storage)?;
        let values = k::HostedValueStore::new(slots, 4096, u32::from(slots) * 4096)
            .map_err(|_| E::Storage)?;
        // The portable Plan byte budget and native event representation have
        // different widths. Reserve enough native slots to cover both bounds.
        let event_bytes = core::mem::size_of::<k::KernelEvent>() as u32;
        let sign_items =
            usize::from(lowered.sign_items).max(lowered.sign_bytes.div_ceil(event_bytes) as usize);
        let signs = k::HostedSignLog::new(sign_items as u16, sign_items as u32 * event_bytes)
            .map_err(|_| E::Storage)?;
        let mut drivers: Vec<RecordingBack> = Vec::with_capacity(N);
        for (index, role) in roles.iter().enumerate() {
            let placement = &lowered.nodes[index].placement_id;
            drivers.push(match *role {
                Role::Generation(i) => RecordingBack::Source(
                    generations[i]
                        .take_selected(fragment, placement)
                        .map_err(E::Software)?,
                ),
                Role::Singleton(i) => RecordingBack::Singleton(
                    singletons[i]
                        .take_selected(fragment, placement)
                        .map_err(E::Software)?,
                ),
                Role::Concat(i) => RecordingBack::Concat(Box::new(
                    concats[i]
                        .take_selected(fragment, placement)
                        .map_err(E::Software)?,
                )),
                Role::Recovery => RecordingBack::Recovery(Box::new(
                    recovery
                        .take_selected(fragment, placement)
                        .map_err(E::Software)?,
                )),
                Role::Read => RecordingBack::Read(HostCallBack::new(512)),
            });
        }
        let drivers: [RecordingBack; N] = drivers.try_into().map_err(|_| E::Shape)?;
        let kernel = RecordingBatchKernel::new_boxed_with_active_counts_and_host_calls(
            N,
            C,
            core::array::from_fn(|i| NodeSpec {
                input_cords: [
                    lowered.node_specs[i].input_cords[0],
                    lowered.node_specs[i].input_cords[1],
                ],
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
            read_nodes,
            recovery_node: recovery_node.ok_or(E::Shape)?,
        })
    }
}
fn owner_index<B>(
    owners: &[ReadyRecordingSoftware<B>],
    gear: &c::GearId,
) -> Result<usize, RecordingKernelRefusal> {
    let mut found = owners.iter().enumerate().filter(|(_, owner)| {
        owner
            .current_selection()
            .is_some_and(|(selected, _, _)| selected == gear)
    });
    let index = found
        .next()
        .ok_or(RecordingKernelRefusal::Software(
            RecoveryInstallationRefusal::AlreadySelected,
        ))?
        .0;
    if found.next().is_some() {
        return Err(RecordingKernelRefusal::Shape);
    }
    Ok(index)
}
