//! Mutations pass a refreshed representation checksum, then meet independent pins.
use conduit_core as c;
use conduit_plan_lowering::{
    selected_assigned_expression::ResidentExpressionBindingRefusal as E,
    selected_assigned_operation::AssignedOperationInput,
};
pub fn run(owner: &mut Option<crate::image::Initialized>, assignment: &crate::support::Assignment) {
    let active = assignment.active();
    let original = owner.as_ref().unwrap() as *const _;
    // fuel, Kind, implementation, artifact, port/name/kind, configuration, work
    for (tag, offset) in [
        (c::ASSIGNED_NODE, 2),
        (c::ASSIGNED_NODE, 4),
        (c::ASSIGNED_NODE, 20),
        (c::ASSIGNED_NODE, 36),
        (c::ASSIGNED_PORT, 2),
        (c::ASSIGNED_PORT, 5),
        (c::ASSIGNED_PORT, 21),
        (c::ASSIGNED_CONFIGURATION, 2),
        (c::ASSIGNED_CONFIGURATION, 19),
        (c::ASSIGNED_CONFIGURATION, 23),
        (c::ASSIGNED_RESOURCE, 2),
        (c::ASSIGNED_RESOURCE, 4),
    ] {
        let bytes = mutate(&assignment.encoded, tag, offset);
        let mut selected = assignment.selected(&active);
        selected.input = AssignedOperationInput::Contiguous(&bytes);
        let (result, cost) = crate::probe::observe(|| {
            crate::image::Initialized::bind_assigned_retained(owner, selected)
        });
        assert!(result.is_err(), "mutation {tag}/{offset}");
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(owner.as_ref().unwrap() as *const _, original);
    }
    let stale = c::bind_active_play(
        &active.plan_id,
        &active.host_id,
        &"boot/stale".into(),
        active.play_sequence,
    );
    let mut forged = active.clone();
    forged.play_sequence += 1;
    for scope in [&stale, &forged] {
        let (result, cost) = crate::probe::observe(|| {
            crate::image::Initialized::bind_assigned_retained(owner, assignment.selected(scope))
        });
        assert!(matches!(result, Err(E::Scope)));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(owner.as_ref().unwrap() as *const _, original);
    }
    let mut selected = assignment.selected(&active);
    selected.work_resource = 1;
    assert!(matches!(
        crate::image::Initialized::bind_assigned_retained(owner, selected),
        Err(E::Selection)
    ));
    assert_eq!(owner.as_ref().unwrap() as *const _, original);
    // Borrowed records must reach the same checker and retain the same owner.
    let records: Vec<_> = c::AssignedPlanRecords::new(&assignment.encoded)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut selected = assignment.selected(&active);
    selected.input = AssignedOperationInput::Records {
        header: &assignment.encoded[..c::ASSIGNED_PLAN_HEADER_BYTES],
        records: &records,
    };
    let (node, cost) =
        crate::probe::observe(|| owner.as_ref().unwrap().check_assigned(selected).unwrap());
    assert_eq!(node, 0);
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(owner.as_ref().unwrap() as *const _, original);
}
fn mutate(original: &[u8], tag: u8, offset: usize) -> Vec<u8> {
    let mut bytes = original.to_vec();
    let mut cursor = c::ASSIGNED_PLAN_HEADER_BYTES;
    let mut changed = false;
    while cursor < bytes.len() {
        let length = usize::from(u16::from_le_bytes(
            bytes[cursor + 1..cursor + 3].try_into().unwrap(),
        ));
        if bytes[cursor] == tag {
            assert!(offset < length);
            bytes[cursor + 3 + offset] ^= 1;
            changed = true;
            break;
        }
        cursor += length + 3;
    }
    assert!(changed);
    let digest = c::assigned_plan_payload_digest(&bytes[c::ASSIGNED_PLAN_HEADER_BYTES..]);
    bytes[93..125].copy_from_slice(&digest);
    bytes
}
