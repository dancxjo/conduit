//! Receipt chronology fixtures exercise admission evidence, not owner execution.
use super::*;

fn face(show: &str) -> String {
    format!("{FACE_PREFIX}{}\n", json!({"show":{"show_id":show}}))
}

fn action(prefix: &str, show: &str, accepted: bool) -> String {
    let disposition = if accepted {
        json!({"Accepted":{}})
    } else {
        json!({"Refused":{"reason":"UnavailableAction"}})
    };
    format!(
        "{prefix}{}\n",
        json!({"show_id":show,"disposition":disposition})
    )
}

#[test]
fn superseded_show_cannot_authorize_an_action() {
    let serial = face("old") + &face("new") + &action(ACTION_PREFIX, "old", true);
    assert!(validate_receipt_order(&serial).is_err());
}

#[test]
fn accepted_show_cannot_authorize_a_second_effect() {
    let serial =
        face("once") + &action(ACTION_PREFIX, "once", true) + &action(ACTION_PREFIX, "once", true);
    assert!(validate_receipt_order(&serial).is_err());
}

#[test]
fn refusal_requires_fresh_show_before_recovery() {
    let refused = face("refused") + &action(ACTION_PREFIX, "refused", false);
    assert!(
        validate_receipt_order(&(refused.clone() + &action(ACTION_PREFIX, "refused", true)))
            .is_err()
    );
    assert!(validate_receipt_order(
        &(refused + &face("recovery") + &action(ACTION_PREFIX, "recovery", true))
    )
    .is_ok());
}

#[test]
fn republishing_a_consumed_show_does_not_revive_it() {
    let serial = face("once")
        + &action(ACTION_PREFIX, "once", true)
        + &face("once")
        + &action(ACTION_PREFIX, "once", true);
    assert!(validate_receipt_order(&serial).is_err());
}

#[test]
fn birth_uses_its_prior_show_and_requires_fresh_show_for_lifecycle() {
    let birth = face("birth") + &action(BIRTH_PREFIX, "birth", true);
    assert!(validate_receipt_order(&birth).is_ok());
    assert!(
        validate_receipt_order(&(birth.clone() + &action(ACTION_PREFIX, "birth", true))).is_err()
    );
    assert!(validate_receipt_order(
        &(birth + &face("body") + &action(ACTION_PREFIX, "body", true))
    )
    .is_ok());
}
