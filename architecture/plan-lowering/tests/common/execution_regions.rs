use conduit_core::*;
#[path = "../../../core/tests/common/sealed_state.rs"]
pub(crate) mod common;

pub(crate) fn plan(lanes: u32) -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    fragment.execution_regions.push(ExecutionRegion {
        region_id: ExecutionRegionId::from("region"),
        admitted_placements: vec![PlacementId::from("placement")],
        execution_profile_id: ExecutionProfileId::from("fixture/cooperative@1"),
        scheduling: ExecutionScheduling::CooperativeBoundedStep,
        lane_count: lanes,
        lane_resource: ResourceBinding {
            pool_id: ResourcePoolId::from("lanes"),
            class_id: ResourceClassId::from("conduit.resource/execution-lane@1"),
            units: lanes,
            protected: None,
            content: None,
            compute: Some(ComputeReservation {
                selected_lanes: lanes,
                service_guarantee: ComputeServiceGuarantee::Shared,
                architecture_base_id: ArchitectureBaseId::from("base"),
                architecture_base_kind: ArchitectureBaseKind::HostedOs,
                topology_group_id: None,
                performance_class: None,
                nominal_clock_hz: None,
            }),
        },
        lane_base_id: HostBaseId::from("base"),
        requirements: ExecutionRegionRequirements {
            runtime_memory_bytes: 0,
            timer_slots: 0,
            cord_item_capacity: 0,
            cord_byte_capacity: 0,
            mandatory_sign_items: fragment.sign_storage_budget.item_capacity,
            mandatory_sign_bytes: fragment.sign_storage_budget.byte_capacity,
        },
        preemption_required: false,
        isolation_required: false,
    });
    common::seal(fragment)
}

pub(crate) fn transfer_plan() -> Plan {
    let mut fragment = plan(1).fragments.remove(0);
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
    fragment.startup_order.push(sink.placement_id.clone());
    let mut region = fragment.execution_regions[0].clone();
    region.region_id = ExecutionRegionId::from("sink");
    region.admitted_placements = vec![sink.placement_id.clone()];
    region.requirements.cord_item_capacity = 2;
    region.requirements.cord_byte_capacity = 2;
    fragment.placements.push(sink);
    fragment.execution_regions.push(region);
    for region in &mut fragment.execution_regions {
        region.scheduling = ExecutionScheduling::PartitionedBoundedStep;
        region.requirements.cord_item_capacity = 2;
        region.requirements.cord_byte_capacity = 2;
    }
    fragment.connections.push(PlannedConnection {
        connection_id: ConnectionId::from("transfer"),
        source_placement_id: PlacementId::from("placement"),
        source_port_id: port_id("current"),
        sink_placement_id: PlacementId::from("sink"),
        sink_port_id: port_id("next"),
        value_kind: kind_id("fixture/byte@1"),
        resource: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 2,
        byte_capacity: 2,
    });
    common::seal(fragment)
}
