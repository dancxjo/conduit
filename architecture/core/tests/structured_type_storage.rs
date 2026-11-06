//! Independently compare schema custody with actual requested allocation sizes.
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{KindId, StructuredFieldType, StructuredInfoType, StructuredVariantCase};

fn text(value: &str, capacity: usize) -> String {
    let mut result = String::with_capacity(capacity);
    result.push_str(value);
    result
}

#[test]
fn unique_schema_backing_agrees_with_requested_live_allocations() {
    let (schema, observed) = allocation_probe::observe(|| {
        let leaf = StructuredInfoType::leaf(KindId(text("value/u8", 97))).unwrap();
        let collection = StructuredInfoType::collection(leaf, Some(2)).unwrap();
        let sequence = StructuredInfoType::sequence(collection, 3).unwrap();
        let nominal =
            StructuredInfoType::nominal(KindId(text("fixture/nominal", 83)), sequence).unwrap();
        let mut fields = Vec::with_capacity(7);
        fields.push(StructuredFieldType::new(text("reading", 71), nominal).unwrap());
        let record =
            StructuredInfoType::record(KindId(text("fixture/record", 67)), fields).unwrap();
        let mut cases = Vec::with_capacity(5);
        cases.push(StructuredVariantCase::new(text("complete", 61), record).unwrap());
        StructuredInfoType::variant(KindId(text("fixture/variant", 59)), cases).unwrap()
    });
    assert_eq!(observed.deallocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.peak_bytes, observed.requested_bytes);
    assert_eq!(
        schema.retained_backing_bytes(),
        Some(observed.requested_bytes)
    );
    let (cloned, cloned_backing) =
        allocation_probe::observe(|| std::hint::black_box(schema.clone()));
    #[cfg(target_has_atomic = "ptr")]
    assert_eq!(cloned_backing.allocations, 0);
    assert_eq!(
        cloned.retained_backing_bytes(),
        schema.retained_backing_bytes()
    );
    drop(schema);
    assert!(cloned.profile().is_ok());
}
