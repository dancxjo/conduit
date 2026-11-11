//! Preserve the retired Body checker's nested canonical validation coverage.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_typed_retention::{CommitOrder, PreparedTypedRetention};
fn accepts(back: &mut PreparedTypedRetention<'_>, bytes: &[u8]) -> bool {
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [None], None, 8);
    let result = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    assert_eq!(io.test_consumed(PortId(0)), result == StepOutcome::Progress);
    <PreparedTypedRetention<'_> as StepBack<1>>::step_committed(back);
    result == StepOutcome::Progress
}
#[test]
fn nested_record_retention_matches_core_validation_without_play_allocation() {
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
    let schema = ty.canonical_bytes().unwrap();
    let linked_schema = Box::leak(schema.clone().into_boxed_slice());
    let order = CommitOrder::prepare(8);
    let mut prepared =
        PreparedTypedRetention::prepare(&schema, value.len(), 4, order.clone()).unwrap();
    let mut linked =
        PreparedTypedRetention::prepare(linked_schema, value.len(), 4, order.clone()).unwrap();
    let mut constrained =
        PreparedTypedRetention::prepare(&schema, value.len() - 1, 4, order.clone()).unwrap();
    let original = PreparedStructuredValueValidator::new(&ty, value.len()).unwrap();
    let mut wrong_type = value.clone();
    wrong_type[5] ^= 1;
    let mut invalid_leaf = value.clone();
    *invalid_leaf.last_mut().unwrap() = 2;
    let mut trailing = value.clone();
    trailing.push(0);
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(
            accepts(&mut prepared, &value),
            original.validate(&value).is_ok()
        );
        assert_eq!(
            accepts(&mut linked, &value),
            original.validate(&value).is_ok()
        );
        for end in 0..value.len() {
            assert_eq!(
                accepts(&mut prepared, &value[..end]),
                original.validate(&value[..end]).is_ok()
            );
        }
        for invalid in [&wrong_type, &invalid_leaf, &trailing] {
            assert!(!accepts(&mut prepared, invalid));
            assert!(!accepts(&mut linked, invalid));
            assert!(original.validate(invalid).is_err());
        }
        assert!(!accepts(&mut constrained, &value));
        assert_eq!(prepared.snapshot().unwrap().canonical, value);
        assert_eq!(linked.snapshot().unwrap().canonical, value);
        assert_eq!(order.committed(), 2);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
