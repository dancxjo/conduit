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
