//! Reconstruct bounded pointer evidence from actual owner and physical records.
use super::*;
use serde_json::json;

pub(in super::super) fn receipt(serial: &str) -> Result<Value, ConduitosError> {
    let changes = face_correspondence::records(serial, CHANGE)?;
    if changes.len() != 4 {
        return Err(refusal(
            "Crèche journey must retain exactly two keyboard and two pointer edits",
        ));
    }
    for (index, change) in changes.iter().enumerate() {
        creche_choices::validate_change(
            serial,
            change,
            if index < 2 { "creche.plot.0" } else { ACTION },
            index % 2 == 1,
        )?;
    }
    let samples = face_correspondence::records(serial, POINTER)?;
    let mut previous = 0;
    for sample in &samples {
        physical(sample.clone())?;
        let sequence = sequence(sample)?;
        if sequence <= previous {
            return Err(refusal("physical pointer history is not ordered"));
        }
        previous = sequence;
    }
    let mut presses = Vec::new();
    let mut releases = Vec::new();
    for change in &changes[2..] {
        let matching: Vec<_> = samples
            .iter()
            .filter(|sample| {
                sample["primary_pressed"] == true
                    && sample["press_edge"] == true
                    && sample["face_id"] == change["before_face_id"]
                    && sample["face_revision"] == change["before_revision"]
                    && sample["show_id"] == change["before_show_id"]
            })
            .collect();
        if matching.len() != 1 {
            return Err(refusal(
                "each typed pointer edit needs exactly one current-Show press",
            ));
        }
        let press = matching[0];
        let release = samples
            .iter()
            .find(|sample| {
                sample["sequence"].as_u64() > press["sequence"].as_u64()
                    && sample["primary_pressed"] == false
                    && sample["press_edge"] == false
                    && sample["face_id"] == change["after_face_id"]
                    && sample["face_revision"] == change["after_revision"]
                    && sample["show_id"] == change["after_show_id"]
            })
            .ok_or_else(|| {
                refusal("pointer edit lacks an observed release under its fresh Show")
            })?;
        presses.push(press.clone());
        releases.push(release.clone());
    }
    let held = samples
        .iter()
        .find(|sample| {
            sample["sequence"].as_u64() > presses[0]["sequence"].as_u64()
                && sample["sequence"].as_u64() < releases[0]["sequence"].as_u64()
                && sample["primary_pressed"] == true
                && sample["press_edge"] == false
                && sample["face_id"] == changes[2]["after_face_id"]
                && sample["face_revision"] == changes[2]["after_revision"]
                && sample["show_id"] == changes[2]["after_show_id"]
                && (sample["delta_x"].as_i64().is_some_and(|n| n != 0)
                    || sample["delta_y"].as_i64().is_some_and(|n| n != 0))
        })
        .ok_or_else(|| refusal("missing held motion through the newly acknowledged Show"))?;
    if sequence(&presses[1])? <= sequence(&releases[0])? {
        return Err(refusal(
            "the second activation preceded release of the first gesture",
        ));
    }
    Ok(json!({
        "schema": "conduit.creche/native-pointer-proof@1", "status": "passed",
        "entrance": "normal-product-creche", "proof_class": "freestanding-emulator",
        "target_action": ACTION, "target_geometry": "observed-current-frame-hit-bounds",
        "keyboard_action_count": 2, "pointer_action_count": 2,
        "source_changes": changes, "presses": presses, "releases": releases,
        "held_motion_after_fresh_show": held, "held_motion_duplicate_effects": 0,
        "physical_evidence": false, "human_enactment": false,
    }))
}
