//! Stable-order and allocation parity for the exact shared production helper.
//! Source/Plan validation remains in the unchanged attachment callers.
use conduit_core::{
    kind_id, port_id, ConnectionTrack, DeliveryPressurePolicy, PlacementId, PlannedForePort,
    PortDirection, PortTemporal,
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../src/canonical/authoring/fore_port_sort.rs"]
mod fore_port_sort;

fn port(index: usize) -> PlannedForePort {
    PlannedForePort {
        front_port_id: port_id(&format!("fore/{}", index % 3)),
        direction: if index.is_multiple_of(2) {
            PortDirection::Input
        } else {
            PortDirection::Output
        },
        placement_id: PlacementId::from(format!("entry/{index}")),
        gear_port_id: port_id("value"),
        value_kind: kind_id("fixture/value"),
        value_contract: None,
        abnormal_kind: None,
        track: match index % 4 {
            0 => ConnectionTrack::Payload,
            1 => ConnectionTrack::NormalClose,
            2 => ConnectionTrack::AbnormalTerminal,
            _ => ConnectionTrack::Quiescence,
        },
        temporal: PortTemporal::Flow { closes: true },
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 8,
        selected_line: None,
    }
}
#[test]
fn shared_sort_matches_both_original_closures_and_preserves_equal_key_order() {
    for count in [0usize, 1, 4, 16, 33, 64, 127] {
        let original: Vec<_> = (0..count).rev().map(port).collect();
        let mut expected = original.clone();
        // Frozen comparator from both original attachment branches.
        expected.sort_by(|left, right| {
            (left.direction as u8, &left.front_port_id, left.track).cmp(&(
                right.direction as u8,
                &right.front_port_id,
                right.track,
            ))
        });
        let mut actual = original.clone();
        fore_port_sort::sort_fore_ports(&mut actual);
        assert_eq!(actual, expected);
        // Every twelfth row has the same complete sort key. Check original
        // relative order independently of the old-sort oracle as well.
        for group in 0..12 {
            let before: Vec<_> = original
                .iter()
                .filter(|p| {
                    p.placement_id
                        .as_str()
                        .strip_prefix("entry/")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()
                        % 12
                        == group
                })
                .map(|p| p.placement_id.as_str())
                .collect();
            let after: Vec<_> = actual
                .iter()
                .filter(|p| {
                    p.placement_id
                        .as_str()
                        .strip_prefix("entry/")
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()
                        % 12
                        == group
                })
                .map(|p| p.placement_id.as_str())
                .collect();
            assert_eq!(after, before);
        }
    }
}
#[test]
fn shared_sort_preserves_existing_stable_sort_allocation_behavior() {
    for count in [0usize, 1, 4, 16, 33, 64, 127] {
        let mut expected: Vec<_> = (0..count).rev().map(port).collect();
        let mut actual = expected.clone();
        let (_, old) = allocation_probe::observe(|| {
            expected.sort_by(|left, right| {
                (left.direction as u8, &left.front_port_id, left.track).cmp(&(
                    right.direction as u8,
                    &right.front_port_id,
                    right.track,
                ))
            })
        });
        let (_, shared) =
            allocation_probe::observe(|| fore_port_sort::sort_fore_ports(&mut actual));
        assert_eq!(actual, expected);
        assert_eq!(
            (
                shared.allocations,
                shared.reallocations,
                shared.requested_bytes,
                shared.peak_bytes
            ),
            (
                old.allocations,
                old.reallocations,
                old.requested_bytes,
                old.peak_bytes
            )
        );
    }
}
