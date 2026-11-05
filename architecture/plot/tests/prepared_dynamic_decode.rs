//! Gate boundaries only; generated storage remains the sole value constructor.
use conduit_core::{kind_id, StructuredFieldValue, StructuredInfoType, StructuredInfoValue};
use conduit_plot::{rust_binding::*, *};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn doc() -> CheckedSyntaxDocument {
    check_syntax_document(
        &parse_syntax_document("type Counter = {\n value: U64\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap()
}
fn native(doc: &CheckedSyntaxDocument) -> &CheckedNativeType {
    doc.native_types
        .iter()
        .find(|n| n.name == "Counter")
        .unwrap()
}
fn admitted(doc: &CheckedSyntaxDocument, value: u64) -> PreparedNativeAdmission<'_> {
    let bytes = StructuredInfoValue::record(
        native(doc).value_type.clone(),
        vec![StructuredFieldValue::new(
            "value",
            primitive_into_structured(
                StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                &value,
            )
            .unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut a = PreparedNativeAdmission::new(
        doc,
        &native(doc).identity,
        PreparedNativeBounds {
            canonical_bytes: 16384,
            scopes: 16,
            invariants: 16,
            contracts: 16,
        },
    )
    .unwrap();
    a.begin(&bytes).unwrap();
    while a.step().unwrap() != PreparedNativeProgress::Admitted {}
    a
}
fn bounds(steps: usize) -> PreparedDecodeBounds {
    PreparedDecodeBounds {
        canonical_bytes: 16384,
        leaf_steps: steps,
        text_bytes: 0,
    }
}
fn quiet(o: allocation_probe::Observation) {
    assert_eq!((o.allocations, o.reallocations, o.deallocations), (0, 0, 0));
}
#[test]
fn actual_work_can_finish_below_the_prepared_ceiling_without_growth() {
    let d = doc();
    let a = admitted(&d, 4);
    let witness = a.admitted().unwrap();
    let mut gate = PreparedDynamicDecodeGate::new(a.native_type(), 4, bounds(2)).unwrap();
    let ((), counts) = allocation_probe::observe(|| {
        assert_eq!(gate.advance(false), Err(PreparedDecodeRefusal::State));
        gate.step_value(&witness).unwrap();
        assert!(matches!(
            gate.step_value(&witness),
            Err(PreparedDecodeRefusal::State)
        ));
        assert_eq!(gate.advance(true), Ok(PreparedDecodeProgress::Complete));
        gate.finish(&witness).unwrap();
        assert!(matches!(
            gate.step_value(&witness),
            Err(PreparedDecodeRefusal::State)
        ));
    });
    quiet(counts);
    assert_eq!(gate.consumed_steps(), 1);
    assert!(gate.canonical_capacity() >= 16384);
}
#[test]
fn exhausted_actual_work_remains_pressure_and_retains_canonical_backing() {
    let d = doc();
    let a = admitted(&d, 4);
    let witness = a.admitted().unwrap();
    let mut gate = PreparedDynamicDecodeGate::new(a.native_type(), 4, bounds(2)).unwrap();
    let capacity = gate.canonical_capacity();
    let ((), counts) = allocation_probe::observe(|| {
        for _ in 0..2 {
            gate.step_value(&witness).unwrap();
            gate.advance(false).unwrap();
        }
        assert!(matches!(
            gate.step_value(&witness),
            Err(PreparedDecodeRefusal::Pressure)
        ));
        assert_eq!(gate.finish(&witness), Err(PreparedDecodeRefusal::Pressure));
        assert_eq!(gate.advance(true), Err(PreparedDecodeRefusal::Pressure));
    });
    quiet(counts);
    assert_eq!(gate.consumed_steps(), 2);
    assert_eq!(gate.canonical_capacity(), capacity);
}
#[test]
fn changed_or_foreign_witness_cancel_and_canonical_pressure_never_finish() {
    let d = doc();
    let other = doc();
    let a = admitted(&d, 4);
    let changed = admitted(&d, 7);
    let foreign = admitted(&other, 4);
    let original = a.admitted().unwrap();
    for replacement in [changed.admitted().unwrap(), foreign.admitted().unwrap()] {
        let mut gate = PreparedDynamicDecodeGate::new(a.native_type(), 4, bounds(4)).unwrap();
        let ((), counts) = allocation_probe::observe(|| {
            gate.step_value(&original).unwrap();
            gate.advance(false).unwrap();
            assert!(matches!(
                gate.step_value(&replacement),
                Err(PreparedDecodeRefusal::ForeignWitness)
            ));
            assert_eq!(
                gate.finish(&original),
                Err(PreparedDecodeRefusal::ForeignWitness)
            );
        });
        quiet(counts);
    }
    let mut cancelled = PreparedDynamicDecodeGate::new(a.native_type(), 4, bounds(4)).unwrap();
    let mut small = PreparedDynamicDecodeGate::new(
        a.native_type(),
        4,
        PreparedDecodeBounds {
            canonical_bytes: 1,
            ..bounds(4)
        },
    )
    .unwrap();
    let ((), counts) = allocation_probe::observe(|| {
        cancelled.step_value(&original).unwrap();
        cancelled.cancel();
        assert_eq!(
            cancelled.finish(&original),
            Err(PreparedDecodeRefusal::Cancelled)
        );
        assert!(matches!(
            small.step_value(&original),
            Err(PreparedDecodeRefusal::Pressure)
        ));
        assert_eq!(
            small.finish(&original),
            Err(PreparedDecodeRefusal::Pressure)
        );
    });
    quiet(counts);
}
