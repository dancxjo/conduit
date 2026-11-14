extern crate alloc;
use super::*;
use alloc::boxed::Box;
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};
use conduit_kernel::{scheduler::HostCallBack, ValueStorage};
struct Owners {
    retained: [RetainedInputs<2>; 4],
    pins: [Option<[bool; 2]>; 4],
}
impl Owners {
    fn copying() -> Self {
        Self {
            retained: [RetainedInputs::NONE; 4],
            pins: [Some([false; 2]); 4],
        }
    }
}
// SAFETY: this fixture's reports describe its selected mock lifecycle. No owners
// or equipment are transferred by inspection; bind is deliberately not exercised.
unsafe impl PreparedBacks<4, 2> for Owners {
    type Driver = HostCallBack;
    type Calls = ();
    fn retained_inputs(&self, node: usize) -> RetainedInputs<2> {
        self.retained
            .get(node)
            .copied()
            .unwrap_or(RetainedInputs::Unknown)
    }
    fn pending_input_pins(&self, node: usize) -> Option<[bool; 2]> {
        self.pins.get(node).copied().flatten()
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> ([HostCallBack; 4], ()) {
        panic!("cold inspection must not bind")
    }
}
fn material() -> (Profile, &'static [u8], Vec<u8>) {
    let ty = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), 17u64.to_le_bytes().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let bytes = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
    (
        Profile::Schema {
            canonical: bytes,
            maximum_bytes: value.len() as u32,
        },
        bytes,
        value,
    )
}
const SINKS: &[Option<(u16, u16)>] = &[Some((1, 0)), Some((1, 1)), Some((2, 0))];
const OUTPUTS: &[(u16, u16)] = &[(0, 0), (0, 1), (2, 0), (3, 0)];
#[test]
fn same_node_outputs_and_full_queue_stock_coexist_without_new_execution_allocations() {
    let (profile, schema, value) = material();
    let (mut pool, _, report) = prepare_shared_schema(
        &[profile; 3],
        SINKS,
        &[profile; 4],
        OUTPUTS,
        &[8],
        &[],
        16384,
        &Owners::copying(),
    )
    .unwrap();
    assert_eq!(
        report,
        SharedStockReport {
            logical_schema_slots: 7,
            physical_schema_slots: 5,
            reduced_classes: 1,
            unknown_owner_fallback: false
        }
    );
    assert_eq!(pool.item_capacity(), 8); // Native result remains canonical.
    let mut refs = [None; 5];
    let (_, cost) = crate::member_return_tests::return_allocation_probe::observe(|| {
        for r in &mut refs {
            *r = Some(pool.store_schema_canonical(schema, &value).unwrap());
        }
        // Fanout holds another reference, not another physical payload.
        let first = refs[0].unwrap();
        pool.retain(first).unwrap();
        pool.release(first).unwrap();
        assert!(pool.store_schema_canonical(schema, &value).is_err());
        pool.release(first).unwrap();
        let next = pool.store_schema_canonical(schema, &value).unwrap();
        assert!(pool.canonical_parts(first).is_err());
        assert_ne!(next, first);
        assert_eq!(
            pool.canonical_parts(next).unwrap().unwrap().node.len(),
            value.len() - schema.len()
        );
    });
    assert_eq!(cost.allocation_attempts, 0);
}
#[test]
fn state_alternatives_share_one_bound_but_native_pins_have_separate_stock() {
    let (profile, _, _) = material();
    let mut owners = Owners::copying();
    owners.retained[1] = RetainedInputs::Bounded {
        maximum_values: 1,
        possible_ports: [true, true],
    };
    let (_, _, held) = prepare_shared_schema(
        &[profile; 3],
        SINKS,
        &[profile; 4],
        OUTPUTS,
        &[8],
        &[],
        16384,
        &owners,
    )
    .unwrap();
    assert_eq!(held.physical_schema_slots, 6); // One held value across two alternatives.
    owners.pins[2] = Some([true, false]);
    let (_, _, pending) = prepare_shared_schema(
        &[profile; 3],
        SINKS,
        &[profile; 4],
        OUTPUTS,
        &[8],
        &[],
        16384,
        &owners,
    )
    .unwrap();
    assert_eq!(pending.physical_schema_slots, 7);
    assert_eq!(pending.reduced_classes, 0); // Never min-clamp an unmet proof.
    owners.retained[1] = RetainedInputs::Bounded {
        maximum_values: 9,
        possible_ports: [true, true],
    };
    let (_, _, unmet) = prepare_shared_schema(
        &[profile; 3],
        SINKS,
        &[profile; 4],
        OUTPUTS,
        &[8],
        &[],
        16384,
        &owners,
    )
    .unwrap();
    assert_eq!(unmet.physical_schema_slots, 7);
    assert_eq!(unmet.reduced_classes, 0);
}
#[test]
fn unknown_back_or_kernel_lifetime_keeps_all_original_stock_and_canonical_parameters() {
    let (profile, schema, value) = material();
    let mut owners = Owners::copying();
    for back_unknown in [true, false] {
        owners.retained[0] = if back_unknown {
            RetainedInputs::Unknown
        } else {
            RetainedInputs::NONE
        };
        owners.pins[0] = if back_unknown { Some([false; 2]) } else { None };
        let (mut pool, parameters, report) = prepare_shared_schema(
            &[profile; 3],
            SINKS,
            &[profile; 4],
            OUTPUTS,
            &[16],
            &[(3, 1000, 9)],
            16384,
            &owners,
        )
        .unwrap();
        assert!(report.unknown_owner_fallback);
        assert_eq!(report.physical_schema_slots, 7);
        assert_eq!(pool.get(parameters[0].1).unwrap(), 1000u64.to_le_bytes());
        for _ in 0..7 {
            pool.store_schema_canonical(schema, &value).unwrap();
        }
        assert!(pool.store_schema_canonical(schema, &value).is_err());
    }
}
#[test]
fn mismatched_duplicate_and_out_of_bounds_topology_cannot_select_stock() {
    let (profile, _, _) = material();
    let owners = Owners::copying();
    for (sinks, outputs) in [
        (&SINKS[..2], OUTPUTS),
        (SINKS, &[(0, 0), (0, 0), (2, 0), (3, 0)][..]),
        (SINKS, &[(0, 0), (0, 1), (2, 0), (4, 0)][..]),
    ] {
        assert!(prepare_shared_schema(
            &[profile; 3],
            sinks,
            &[profile; 4],
            outputs,
            &[8],
            &[],
            16384,
            &owners
        )
        .is_err());
    }
}
