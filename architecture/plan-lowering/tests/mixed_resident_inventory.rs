//! Representation preflight only: these node records are not admitted Plans.
use conduit_core as c;
use conduit_plan_lowering::{
    resident_inventory::*, selected_assigned_expression::ResidentExpressionIdentity,
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;
static RESIDENT: ResidentExpressionIdentity = ResidentExpressionIdentity {
    kind: c::AssignedIdentity([1; 16]),
    implementation: c::AssignedIdentity([2; 16]),
    artifact: c::AssignedIdentity([3; 16]),
    input_port: c::AssignedIdentity([4; 16]),
    input_kind: c::AssignedIdentity([5; 16]),
    output_port: c::AssignedIdentity([6; 16]),
    output_kind: c::AssignedIdentity([7; 16]),
    computation: c::AssignedComputationRequirement {
        node: 0,
        key: c::AssignedIdentity([8; 16]),
        text_bytes: 17,
        text_digest: [9; 32],
    },
    private_work_units: 1,
};
fn other(node: u16) -> OtherNodeIdentity {
    OtherNodeIdentity {
        node,
        kind: c::AssignedIdentity([10; 16]),
        implementation: c::AssignedIdentity([11; 16]),
        artifact: c::AssignedIdentity([12; 16]),
    }
}
fn resident(node: u16) -> OtherNodeIdentity {
    OtherNodeIdentity {
        node,
        kind: RESIDENT.kind,
        implementation: RESIDENT.implementation,
        artifact: RESIDENT.artifact,
    }
}
fn records(nodes: &[OtherNodeIdentity]) -> Vec<u8> {
    let mut bytes = vec![0; c::ASSIGNED_PLAN_HEADER_BYTES];
    bytes[..8].copy_from_slice(&c::assigned_plan_magic());
    bytes[8..10].copy_from_slice(&c::ASSIGNED_PLAN_SCHEMA.to_le_bytes());
    for n in nodes {
        bytes.push(c::ASSIGNED_NODE);
        bytes.extend_from_slice(&52u16.to_le_bytes());
        bytes.extend_from_slice(&n.node.to_le_bytes());
        bytes.extend_from_slice(&[0; 2]);
        bytes.extend_from_slice(&n.kind.0);
        bytes.extend_from_slice(&n.implementation.0);
        bytes.extend_from_slice(&n.artifact.0);
    }
    let length = bytes.len() as u16;
    bytes[10..12].copy_from_slice(&length.to_le_bytes());
    bytes
}
#[test]
fn mixed_preflight_preserves_sparse_resident_node_numbers_without_allocating() {
    for nodes in [[other(0), resident(1)], [resident(1), other(0)]] {
        let bytes = records(&nodes);
        assert!(select_resident_inventory::<1>(&bytes, |_| Some(&RESIDENT)).is_err());
        let (selected, cost) = probe::observe(|| {
            select_resident_inventory_with_other_nodes::<1>(&bytes, &[other(0)], |_| {
                Some(&RESIDENT)
            })
            .unwrap()
        });
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(
            selected.nodes().collect::<Vec<_>>(),
            vec![ResidentNodeSelection { node: 1, entry: 0 }]
        );
        assert_eq!(selected.computations()[0].node, 1);
        assert_eq!(
            selected.computations()[0].text_digest,
            RESIDENT.computation.text_digest
        );
    }
    let bytes = records(&[resident(0)]);
    let selected = select_resident_inventory::<1>(&bytes, |_| Some(&RESIDENT)).unwrap();
    assert_eq!(selected.nodes().next().unwrap().node, 0);
}
#[test]
fn mixed_preflight_refuses_unaccounted_ambiguous_stale_and_non_dense_nodes() {
    let valid = records(&[other(0), resident(1)]);
    for field in 0..3 {
        let mut stale = other(0);
        match field {
            0 => stale.kind.0[0] ^= 1,
            1 => stale.implementation.0[0] ^= 1,
            _ => stale.artifact.0[0] ^= 1,
        }
        assert!(
            select_resident_inventory_with_other_nodes::<1>(&valid, &[stale], |_| Some(&RESIDENT))
                .is_err()
        );
    }
    for nodes in [
        vec![resident(1)],
        vec![other(0), resident(0)],
        vec![other(0), resident(2)],
        vec![other(0), resident(1), resident(1)],
    ] {
        assert!(select_resident_inventory_with_other_nodes::<1>(
            &records(&nodes),
            &[other(0)],
            |_| Some(&RESIDENT)
        )
        .is_err());
    }
    assert!(
        select_resident_inventory_with_other_nodes::<1>(&valid, &[other(0), other(0)], |_| Some(
            &RESIDENT
        ))
        .is_err()
    );
    assert!(
        select_resident_inventory_with_other_nodes::<1>(&valid, &[resident(0)], |_| Some(
            &RESIDENT
        ))
        .is_err()
    );
    assert!(
        select_resident_inventory_with_other_nodes::<1>(&valid, &[other(0)], |_| None).is_err()
    );
    let duplicate = records(&[other(0), resident(1), resident(2)]);
    assert!(
        select_resident_inventory_with_other_nodes::<2>(&duplicate, &[other(0)], |i| if i == 0 {
            Some(&RESIDENT)
        } else {
            None
        })
        .is_err()
    );
    assert!(
        select_resident_inventory_with_other_nodes::<2>(&valid, &[other(0)], |_| Some(&RESIDENT))
            .is_err()
    );
}

#[test]
fn full_bitmap_boundary_is_finite_and_out_of_range_nodes_refuse() {
    let others: Vec<_> = (1..32).map(other).collect();
    let mut nodes = others.clone();
    nodes.push(resident(0));
    let selected =
        select_resident_inventory_with_other_nodes::<1>(&records(&nodes), &others, |_| {
            Some(&RESIDENT)
        })
        .unwrap();
    assert_eq!(selected.nodes().next().unwrap().node, 0);
    let mut invalid = others.clone();
    invalid[0].node = u16::MAX;
    assert!(
        select_resident_inventory_with_other_nodes::<1>(&records(&nodes), &invalid, |_| Some(
            &RESIDENT
        ))
        .is_err()
    );
    invalid = others;
    invalid.push(other(32));
    assert!(
        select_resident_inventory_with_other_nodes::<1>(&records(&nodes), &invalid, |_| Some(
            &RESIDENT
        ))
        .is_err()
    );
}
