use super::*;

fn records() -> Vec<Value> {
    let mut records = expected()
        .into_iter()
        .enumerate()
        .map(|(index, expected)| {
            let form = identity(expected.form).unwrap();
            serde_json::json!({
                "status": "quiescent-awaiting-input", "revision": index + 10,
                "body_id": "body", "wake_id": "wake", "plan_id": "plan", "active_play_id": "play",
                "source_document_id": form.source_document_id,
                "checked_form_id": form.checked_form_id,
                "expanded_form_id": form.expanded_form_id,
                "input_count": expected.count, "result": expected.result,
                "result_omitted_bytes": 0, "kernel_sign_gap": null
            })
        })
        .collect::<Vec<_>>();
    for status in ["stopped", "lulled"] {
        let mut record = records.last().unwrap().clone();
        record["status"] = status.into();
        records.push(record);
    }
    records
}

#[test]
fn two_form_proof_rejects_restarts_identity_substitution_input_loss_and_state_loss() {
    let valid = records();
    let (initial, proof) = validate(&valid).unwrap();
    assert_eq!(initial["result"], "HELLO");
    assert_eq!(proof.forms.len(), 2);
    assert_eq!(proof.switches, 6);
    for field in [
        "body_id",
        "wake_id",
        "plan_id",
        "active_play_id",
        "source_document_id",
        "checked_form_id",
        "expanded_form_id",
    ] {
        for index in [20, 37, 38] {
            let mut changed = valid.clone();
            changed[index][field] = "substituted".into();
            assert!(validate(&changed).is_err(), "{field} at {index}");
        }
    }
    for index in [18, 21, 26, 36] {
        let mut changed = valid.clone();
        changed[index]["result"] = "lost state".into();
        assert!(validate(&changed).is_err());
    }
    let mut lost = valid.clone();
    lost.remove(21); // Captured release after switching to Memory.
    assert!(validate(&lost).is_err());
    let mut duplicate = valid.clone();
    duplicate.insert(21, duplicate[21].clone());
    assert!(validate(&duplicate).is_err());
    let mut omitted = valid;
    omitted[27]["result"] = Value::Null; // Empty text is a value, not absent output.
    assert!(validate(&omitted).is_err());
}
