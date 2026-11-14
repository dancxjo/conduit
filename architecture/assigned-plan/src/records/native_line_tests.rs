//! Layout pins are stable structure; full native bytes remain possession input.
use crate::*;
fn binding(boot: u8) -> AssignedNativeBinding {
    let id = AssignedIdentity([1; 16]);
    let fresh = AssignedIdentity([boot; 16]);
    AssignedNativeBinding {
        capability: id,
        kind_revision: id,
        execution_profile: id,
        maximum_instances: 1,
        queue_items: 1,
        queue_bytes: 64,
        base: id,
        instance: fresh,
        generation: u64::from(boot),
        base_implementation: id,
        mechanism: id,
        enforcement: 0,
        grant: fresh,
        authority: id,
        call: id,
        subject: id,
        host: id,
        boot: fresh,
        authority_capability: id,
        resource: 0,
        pool: id,
        resource_class: id,
        units: 1,
    }
}
#[test]
fn schema8_real_native_call_pins_stable_prefix_and_preserves_exact_tail_for_binder() {
    let prefix = core::array::from_fn(|i| i as u8);
    let mut a = [0; ASSIGNED_NATIVE_CALL_BYTES];
    let mut b = a;
    let first = binding(2);
    let later = binding(3);
    assert_eq!(
        encode_assigned_native_binding(&prefix, Some(&first), &mut a),
        Ok(ASSIGNED_NATIVE_CALL_BYTES)
    );
    assert_eq!(
        encode_assigned_native_binding(&prefix, Some(&later), &mut b),
        Ok(ASSIGNED_NATIVE_CALL_BYTES)
    );
    let pin = |payload: &[u8], schema| {
        AssignedRecordRequirement::from_kernel_record(
            AssignedRecord {
                tag: ASSIGNED_HOST_CALL,
                payload,
            },
            schema,
        )
    };
    for schema in [
        ASSIGNED_NATIVE_PLAN_SCHEMA,
        ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
        ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA,
    ] {
        assert_eq!(pin(&a, schema), pin(&b, schema));
    }
    assert_eq!(
        pin(&a, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA),
        AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_HOST_CALL,
            payload: &a[..47]
        })
    );
    assert_ne!(a, b);
    // The projection still carries the complete exact native binding. Layout
    // equality cannot substitute a foreign Boot/provider/grant for possession.
    assert_eq!(assigned_native_binding(&a), Ok(Some(first)));
    assert_eq!(assigned_native_binding(&b), Ok(Some(later)));
    assert_ne!(assigned_native_binding(&a), assigned_native_binding(&b));
    for offset in 0..47 {
        let mut changed = a;
        changed[offset] ^= 1;
        assert_ne!(
            pin(&a, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA),
            pin(&changed, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA)
        );
    }
    assert_ne!(
        pin(&a, ASSIGNED_KERNEL_PLAN_SCHEMA),
        pin(&b, ASSIGNED_KERNEL_PLAN_SCHEMA)
    );
}
#[test]
fn schema8_remote_cord_identity_endpoints_capacities_and_track_stay_exact() {
    let mut cord = [0; 42];
    cord[18] = 1;
    cord[30..32].copy_from_slice(&1u16.to_le_bytes());
    cord[32..36].copy_from_slice(&64u32.to_le_bytes());
    cord[37..41].copy_from_slice(&64u32.to_le_bytes());
    let pin = |bytes: &[u8], schema| {
        AssignedRecordRequirement::from_kernel_record(
            AssignedRecord {
                tag: ASSIGNED_CORD,
                payload: bytes,
            },
            schema,
        )
    };
    for offset in [2, 17, 19, 23, 24, 30, 32, 36, 37, 41] {
        let mut changed = cord;
        changed[offset] ^= 1;
        assert_ne!(
            pin(&cord, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA),
            pin(&changed, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA)
        );
    }
    let mut renamed = cord;
    renamed[2..18].fill(9);
    assert_eq!(
        pin(&cord, ASSIGNED_FORE_NATIVE_PLAN_SCHEMA),
        pin(&renamed, ASSIGNED_FORE_NATIVE_PLAN_SCHEMA)
    );
    assert_ne!(
        pin(&cord, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA),
        pin(&renamed, ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA)
    );
}

#[test]
fn explicit_line_profile_normalizes_only_exact_local_fore_cords() {
    let local = AssignedForeRequirement::new(
        2,
        3,
        AssignedForeDirection::Input,
        AssignedIdentity([1; 16]),
        [2; 32],
    );
    let binding = AssignedRemoteBinding {
        line: AssignedIdentity([3; 16]),
        local_host: AssignedIdentity([4; 16]),
        local_boot: AssignedIdentity([5; 16]),
        peer_host: AssignedIdentity([6; 16]),
        peer_boot: AssignedIdentity([7; 16]),
    };
    let line = AssignedLineForeRequirement::new(
        AssignedForeRequirement::new(
            8,
            9,
            AssignedForeDirection::Output,
            AssignedIdentity([8; 16]),
            [9; 32],
        ),
        binding,
        AssignedIdentity([10; 16]),
        AssignedIdentity([11; 16]),
        [12; 32],
    );
    let local_pins = [local];
    let lines = [line];
    let profile = AssignedLineForeProfile {
        local: &local_pins,
        lines: &lines,
    };
    let mut cord = [0; 42];
    cord[..2].copy_from_slice(&3u16.to_le_bytes());
    cord[18] = 1;
    cord[19..21].copy_from_slice(&2u16.to_le_bytes());
    let pin = |bytes: &[u8], profile| {
        AssignedRecordRequirement::from_kernel_record_with_line_fore(
            AssignedRecord {
                tag: ASSIGNED_CORD,
                payload: bytes,
            },
            profile,
        )
    };
    let mut renamed = cord;
    renamed[2..18].fill(9);
    assert_eq!(pin(&cord, profile), pin(&renamed, profile));
    let mut changed = renamed;
    changed[19] ^= 1;
    assert_ne!(pin(&cord, profile), pin(&changed, profile));
    changed = renamed;
    changed[32] ^= 1;
    assert_ne!(pin(&cord, profile), pin(&changed, profile));
    let mut remote = [0; 42];
    remote[..2].copy_from_slice(&9u16.to_le_bytes());
    remote[23] = 1;
    remote[24..26].copy_from_slice(&8u16.to_le_bytes());
    let mut changed = remote;
    changed[2..18].fill(9);
    assert_ne!(pin(&remote, profile), pin(&changed, profile));
    // Even a malformed ambiguous profile cannot launder a selected Line cord.
    let aliases = [*line.fore()];
    let ambiguous = AssignedLineForeProfile {
        local: &aliases,
        lines: &lines,
    };
    assert_ne!(pin(&remote, ambiguous), pin(&changed, ambiguous));
}
