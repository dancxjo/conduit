//! Sealed local pressure policy survives lowering into the production kernel.
use conduit_core::*;
use conduit_kernel::{
    scheduler::{FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, FixedRoutes, FixedSignLog, FixedValueStore, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_from_plan, FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

#[derive(Clone, Copy)]
struct Back {
    source: bool,
    committed: u8,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.source || self.committed == 3 || !io.output_ready(conduit_kernel::PortId(0)) {
            return StepOutcome::Await;
        }
        io.send_canonical(
            conduit_kernel::PortId(0),
            CanonicalValue::new(&[self.committed + 1]).unwrap(),
        )
        .unwrap();
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.source {
            self.committed += 1;
        }
    }
}

#[test]
fn sealed_local_coalescing_plan_executes_newest_replacement_after_lowering() {
    for policy in [
        DeliveryPressurePolicy::PreserveOrder,
        DeliveryPressurePolicy::CoalesceLatest,
    ] {
        let mut fragment = common::fragment();
        fragment.states.clear();
        fragment.placements[0].inputs.clear();
        fragment.placements[0].limits.max_queue_items = 2;
        fragment.placements[0].limits.max_queue_bytes = 2;
        let mut sink = fragment.placements[0].clone();
        sink.placement_id = PlacementId::from("sink");
        sink.gear_id = GearId::from("sink");
        sink.inputs = vec![PortDescriptor {
            port_id: port_id("next"),
            value_kind: kind_id("fixture/byte@1"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }];
        sink.outputs.clear();
        fragment.placements.push(sink);
        fragment.startup_order.push(PlacementId::from("sink"));
        fragment.connections.push(PlannedConnection {
            connection_id: ConnectionId::from("local"),
            source_placement_id: PlacementId::from("placement"),
            source_port_id: port_id("current"),
            sink_placement_id: PlacementId::from("sink"),
            sink_port_id: port_id("next"),
            value_kind: kind_id("fixture/byte@1"),
            resource: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: policy,
            selected_line: None,
            admitted_lines: vec![],
            item_capacity: 2,
            byte_capacity: 2,
        });
        fragment.sign_storage_budget =
            mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
        let plan = common::seal(fragment);
        assert!(verify_plan(&plan));
        let (lowered, activations) =
            lower_plan_fragment_from_plan(&plan, &plan.fragments[0].fragment_id).unwrap();
        assert!(activations.entries.is_empty());
        assert!(lowered.states.is_empty());
        assert!(lowered.host_calls.is_empty());
        assert_eq!(lowered.cords.len(), 1);
        assert_eq!(
            lowered.cords[0].spec.pressure_policy,
            if policy == DeliveryPressurePolicy::CoalesceLatest {
                conduit_kernel::scheduler::AssignedPressurePolicy::CoalesceLatest
            } else {
                conduit_kernel::scheduler::AssignedPressurePolicy::default()
            }
        );
        let mut routes = FixedRoutes::<2, 1>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .unwrap();
        }
        routes.seal().unwrap();
        // Two admitted queued values plus one bounded generated-output slot.
        let mut scheduler = FixedScheduler::<_, _, _, 2, 1, PORTS, 2, 2, 1>::new(
            lowered.node_specs.try_into().unwrap(),
            [lowered.cords[0].spec],
            routes,
            [
                Back {
                    source: true,
                    committed: 0,
                },
                Back {
                    source: false,
                    committed: 0,
                },
            ],
            FixedValueStore::<3, 1>::new(3).unwrap(),
            FixedSignLog::<64>::new(
                64 * core::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
            )
            .unwrap(),
        )
        .unwrap();
        for _ in 0..10 {
            scheduler.step().unwrap();
        }
        let coalescing = policy == DeliveryPressurePolicy::CoalesceLatest;
        assert_eq!(
            scheduler.drivers()[0].committed,
            if coalescing { 3 } else { 2 }
        );
        assert_eq!(
            scheduler.cord_usage(conduit_kernel::CordId(0)).unwrap(),
            (2, 2)
        );
        assert_eq!(scheduler.values().used_items(), 2);
        let routed = scheduler
            .signs()
            .events()
            .filter(|e| e.kind == conduit_kernel::KernelEventKind::ValueRouted)
            .count();
        assert_eq!(routed, if coalescing { 3 } else { 2 });
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.values().used_items(), 0);
    }
}
