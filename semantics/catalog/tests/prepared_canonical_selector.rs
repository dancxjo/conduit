//! Shared selection has no dependency on a thermometer, device or Body.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

use conduit_core::{
    KindId, StructuredInfoType, StructuredInfoValue, StructuredSelector, StructuredSelectorRefusal,
    StructuredVariantCase, UnmatchedVariantDisposition,
};
use conduit_semantic_catalog::PreparedCanonicalSelector;

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from("value/count")).unwrap()
}
fn count(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(count_type(), value.to_le_bytes().to_vec()).unwrap()
}
fn event_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        "music/control-event@1".into(),
        vec![
            StructuredVariantCase::new("pitch", count_type()).unwrap(),
            StructuredVariantCase::new("velocity", count_type()).unwrap(),
        ],
    )
    .unwrap()
}
fn event(tag: &str, value: u64) -> Vec<u8> {
    StructuredInfoValue::variant(event_type(), tag, count(value))
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn selector(disposition: UnmatchedVariantDisposition) -> StructuredSelector {
    StructuredSelector::variant(event_type(), "pitch", disposition).unwrap()
}

#[test]
fn shared_music_selector_preserves_value_drop_and_refusal_without_allocation() {
    let pitch = event("pitch", 62);
    let velocity = event("velocity", 96);
    let expected = count(62).canonical_bytes().unwrap();
    let mut dropping = PreparedCanonicalSelector::prepare(
        selector(UnmatchedVariantDisposition::Drop),
        512,
        expected.len(),
    )
    .unwrap();
    let mut refusing = PreparedCanonicalSelector::prepare(
        selector(UnmatchedVariantDisposition::Refuse),
        512,
        expected.len(),
    )
    .unwrap();
    let ((), allocations) = allocation_probe::observe(|| {
        assert_eq!(dropping.select(&pitch).unwrap(), Some(expected.as_slice()));
        assert_eq!(dropping.select(&velocity).unwrap(), None);
        assert_eq!(
            refusing.select(&velocity),
            Err(StructuredSelectorRefusal::UnmatchedVariant)
        );
        assert_eq!(refusing.select(&pitch).unwrap(), Some(expected.as_slice()));
    });
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
}

#[test]
fn wrong_type_torn_input_and_output_pressure_refuse_without_growth() {
    let pitch = event("pitch", 62);
    let wrong = count(62).canonical_bytes().unwrap();
    let mut full = PreparedCanonicalSelector::prepare(
        selector(UnmatchedVariantDisposition::Drop),
        512,
        wrong.len(),
    )
    .unwrap();
    let mut small = PreparedCanonicalSelector::prepare(
        selector(UnmatchedVariantDisposition::Drop),
        512,
        wrong.len() - 1,
    )
    .unwrap();
    let ((), allocations) = allocation_probe::observe(|| {
        assert_eq!(
            full.select(&wrong),
            Err(StructuredSelectorRefusal::MalformedCheckedValue)
        );
        assert_eq!(
            full.select(&pitch[..pitch.len() - 1]),
            Err(StructuredSelectorRefusal::MalformedCheckedValue)
        );
        assert_eq!(
            small.select(&pitch),
            Err(StructuredSelectorRefusal::CanonicalEncodingTooLarge)
        );
        assert_eq!(full.select(&pitch).unwrap(), Some(wrong.as_slice()));
    });
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
}
