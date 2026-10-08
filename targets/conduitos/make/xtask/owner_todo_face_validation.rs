//! Independently bind the QMP guest's Face identity to current Todo meaning.

use std::{fs, process::Command};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{ConduitosError, LiveOwnerTodoFaceProofArgs, refusal};

pub(super) fn read_and_validate(
    args: &LiveOwnerTodoFaceProofArgs,
    guest_face: &Value,
) -> Result<Value, ConduitosError> {
    read_and_validate_current(args, guest_face, &args.expected_status, None)
}

pub(super) fn read_and_validate_current(
    args: &LiveOwnerTodoFaceProofArgs,
    guest_face: &Value,
    expected_status: &str,
    expected_action: Option<&str>,
) -> Result<Value, ConduitosError> {
    let bin = fs::canonicalize(&args.owner_conduit_bin)
        .map_err(|error| ConduitosError::refusal("native-todo-owner-bin", error.to_string()))?;
    let state = fs::canonicalize(&args.owner_state_dir)
        .map_err(|error| ConduitosError::refusal("native-todo-owner-state", error.to_string()))?;
    if !bin.is_file() || !state.is_dir() {
        return Err(refusal("native-todo-owner-entrance-invalid"));
    }
    let output = Command::new(&bin)
        .args(["body", "face", "--state-dir"])
        .arg(&state)
        .arg("--json")
        .output()
        .map_err(|error| ConduitosError::refusal("native-todo-owner-face", error.to_string()))?;
    if !output.status.success() || output.stdout.len() > 512 * 1024 {
        return Err(refusal("native-todo-owner-face-unavailable"));
    }
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| refusal("native-todo-owner-face-invalid"))?;
    let face = &document["presentation"];
    let items = face["subjects"]
        .as_array()
        .ok_or_else(|| refusal("native-todo-owner-face-invalid"))?
        .iter()
        .filter(|subject| {
            subject["identity"]
                .as_str()
                .is_some_and(|identity| identity.starts_with("todo/item/"))
        })
        .count();
    let has_list = face["subjects"].as_array().is_some_and(|subjects| {
        subjects
            .iter()
            .any(|subject| subject["identity"] == "todo/list")
    });
    let has_status = face["text"].as_array().is_some_and(|texts| {
        texts
            .iter()
            .any(|text| text["subject"] == "todo/status" && text["text"] == expected_status)
    });
    let action_available = expected_action.is_none_or(|identity| {
        face["actions"].as_array().is_some_and(|actions| {
            actions.iter().any(|action| {
                action["identity"] == identity && action["availability"] == "Available"
            })
        })
    });
    if document["schema"] != "conduit.body/local-face-snapshot@1"
        || face["basis"]["body_id"] != args.expected_body_id
        || face["identity"] != guest_face["face_id"]
        || face["revision"] != guest_face["face_revision"]
        || !has_list
        || !has_status
        || !action_available
        || items != args.expected_item_count
    {
        return Err(refusal("native-todo-owner-face-does-not-match-guest"));
    }
    Ok(json!({
        "source":"installed-owner-body-face-json",
        "body_id":args.expected_body_id,
        "face_id":face["identity"],
        "face_revision":face["revision"],
        "item_count":items,
        "status":expected_status,
        "expected_action_available":action_available,
        "snapshot_sha256":format!("sha256:{:x}", Sha256::digest(&output.stdout)),
    }))
}
