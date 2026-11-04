//! Strict response classification below the pinned owner transport. No Host
//! admission or physical output is inferred from these protocol documents.
use super::*;

#[test]
fn only_explicit_stale_refusal_requests_a_fresh_presentation() {
    let response = |accepted, code| {
        json!({
            "schema":"conduit.body/native-owner-show-ack-response@1", "accepted":accepted, "code":code
        })
    };
    assert_eq!(
        decode_show_acceptance(&response(true, "accepted")).unwrap(),
        ShowAcceptance::Accepted
    );
    assert_eq!(
        decode_show_acceptance(&response(false, "stale-face")).unwrap(),
        ShowAcceptance::Stale
    );
    for code in [
        "show-refused",
        "show-grant-or-basis-invalid",
        "show-frame-invalid",
        "expired",
        "stale-face-other",
        "accepted",
    ] {
        assert!(
            decode_show_acceptance(&response(false, code)).is_err(),
            "{code}"
        );
    }
    assert!(decode_show_acceptance(&response(true, "stale-face")).is_err());
    let mut wrong = response(false, "stale-face");
    wrong["schema"] = json!("other/response");
    assert!(decode_show_acceptance(&wrong).is_err());
    wrong = response(false, "stale-face");
    wrong["accepted"] = json!("false");
    assert!(decode_show_acceptance(&wrong).is_err());
}
