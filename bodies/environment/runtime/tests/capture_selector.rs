//! Full canonical validation survives preparation-memory reduction.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
use conduit_environment_runtime::PreparedCaptureSelector;
#[test]
fn exact_type_full_value_and_capacity_refusals_do_not_allocate_or_poison_next_selection() {
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
    let selector = StructuredSelector::field(ty.clone(), "on").unwrap();
    let expected = selector
        .select(&StructuredInfoValue::from_canonical_bytes(&value).unwrap())
        .unwrap();
    let StructuredSelection::Matched(expected) = expected else {
        unreachable!()
    };
    let expected = expected.canonical_bytes().unwrap();
    let mut prepared =
        PreparedCaptureSelector::prepare(selector.clone(), value.len(), 1024).unwrap();
    let mut constrained =
        PreparedCaptureSelector::prepare(selector.clone(), value.len(), expected.len() - 1)
            .unwrap();
    assert!(PreparedCaptureSelector::prepare(selector.clone(), 0, 1024).is_err());
    assert!(PreparedCaptureSelector::prepare(
        selector.clone(),
        ty.canonical_bytes().unwrap().len(),
        1024
    )
    .is_err());
    let mut wrong_type = value.clone();
    wrong_type[5] ^= 1;
    let mut invalid_leaf = value.clone();
    *invalid_leaf.last_mut().unwrap() = 2;
    let mut trailing = value.clone();
    trailing.push(0);
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(prepared.select(&value).unwrap().unwrap(), expected);
        for end in 0..value.len() {
            assert!(prepared.select(&value[..end]).is_err());
        }
        assert!(prepared.select(&wrong_type).is_err());
        assert!(prepared.select(&invalid_leaf).is_err());
        assert!(prepared.select(&trailing).is_err());
        assert!(constrained.select(&value).is_err());
        assert_eq!(prepared.select(&value).unwrap().unwrap(), expected);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
