use conduit_core::PlannedStateBoundary;
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
use common::fragment;

#[test]
fn legacy_fresh_state_deserializes_without_a_continuity_record() {
    let original = fragment().states.remove(0);
    let json = serde_json::to_value(&original).unwrap();
    assert!(json.get("retained").is_none());
    let decoded: PlannedStateBoundary = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, original);
}
