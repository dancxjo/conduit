//! Local verified metadata to the ordinary output kernel, without an assigned
//! wire round trip, invented transfer identity or duplicate rich lowering.
use super::*;
use crate::appliance::software_compact::CompactOutputSoftware;
use conduit_plan_lowering::local_tables::VerifiedLocalLowering;
use conduit_planner::indexed_source::IndexedAuthoringBoundary;
use conduit_plot::flat_image::indexed::IndexedSourceImage;

pub struct LocalDisplayInput<'a, 'p> {
    pub verified: &'a VerifiedLocalLowering<'p>,
    /// Independently authenticated checked Make graph and matching Fore.
    pub image: &'a IndexedSourceImage<'a>,
    pub boundary: &'a IndexedAuthoringBoundary,
    /// Current Root scope, never copied from a received Plan.
    pub scope: (&'a c::HostId, &'a c::BootId, c::OfferGeneration),
    pub maximum_frames: u32,
    /// Independently linked Make ceiling over every output payload Type.
    pub maximum_value_bytes: u32,
}
impl<S: k::ValueStorage, L: k::SignSink> PreparedDisplayKernel<S, L> {
    /// Dormant preparation only. Root must separately admit execution and issue
    /// actual native possession for this exact Plan/Play before any Step.
    /// Every fallible check precedes exclusive pure-software transfer.
    pub fn prepare_local(
        input: LocalDisplayInput<'_, '_>,
        software: &mut CompactOutputSoftware<'_>,
        values: S,
        signs: L,
    ) -> Result<Self, DisplayPreparationRefusal> {
        use DisplayPreparationRefusal as E;
        let lowered = input.verified.lowered();
        let (plan, _) = input.verified.preparation_scope();
        input
            .boundary
            .check_selected_fore(plan)
            .map_err(|_| E::Shape)?;
        software
            .check_local(input.verified, input.image, input.scope)
            .map_err(E::Software)?;
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
            || plan
                .preparation_metadata()
                .fragments
                .iter()
                .any(|f| !f.execution_regions.is_empty())
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
        if input.maximum_frames == 0
            || lowered.cord_value_slots > 5
            || lowered.cord_value_bytes > 5 * 1024
            || lowered.sign_items == 0
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
            DisplaySignBudget::derive(input.maximum_frames, lowered.sign_items, lowered.sign_bytes)
                .map_err(|_| E::Bounds)?;
        let value_budget =
            DisplayValueBudget::derive(input.maximum_frames, input.maximum_value_bytes)
                .map_err(|_| E::Bounds)?;
        if values.item_capacity() < value_budget.items()
            || values.byte_capacity() < value_budget.bytes()
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
                        && f.item_capacity == 1
                        && f.selected_line.is_none()
                })
                .cloned()
                .ok_or(E::Shape)
        };
        let frame = fore("frame", c::PortDirection::Input)?;
        let refusal = fore("invalid_brightness", c::PortDirection::Output)?;
        // The lifetime storage/sign derivation is for this acyclic split, not
        // an arbitrary four-node graph with the same counts. Every frame can
        // traverse the encoder and each selector at most once.
        let local = |node| k::CordEndpoint::local(k::NodeId(node), k::PortId(0));
        let expected = [
            (k::CordEndpoint::Remote(frame.endpoint), local(0)),
            (local(0), local(1)),
            (local(0), local(2)),
            (local(1), local(3)),
            (local(2), k::CordEndpoint::Remote(refusal.endpoint)),
        ];
        if expected.iter().any(|&(source, sink)| {
            lowered
                .cords
                .iter()
                .filter(|c| c.spec.source == source && c.spec.sink == sink)
                .count()
                != 1
        }) {
            return Err(E::Shape);
        }
        let fragment = plan
            .fragment(&lowered.identity.fragment_id)
            .ok_or(E::Identity)?;
        let metadata = fragment.lowering_metadata();
        let write = lowered
            .nodes
            .iter()
            .find(|n| {
                metadata.placements.iter().any(|p| {
                    p.placement_id == n.placement_id
                        && p.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND
                })
            })
            .map(|n| n.node)
            .ok_or(E::Shape)?;
        let calls: alloc::vec::Vec<_> = lowered
            .host_calls
            .iter()
            .filter(|call| call.node == write)
            .collect();
        if write != k::NodeId(3)
            || calls.len() != 1
            || calls[0].contract_id.as_str() != conduit_tm1637_wire::WRITE_CALL
            || calls[0].call != k::HostCallId(0)
            || calls[0].maximum_in_flight != 1
            || calls[0].binding.maximum_input_bytes != 1024
            || calls[0].binding.maximum_output_bytes != 0
            || lowered.resources[0].node != write
        {
            return Err(E::Shape);
        }
        let (routes, calls) = tables(lowered)?;
        // Constructor preflight uses no callable Back. It cannot produce output
        // or touch GPIO, even if later software transfer is refused.
        type Check = FixedScheduler<
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
        Check::new_with_host_calls(
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
        let mut operations = software
            .take_local(input.verified, input.image, input.scope)
            .map_err(E::Software)?;
        let mut selectors = core::array::from_fn(|_| None);
        let drivers = core::array::from_fn(|i| match operations[i].take() {
            Some(op) => {
                selectors[i] = op.selector;
                DisplayBack::Software(op.driver)
            }
            None => DisplayBack::Write(WriteBack::new(input.maximum_frames)),
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
        .expect("preflighted immutable local output tables");
        Ok(Self {
            kernel,
            selectors,
            write,
            input: frame,
            refusal,
        })
    }
}
