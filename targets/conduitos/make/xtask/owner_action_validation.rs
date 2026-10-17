//! Validate the current native owner action and acknowledged replacement Show.
use serde_json::Value;

use super::{refusal, ConduitosError};

pub(super) fn validate_success(
    before: &Value,
    before_ack: &Value,
    action: &Value,
    after: &Value,
    after_ack: &Value,
) -> Result<(), ConduitosError> {
    if action.get("status").and_then(Value::as_str) != Some("accepted")
        || action.get("face_refreshed").and_then(Value::as_bool) != Some(true)
        || action.get("requested_interval_ms").and_then(Value::as_u64) != Some(500)
        || !action
            .get("action_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("body/action/change-clock-interval/"))
        || after.get("status").and_then(Value::as_str) != Some("shown")
        || before_ack.get("status").and_then(Value::as_str) != Some("acknowledged")
        || after_ack.get("status").and_then(Value::as_str) != Some("acknowledged")
        || before_ack.get("show_id") != before.get("show_id")
        || after_ack.get("show_id") != after.get("show_id")
        || after.get("local_show_available").and_then(Value::as_bool) != Some(true)
        || after
            .get("owner_show_acknowledged")
            .and_then(Value::as_bool)
            != Some(false)
        || action.get("prior_show_id") != before.get("show_id")
        || action.get("face_id") != before.get("face_id")
        || action.get("face_revision") != before.get("face_revision")
        || before.get("show_id") == after.get("show_id")
        || before.get("face_id") == after.get("face_id")
    {
        return Err(refusal("native-owner-action-not-accepted-and-refreshed"));
    }
    Ok(())
}

pub(super) fn refreshed_show_after_action(
    action: &Value,
    faces: &[Value],
) -> Option<(Value, Value)> {
    let prior_face = action.get("face_id")?;
    let prior_revision = action.get("face_revision")?.as_u64()?;
    faces.iter().enumerate().find_map(|(index, after)| {
        if after.get("status")?.as_str()? != "shown"
            || after.get("interactions_admitted") != Some(&Value::Bool(true))
            || after.get("face_id") == Some(prior_face)
            || after.get("face_revision")?.as_u64()? <= prior_revision
        {
            return None;
        }
        let show_id = after.get("show_id")?;
        let ack = faces[index + 1..].iter().find(|face| {
            face.get("status").and_then(Value::as_str) == Some("acknowledged")
                && face.get("show_id") == Some(show_id)
        })?;
        Some((after.clone(), ack.clone()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn action_proof_rejects_stale_basis_and_unaccepted_return() {
        let before =
            json!({"status":"shown","show_id":"show/one","face_id":"face/one","face_revision":1});
        let accepted = json!({
            "status":"accepted","face_refreshed":true,"requested_interval_ms":500,
            "action_id":"body/action/change-clock-interval/1",
            "prior_show_id":"show/one","face_id":"face/one","face_revision":1,
        });
        let after = json!({"status":"shown","local_show_available":true,"owner_show_acknowledged":false,"show_id":"show/two","face_id":"face/two"});
        let before_ack = json!({"status":"acknowledged","show_id":"show/one"});
        let after_ack = json!({"status":"acknowledged","show_id":"show/two"});
        assert!(validate_success(&before, &before_ack, &accepted, &after, &after_ack).is_ok());
        let mut invented_owner_ack = after.clone();
        invented_owner_ack["owner_show_acknowledged"] = json!(true);
        assert!(validate_success(
            &before,
            &before_ack,
            &accepted,
            &invented_owner_ack,
            &after_ack
        )
        .is_err());
        let mut stale = accepted.clone();
        stale["prior_show_id"] = json!("show/older");
        assert!(validate_success(&before, &before_ack, &stale, &after, &after_ack).is_err());
        let mut refused = accepted.clone();
        refused["status"] = json!("refused");
        assert!(validate_success(&before, &before_ack, &refused, &after, &after_ack).is_err());
        let mut wrong_value = accepted;
        wrong_value["requested_interval_ms"] = json!(250);
        assert!(validate_success(&before, &before_ack, &wrong_value, &after, &after_ack).is_err());
    }

    #[test]
    fn action_proof_correlates_refreshed_show_after_standby() {
        let action = json!({"face_id":"face/one","face_revision":7});
        let mut faces = vec![
            json!({"status":"shown","face_id":"face/one","face_revision":7,"show_id":"show/standby","interactions_admitted":false}),
            json!({"status":"shown","face_id":"face/one","face_revision":7,"show_id":"show/active","interactions_admitted":true}),
            json!({"status":"acknowledged","show_id":"show/active"}),
            json!({"status":"shown","face_id":"face/two","face_revision":9,"show_id":"show/refreshed","interactions_admitted":true}),
        ];
        assert!(refreshed_show_after_action(&action, &faces).is_none());
        faces.push(json!({"status":"acknowledged","show_id":"show/active"}));
        assert!(refreshed_show_after_action(&action, &faces).is_none());
        faces.push(json!({"status":"acknowledged","show_id":"show/refreshed"}));
        let (after, ack) = refreshed_show_after_action(&action, &faces).unwrap();
        assert_eq!(after["show_id"], "show/refreshed");
        assert_eq!(ack["show_id"], after["show_id"]);
    }
}
