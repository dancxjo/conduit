//! Full canonical validation survives preparation-memory reduction.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
extern crate alloc;
#[path = "../src/canonical_input.rs"]
mod canonical_input;
#[test]
fn borrowed_validation_matches_prepared_shape_checks_without_allocation() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("fixture/selector@1"),
        vec![StructuredFieldType::new("on", leaf.clone()).unwrap()],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "on",
            StructuredInfoValue::leaf(leaf, InfoBool::TRUE.encode().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let prepared = canonical_input::PreparedCanonicalInput::new(&ty, value.len()).unwrap();
    let linked_schema = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
    let linked = canonical_input::PreparedCanonicalInput::with_static_schema(
        &ty,
        linked_schema,
        value.len(),
    )
    .unwrap();
    assert!(
        canonical_input::PreparedCanonicalInput::with_static_schema(&ty, &[], value.len(),)
            .is_err()
    );
    let constrained = canonical_input::PreparedCanonicalInput::new(&ty, value.len() - 1).unwrap();
    let original = PreparedStructuredValueValidator::new(&ty, value.len()).unwrap();
    assert!(canonical_input::PreparedCanonicalInput::new(&ty, 0).is_err());
    assert!(
        canonical_input::PreparedCanonicalInput::new(&ty, ty.canonical_bytes().unwrap().len())
            .is_err()
    );
    let mut wrong_type = value.clone();
    wrong_type[5] ^= 1;
    let mut invalid_leaf = value.clone();
    *invalid_leaf.last_mut().unwrap() = 2;
    let mut trailing = value.clone();
    trailing.push(0);
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(prepared.validate(&value), original.validate(&value));
        assert_eq!(linked.validate(&value), original.validate(&value));
        for end in 0..value.len() {
            assert_eq!(
                prepared.validate(&value[..end]).is_ok(),
                original.validate(&value[..end]).is_ok()
            );
        }
        for invalid in [&wrong_type, &invalid_leaf, &trailing] {
            assert!(prepared.validate(invalid).is_err());
            assert!(linked.validate(invalid).is_err());
            assert_eq!(
                prepared.validate(invalid).is_ok(),
                original.validate(invalid).is_ok()
            );
        }
        assert!(constrained.validate(&value).is_err());
        assert!(prepared.validate(&value).is_ok());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
