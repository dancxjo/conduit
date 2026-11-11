//! A prepared selector borrows its one checked schema and owns one output slot.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
use conduit_semantic_catalog::PreparedCanonicalSelector;
#[test]
fn selection_validates_unselected_primitive_leaves_without_play_allocations() {
    let leaf = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = StructuredInfoType::record(
        kind_id("test/two-bools@1"),
        vec![
            StructuredFieldType::new("chosen", leaf.clone()).unwrap(),
            StructuredFieldType::new("other", leaf.clone()).unwrap(),
        ],
    )
    .unwrap();
    let yes = StructuredInfoValue::leaf(leaf.clone(), InfoBool::TRUE.encode().to_vec()).unwrap();
    let no = StructuredInfoValue::leaf(leaf, InfoBool::FALSE.encode().to_vec()).unwrap();
    let expected = yes.canonical_bytes().unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new("chosen", yes).unwrap(),
            StructuredFieldValue::new("other", no).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let selector = StructuredSelector::field(ty.clone(), "chosen").unwrap();
    let mut prepared = PreparedCanonicalSelector::prepare(selector.clone(), 512, 512).unwrap();
    let canonical = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
    let mut linked = PreparedCanonicalSelector::prepare_static(canonical, 512, 512).unwrap();
    let mut trailing_selector = canonical.to_vec();
    trailing_selector.push(0);
    assert!(PreparedCanonicalSelector::prepare_static(
        Box::leak(trailing_selector.into_boxed_slice()),
        512,
        512,
    )
    .is_err());
    let mut malformed = value.clone();
    *malformed.last_mut().unwrap() = 2; // Invalid Boolean in the unselected field.
    let mut trailing = value.clone();
    trailing.push(0);
    let mut wrong_type = value.clone();
    wrong_type[5] ^= 1;
    let mut too_small = PreparedCanonicalSelector::prepare(selector, value.len() - 1, 512).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        for prepared in [&mut prepared, &mut linked] {
            assert_eq!(prepared.select(&value).unwrap(), Some(expected.as_slice()));
            assert_eq!(prepared.output(), expected);
            for input in [&malformed, &trailing, &wrong_type] {
                assert_eq!(
                    prepared.select(input),
                    Err(StructuredSelectorRefusal::MalformedCheckedValue)
                );
            }
        }
        assert_eq!(
            too_small.select(&value),
            Err(StructuredSelectorRefusal::MalformedCheckedValue)
        );
    });
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
}
