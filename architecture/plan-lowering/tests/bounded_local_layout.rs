//! Requested owned-storage proof for the same ordinary local lowering.
use conduit_core::*;
use conduit_plan_lowering::{
    fragment_set::{
        lower_local_fragment_set_from_plans, lower_local_fragment_set_from_plans_with_limit,
        lower_local_fragment_set_layout_from_plans, lowered_local_fragment_set_backing_bytes,
        FragmentSetBounds, FragmentSetError,
    },
    lowering::{LoweringError, FIXED_KERNEL_STORAGE_PROFILE},
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn bounds() -> FragmentSetBounds {
    FragmentSetBounds {
        fragments: 4,
        nodes: 16,
        cords: 16,
        queue_slots: 32,
        value_bytes: 4096,
        sign_items: 128,
        sign_bytes: 8192,
    }
}
fn fanout(source: &str) -> Plan {
    let mut fragment = common::fragment();
    fragment.source_document_id = source.into();
    fragment.states.clear();
    fragment.placements[0].inputs.clear();
    fragment.placements[0].limits.max_queue_items = 8;
    fragment.placements[0].limits.max_queue_bytes = 16;
    for name in ["sink-z", "sink-a"] {
        let mut sink = fragment.placements[0].clone();
        sink.placement_id = name.into();
        sink.gear_id = name.into();
        sink.inputs = vec![PortDescriptor {
            port_id: port_id("next"),
            value_kind: kind_id("fixture/byte@1"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }];
        sink.outputs.clear();
        fragment.placements.push(sink);
        fragment.startup_order.push(name.into());
        fragment.connections.push(PlannedConnection {
            connection_id: name.into(),
            source_placement_id: "placement".into(),
            source_port_id: port_id("current"),
            sink_placement_id: name.into(),
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
    }
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    common::seal(fragment)
}
fn fore() -> Plan {
    let mut fragment = common::fragment();
    fragment.states.clear();
    fragment.fore_ports = [
        ("commands", "next", PortDirection::Input),
        ("states", "current", PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| PlannedForePort {
        front_port_id: port_id(front),
        direction,
        placement_id: "placement".into(),
        gear_port_id: port_id(gear),
        value_kind: kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    })
    .collect();
    fragment.sign_storage_budget =
        mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    common::seal(fragment)
}
#[test]
fn borrowed_layout_and_one_byte_short_refusal_allocate_nothing() {
    let plan = fanout("first");
    let plans = [&plan];
    let (layout, observation) = allocation_probe::observe(|| {
        lower_local_fragment_set_layout_from_plans(&plans, FIXED_KERNEL_STORAGE_PROFILE, bounds())
    });
    let layout = layout.unwrap();
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    let (refused, observation) = allocation_probe::observe(|| {
        lower_local_fragment_set_from_plans_with_limit(
            &plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
            layout.peak_owned_bytes - 1,
        )
    });
    assert!(
        matches!(refused,Err(FragmentSetError::Capacity(ref d)) if d.resource=="lowering-owned-bytes")
    );
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
}
#[test]
fn funded_and_existing_entrances_preserve_full_tables_and_original_ordinals() {
    let first = fanout("first");
    let second = fanout("second");
    let plans = [&first, &second];
    let layout =
        lower_local_fragment_set_layout_from_plans(&plans, FIXED_KERNEL_STORAGE_PROFILE, bounds())
            .unwrap();
    let original =
        lower_local_fragment_set_from_plans(&plans, FIXED_KERNEL_STORAGE_PROFILE, bounds())
            .unwrap();
    let (funded, allocation) = allocation_probe::observe(|| {
        lower_local_fragment_set_from_plans_with_limit(
            &plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
            layout.peak_owned_bytes,
        )
    });
    let funded = funded.unwrap();
    let (actual, inspection) =
        allocation_probe::observe(|| lowered_local_fragment_set_backing_bytes(&funded));
    assert!(actual.unwrap() <= layout.retained_owned_bytes);
    assert_eq!(inspection.allocations, 0);
    assert_eq!(funded.partitions, original.partitions);
    assert!(
        allocation.peak_bytes <= layout.peak_owned_bytes,
        "{allocation:?}, {layout:?}"
    );
    assert_eq!(allocation.reallocations, 0);
    assert_eq!(
        (
            funded.nodes,
            funded.cords,
            funded.queue_slots,
            funded.value_bytes
        ),
        (6, 4, 8, 8)
    );
    let first = &funded.partitions[0];
    assert_eq!(first.nodes[1].placement_id.as_str(), "sink-z");
    assert_eq!(first.nodes[2].placement_id.as_str(), "sink-a");
    assert_eq!(first.routes[0].targets[0].cord.0, 0);
    assert_eq!(first.routes[0].targets[1].cord.0, 1);
    assert_eq!(funded.partitions[1].routes[0].source_node.0, 3);
    assert_eq!(funded.partitions[1].routes[0].range.start, 2);
}
#[test]
fn fore_identity_and_all_intermediate_storage_fit_the_same_layout() {
    let plan = fore();
    let plans = [&plan];
    let layout =
        lower_local_fragment_set_layout_from_plans(&plans, FIXED_KERNEL_STORAGE_PROFILE, bounds())
            .unwrap();
    let expected =
        lower_local_fragment_set_from_plans(&plans, FIXED_KERNEL_STORAGE_PROFILE, bounds())
            .unwrap();
    let (result, allocation) = allocation_probe::observe(|| {
        lower_local_fragment_set_from_plans_with_limit(
            &plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
            layout.peak_owned_bytes,
        )
    });
    assert_eq!(result.unwrap().partitions, expected.partitions);
    assert!(
        allocation.peak_bytes <= layout.peak_owned_bytes,
        "{allocation:?}, {layout:?}"
    );
    assert_eq!(allocation.reallocations, 0);
}
#[test]
fn unsupported_shape_overflow_and_invalid_seal_never_gain_a_lowering_receipt() {
    let mut state = common::retained_fragment();
    state.sign_storage_budget = mandatory_sign_storage_requirement(&state.expected_sign).unwrap();
    let state = common::seal(state);
    let (result, allocation) = allocation_probe::observe(|| {
        lower_local_fragment_set_layout_from_plans(
            &[&state],
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
        )
    });
    assert!(matches!(
        result,
        Err(FragmentSetError::Fragment(
            LoweringError::UnsupportedOwnedLayout
        ))
    ));
    assert_eq!(allocation.allocations, 0);
    let mut plan = fanout("overflow");
    plan.fragments[0].connections[0].byte_capacity = u32::MAX;
    plan.fragments[0].connections[1].byte_capacity = 1;
    let (result, allocation) = allocation_probe::observe(|| {
        lower_local_fragment_set_layout_from_plans(&[&plan], FIXED_KERNEL_STORAGE_PROFILE, bounds())
    });
    assert!(matches!(
        result,
        Err(FragmentSetError::Fragment(LoweringError::CapacityOverflow))
    ));
    assert_eq!(allocation.allocations, 0);
    let mut plan = fanout("bad-seal");
    plan.plan_id = "unsealed".into();
    let layout = lower_local_fragment_set_layout_from_plans(
        &[&plan],
        FIXED_KERNEL_STORAGE_PROFILE,
        bounds(),
    )
    .unwrap();
    assert!(matches!(
        lower_local_fragment_set_from_plans_with_limit(
            &[&plan],
            FIXED_KERNEL_STORAGE_PROFILE,
            bounds(),
            layout.peak_owned_bytes
        ),
        Err(FragmentSetError::Fragment(LoweringError::InvalidFragment))
    ));
}
