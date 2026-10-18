//! Birth is an observed owner transition, never inferred from later navigation.
use super::{
    refusal, validate_encounter, validate_source_interaction, ConduitosError, Face, Value,
};

pub(super) fn validate(
    births: &[Value],
    faces: &[Value],
    decoded: &[Face],
    actions: &[Value],
) -> Result<Value, ConduitosError> {
    let [birth] = births else {
        return Err(refusal(
            "single product boot requires exactly one enacted Birth receipt",
        ));
    };
    validate_transition(birth, actions)?;
    let index = faces
        .iter()
        .position(|record| {
            record["show"]["show_id"] == birth["show_id"]
                && record["show"]["face_id"] == birth["face_id"]
                && record["show"]["face_revision"] == birth["face_revision"]
        })
        .ok_or_else(|| refusal("Birth lacks retained source Face and acknowledged Show"))?;
    validate_encounter(&faces[index]["show"], &birth["before"])?;
    let face = &decoded[index];
    if face.basis.body_id.is_some()
        || face
            .subjects
            .iter()
            .any(|subject| subject.role == conduit_face::FaceRole::Body)
    {
        return Err(refusal("pre-Birth source Face claims an existing Body"));
    }
    validate_source_interaction(face, birth)?;
    Ok(birth.clone())
}

fn validate_transition(birth: &Value, actions: &[Value]) -> Result<(), ConduitosError> {
    if birth["schema"] != "conduit.native-action/v1"
        || birth["requested_intent"] != "conduit.intent/creche.birth@1"
        || birth["disposition"].get("Accepted").is_none()
        || birth["accepted_effects"] != 1
        || !birth["before"]["body_id"].is_null()
        || !birth["before"]["born_sign_id"].is_null()
        || birth["after"]["body_id"].as_str().is_none_or(str::is_empty)
        || birth["after"]["born_sign_id"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err(refusal(
            "Birth receipt lacks one actual zero-Body to born-Body effect",
        ));
    }
    for identity in ["host_id", "boot_id", "offer_generation"] {
        if birth["before"][identity].is_null()
            || birth["before"][identity] != birth["after"][identity]
        {
            return Err(refusal(
                "Birth changed its Host/Boot/offer encounter identity",
            ));
        }
    }
    for action in actions {
        for side in ["before", "after"] {
            for identity in ["body_id", "born_sign_id"] {
                if action[side][identity] != birth["after"][identity] {
                    return Err(refusal(
                        "navigation or lifecycle action replaced the original Birth identity",
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn later_navigation_cannot_substitute_another_birth() {
        let before = json!({"body_id":null,"born_sign_id":null,"host_id":"host/a","boot_id":"boot/a","offer_generation":1});
        let mut after = before.clone();
        after["body_id"] = json!("body/a");
        after["born_sign_id"] = json!("born/a");
        let birth = json!({"schema":"conduit.native-action/v1","requested_intent":"conduit.intent/creche.birth@1",
            "accepted_effects":1,"disposition":{"Accepted":{}},"before":before,"after":after});
        let mut action = json!({"before":after,"after":after});
        validate_transition(&birth, &[action.clone()]).unwrap();
        action["after"]["born_sign_id"] = json!("born/another");
        assert!(validate_transition(&birth, &[action]).is_err());
        let mut already_born = birth;
        already_born["before"]["body_id"] = json!("body/a");
        assert!(validate_transition(&already_born, &[]).is_err());
    }

    #[test]
    fn absent_or_duplicate_birth_receipts_are_refused() {
        assert!(validate(&[], &[], &[], &[]).is_err());
        assert!(validate(&[Value::Null, Value::Null], &[], &[], &[]).is_err());
    }
}
