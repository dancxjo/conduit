use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn schema(ty: &StructuredInfoType) -> &'static [u8] {
    Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice())
}
#[test]
fn static_selector_preserves_outputs_refusals_and_zero_play_allocation() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = StructuredInfoType::collection(leaf.clone(), Some(2)).unwrap();
    let value = StructuredInfoValue::collection(
        ty.clone(),
        vec![
            StructuredInfoValue::leaf(leaf.clone(), InfoBool::TRUE.encode().to_vec()).unwrap(),
            StructuredInfoValue::leaf(leaf, InfoBool::FALSE.encode().to_vec()).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let selector = StructuredSelector::index(ty, 1).unwrap();
    let input_schema = schema(selector.input_type());
    let output_schema = schema(selector.output_type());
    let (mut owned, owned_memory) = allocation_probe::observe(|| {
        PreparedCanonicalStructuredSelector::prepare(&selector, 1024, 1024).unwrap()
    });
    let (mut borrowed, static_memory) = allocation_probe::observe(|| {
        PreparedCanonicalStructuredSelector::with_static_schema(
            &selector,
            input_schema,
            output_schema,
            1024,
            1024,
        )
        .unwrap()
    });
    assert_eq!(
        owned_memory.retained_bytes() - static_memory.retained_bytes(),
        input_schema.len() + output_schema.len()
    );
    assert!(PreparedCanonicalStructuredSelector::with_static_schema(
        &selector,
        b"wrong",
        output_schema,
        1024,
        1024
    )
    .is_err());
    assert!(PreparedCanonicalStructuredSelector::with_static_schema(
        &selector,
        input_schema,
        b"wrong",
        1024,
        1024
    )
    .is_err());
    let mut corpus = vec![value.clone()];
    for i in 0..value.len() {
        corpus.push(value[..i].to_vec());
        let mut bad = value.clone();
        bad[i] ^= 255;
        corpus.push(bad);
    }
    let (_, runtime) = allocation_probe::observe(|| {
        for bytes in &corpus {
            assert_eq!(owned.select(bytes), borrowed.select(bytes));
        }
    });
    assert_eq!(runtime.allocations, 0);
    assert_eq!(runtime.reallocations, 0);
    println!(
        "selector owned_heap={} static_heap={} schema_bytes={}",
        owned_memory.retained_bytes(),
        static_memory.retained_bytes(),
        input_schema.len() + output_schema.len()
    );
}
#[test]
fn static_validator_retains_no_schema_heap_and_same_runtime_bounds() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let schema = schema(&ty);
    let value = StructuredInfoValue::leaf(ty.clone(), InfoBool::TRUE.encode().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let (owned, owned_memory) =
        allocation_probe::observe(|| PreparedStructuredValueValidator::new(&ty, 64).unwrap());
    let (borrowed, static_memory) = allocation_probe::observe(|| {
        PreparedStructuredValueValidator::with_static_schema(&ty, schema, 64).unwrap()
    });
    assert_eq!(owned_memory.retained_bytes(), schema.len());
    assert_eq!(static_memory.retained_bytes(), 0);
    let (_, runtime) = allocation_probe::observe(|| {
        for _ in 0..100 {
            assert_eq!(owned.validate(&value), borrowed.validate(&value));
        }
    });
    assert_eq!(runtime.allocations, 0);
    for maximum in [0, schema.len(), MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1] {
        assert_eq!(
            PreparedStructuredValueValidator::new(&ty, maximum).err(),
            PreparedStructuredValueValidator::with_static_schema(&ty, schema, maximum).err()
        );
    }
    println!(
        "validator owned_heap={} static_heap={}",
        owned_memory.retained_bytes(),
        static_memory.retained_bytes()
    );
}
