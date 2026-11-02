//! Authorization boundary only; actual durable owner effects have separate tests.
use super::super::member_leave::{authorize_document, SCHEMA};
use super::*;
#[test]
fn explicit_member_leave_requires_current_exact_grant_and_closed_deadline() {
    let grant = Grant::issue(&member(), MaskSelection::Native).unwrap();
    let document = serde_json::json!({"schema":SCHEMA,"token":grant.token,"request":request()});
    assert!(authorize_document(&grant, document.clone(), grant.expires_at_millis - 1).is_ok());
    assert!(authorize_document(&grant, document.clone(), grant.expires_at_millis).is_err());
    for field in [
        "schema",
        "credential_id",
        "body_id",
        "part_id",
        "host_id",
        "boot_id",
    ] {
        let mut changed = document.clone();
        changed["request"][field] = serde_json::json!("other/identity");
        assert!(
            authorize_document(&grant, changed, grant.expires_at_millis - 1).is_err(),
            "{field}"
        );
    }
    let mut changed = document.clone();
    changed["token"][0] = serde_json::json!(grant.token[0] ^ 1);
    assert!(authorize_document(&grant, changed, grant.expires_at_millis - 1).is_err());
    let mut changed = document.clone();
    changed["schema"] = serde_json::json!("other/message@1");
    assert!(authorize_document(&grant, changed, grant.expires_at_millis - 1).is_err());
    let mut changed = document.clone();
    changed["unexpected"] = serde_json::json!(true);
    assert!(authorize_document(&grant, changed, grant.expires_at_millis - 1).is_err());
    let replacement = grant.replacement(1).unwrap();
    assert!(authorize_document(&replacement, document, grant.expires_at_millis - 1).is_err());
}
