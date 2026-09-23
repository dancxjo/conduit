//! Strict Home front receipt derived from one completed ordinary QEMU journey.

use std::{fs, path::Path};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::ConduitosError;

#[derive(Serialize)]
struct HomeFrontReceipt {
    schema: &'static str,
    front_id: &'static str,
    proof_class: &'static str,
    step_ids: [&'static str; 8],
    host_id: String,
    boot_id: String,
    plan_id: String,
    active_play_id: String,
    renderer_id: String,
    manifestation_id: String,
    artifact_path: &'static str,
    artifact_sha256: String,
}

pub(super) fn retain(target: &Path) -> Result<(), ConduitosError> {
    let visual_path = target.join("journey-frames/manifest.json");
    let visual: Value = serde_json::from_slice(&fs::read(&visual_path).map_err(io_error)?)
        .map_err(|error| refusal("home-front-visual-invalid", error.to_string()))?;
    if visual["status"] != "complete" || visual["proof_class"] != "freestanding-emulator" {
        return Err(refusal(
            "home-front-visual-incomplete",
            "the ordinary QEMU journey is not complete",
        ));
    }
    let checkpoints = visual["checkpoints"]
        .as_array()
        .ok_or_else(|| refusal("home-front-checkpoints-missing", "checkpoints"))?;
    let play = checkpoint(checkpoints, "home-play-observed")?;
    for (checkpoint_name, expected) in [
        (
            "body-awake",
            &[conduit_home_model::HOME_ARRIVED_STEP_ID][..],
        ),
        (
            "home-forms",
            &[
                conduit_home_model::FORMS_OPENED_STEP_ID,
                conduit_home_model::FORM_SELECTED_STEP_ID,
            ][..],
        ),
        (
            "home-prompt",
            &[conduit_home_model::PROMPT_OPENED_STEP_ID][..],
        ),
        (
            "home-play-observed",
            &[
                conduit_home_model::FORM_RUN_STEP_ID,
                conduit_home_model::PLAY_OBSERVED_STEP_ID,
            ][..],
        ),
        (
            "home-patchbay-open",
            &[conduit_home_model::PATCHBAY_OPENED_STEP_ID][..],
        ),
        (
            "home-returned",
            &[conduit_home_model::HOME_RETURNED_STEP_ID][..],
        ),
    ] {
        let actual = checkpoint(checkpoints, checkpoint_name)?["journey_step_ids"]
            .as_array()
            .ok_or_else(|| refusal("home-front-step-correlation-missing", checkpoint_name))?;
        if actual
            .iter()
            .filter_map(Value::as_str)
            .ne(expected.iter().copied())
        {
            return Err(refusal(
                "home-front-step-correlation-invalid",
                checkpoint_name,
            ));
        }
    }

    let record = &play["guest_record"];
    let frame = &play["frame"];
    let frame_name = text(frame, "png")?;
    let source = target.join("journey-frames").join(&frame_name);
    let artifact = fs::read(&source).map_err(io_error)?;
    let digest = format!("sha256:{:x}", Sha256::digest(&artifact));
    if frame["png_sha256"].as_str() != Some(&digest[7..]) {
        return Err(refusal(
            "home-front-artifact-digest-invalid",
            "visual manifest and retained PNG differ",
        ));
    }
    let output = target.join("home-front-conduitos");
    // This is a derived proof directory. Re-running the supported entrance for
    // the same exact journey replaces its two named outputs without requiring
    // destructive cleanup of the surrounding target tree.
    fs::create_dir_all(&output).map_err(io_error)?;
    fs::write(output.join("home-conduitos.png"), &artifact).map_err(io_error)?;
    let receipt = HomeFrontReceipt {
        schema: "conduit.evidence/home-front@1",
        front_id: "conduitos",
        proof_class: "freestanding-emulator",
        step_ids: conduit_home_model::JOURNEY_STEP_IDS,
        host_id: text(record, "host_id")?,
        boot_id: text(record, "boot_id")?,
        plan_id: text(record, "plan_id")?,
        active_play_id: text(record, "active_play_id")?,
        renderer_id: text(record, "presenter_implementation_id")?,
        manifestation_id: text(record, "manifestation_id")?,
        artifact_path: "home-conduitos.png",
        artifact_sha256: digest,
    };
    fs::write(
        output.join("home-front-conduitos.json"),
        serde_json::to_vec_pretty(&receipt)
            .map_err(|error| refusal("home-front-receipt-invalid", error.to_string()))?,
    )
    .map_err(io_error)
}

fn checkpoint<'a>(checkpoints: &'a [Value], name: &str) -> Result<&'a Value, ConduitosError> {
    checkpoints
        .iter()
        .find(|entry| entry["checkpoint"].as_str() == Some(name))
        .ok_or_else(|| refusal("home-front-checkpoint-missing", name))
}

fn text(value: &Value, field: &str) -> Result<String, ConduitosError> {
    value[field]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| refusal("home-front-identity-missing", field))
}

fn io_error(error: std::io::Error) -> ConduitosError {
    refusal("home-front-evidence-io", error.to_string())
}

fn refusal(reason: &'static str, detail: impl ToString) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retains_only_a_complete_correlated_home_front() {
        let root = std::env::temp_dir().join(format!(
            "conduit-home-front-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let frames = root.join("journey-frames");
        fs::create_dir_all(&frames).unwrap();
        let artifact = b"accepted PNG fixture";
        fs::write(frames.join("home-play-observed.png"), artifact).unwrap();
        let digest = format!("{:x}", Sha256::digest(artifact));
        let mut checkpoints = Vec::new();
        for (name, steps) in [
            ("body-awake", vec![conduit_home_model::HOME_ARRIVED_STEP_ID]),
            (
                "home-forms",
                vec![
                    conduit_home_model::FORMS_OPENED_STEP_ID,
                    conduit_home_model::FORM_SELECTED_STEP_ID,
                ],
            ),
            (
                "home-prompt",
                vec![conduit_home_model::PROMPT_OPENED_STEP_ID],
            ),
            (
                "home-play-observed",
                vec![
                    conduit_home_model::FORM_RUN_STEP_ID,
                    conduit_home_model::PLAY_OBSERVED_STEP_ID,
                ],
            ),
            (
                "home-patchbay-open",
                vec![conduit_home_model::PATCHBAY_OPENED_STEP_ID],
            ),
            (
                "home-returned",
                vec![conduit_home_model::HOME_RETURNED_STEP_ID],
            ),
        ] {
            checkpoints.push(json!({
                "checkpoint": name,
                "journey_step_ids": steps,
                "guest_record": if name == "home-play-observed" { json!({
                    "host_id":"host/conduitos", "boot_id":"boot/conduitos",
                    "plan_id":"plan/conduitos", "active_play_id":"play/conduitos",
                    "presenter_implementation_id":"presenter/native-graphical@1",
                    "manifestation_id":"manifestation/conduitos"
                }) } else { Value::Null },
                "frame": if name == "home-play-observed" { json!({
                    "png":"home-play-observed.png", "png_sha256":digest
                }) } else { Value::Null }
            }));
        }
        fs::write(
            frames.join("manifest.json"),
            serde_json::to_vec(&json!({
                "status":"complete", "proof_class":"freestanding-emulator",
                "checkpoints":checkpoints
            }))
            .unwrap(),
        )
        .unwrap();

        retain(&root).unwrap();
        let receipt: Value = serde_json::from_slice(
            &fs::read(root.join("home-front-conduitos/home-front-conduitos.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            receipt["step_ids"],
            json!(conduit_home_model::JOURNEY_STEP_IDS)
        );
        assert_eq!(receipt["artifact_sha256"], format!("sha256:{digest}"));
        assert!(root
            .join("home-front-conduitos/home-conduitos.png")
            .is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
