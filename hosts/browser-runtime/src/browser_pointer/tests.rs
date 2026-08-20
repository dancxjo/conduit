use super::*;

fn sample() -> NormalizedPointerSample {
    NormalizedPointerSample {
        position_x: 250_000,
        position_y: 750_000,
        delta_x: 125_000,
        delta_y: -250_000,
        primary_pressed: true,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 1,
        sequence: 0,
    }
}

#[test]
fn pointer_crosses_checked_plan_and_kernel_with_exact_identities() {
    let receipt = execute_browser_pointer(sample()).unwrap();
    assert_eq!(receipt.position_x, 250_000);
    assert_eq!(receipt.schema, "input/pointer-event@1");
    assert!(receipt.value_kind.starts_with("structured-info/profile-"));
    assert!(!receipt.plan_id.is_empty());
    assert!(!receipt.play_id.is_empty());
    assert!(!receipt.sign_id.is_empty());
    assert_ne!(
        receipt.source_placement_id,
        receipt.presentation_placement_id
    );
}

#[test]
fn malformed_normalized_browser_values_refuse_before_play() {
    let mut invalid = sample();
    invalid.position_x = 1_000_001;
    assert!(execute_browser_pointer(invalid).is_err());
    invalid = sample();
    invalid.queue_capacity = 0;
    assert!(execute_browser_pointer(invalid).is_err());
}
