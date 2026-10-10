//! Value-parameterized Source reaches the installed browser Plan and kernel.
use super::*;
const SOURCE: &str =
    include_str!("../../../../../proof/browser/fixtures/native-value-window.conduit");

#[test]
fn computed_native_shapes_execute_through_browser_plan_and_kernel() {
    let (mut session, effect) =
        TourSession::prepare("browser/native-value", "boot/native-value", SOURCE, 1).unwrap();
    let TourHostEffect::Manifestation(effect) = effect else {
        panic!("planned Boolean effect")
    };
    assert_eq!(effect.text.as_deref(), Some("true"));
    assert_eq!(effect.presentation_kind, "presentation/bool-value");
    assert_eq!(effect.plan_id, session.fragments[0].plan_id.as_str());
    let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
        panic!("completed receipt")
    };
    assert_eq!(receipt.disposition, "completed");
    assert_eq!(receipt.active_play_id, effect.active_play_id);
}

#[test]
fn wrong_native_window_payload_refuses_before_browser_play() {
    let wrong = SOURCE.replace("history: [[1,2,3],[4,5,6]]", "history: [[1,2,3]]");
    assert!(TourSession::prepare("browser/native-value", "boot/native-value", &wrong, 1).is_err());
}
