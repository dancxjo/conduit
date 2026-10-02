//! Producer-side retention for one lived Body tutorial track.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

const STEPS: [(&str, &str, &str); 13] = [
    ("body.absent", "body-absent", "runtime-receipt"),
    ("bootstrap.started", "bootstrap-started", "runtime-receipt"),
    ("body.born", "body-born", "body-biography"),
    ("body.awake", "body-awake", "body-biography"),
    ("plot.used", "standing-plot-used", "runtime-receipt"),
    ("body.inspected", "body-inspected", "semantic-face"),
    ("workload.revised", "workload-revised", "body-biography"),
    ("host.added", "host-added", "runtime-receipt"),
    ("fault.observed", "fault-observed", "stream-disposition"),
    ("body.repaired", "body-repaired", "runtime-receipt"),
    ("body.long-running", "body-long-running", "runtime-receipt"),
    ("body.lulled", "body-lulled", "lifecycle-action"),
    ("body.fulfilled", "body-fulfilled", "fulfilled-transition"),
];

pub(crate) struct TrackIdentities {
    pub body: String,
    pub host: String,
    pub boot: String,
    pub peer_host: String,
    pub peer_boot: String,
    pub plan: String,
    pub distributed_plan: Option<String>,
    pub play: String,
    pub face: String,
    pub show: String,
    pub line: Option<String>,
    pub signs: BTreeMap<&'static str, String>,
}

pub(crate) struct TrackSource {
    pub commit: String,
    pub track_id: &'static str,
    pub embodiment: &'static str,
    pub mask_plot_id: &'static str,
    pub construction: Vec<ConstructionTruth>,
    pub identities: TrackIdentities,
    pub facts: Vec<Value>,
    /// Exact producer-owned outcomes from the shared ordered Mask journey.
    pub mask_actions: Value,
    /// Producer-owned descriptions of the concrete event that realized each public action.
    pub action_events: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
pub(crate) struct ConstructionTruth {
    pub host_id: String,
    pub profile: ConstructionStage,
    pub build: ConstructionStage,
    pub image: ConstructionStage,
}

#[derive(Serialize)]
#[serde(tag = "disposition", rename_all = "kebab-case")]
pub(crate) enum ConstructionStage {
    Exact { identity: String },
    Omitted { reason: String },
}

impl ConstructionStage {
    pub(crate) fn exact(identity: impl Into<String>) -> Self {
        Self::Exact {
            identity: identity.into(),
        }
    }

    pub(crate) fn omitted(reason: impl Into<String>) -> Self {
        Self::Omitted {
            reason: reason.into(),
        }
    }
}

#[derive(Serialize)]
struct Artifact<'a> {
    schema: &'static str,
    git_commit: &'a str,
    track: &'a str,
    step_id: &'a str,
    assertion: &'a str,
    source_facts: &'a Value,
}

pub(crate) fn write(mut source: TrackSource, output: &Path) -> Result<(), String> {
    if source.commit.len() != 40 || !source.commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Body track requires an exact commit".into());
    }
    let artifacts = output.join("artifacts");
    fs::create_dir_all(&artifacts)
        .map_err(|error| format!("create Body track artifacts: {error}"))?;
    if source.facts.len() != STEPS.len() {
        return Err("Body track facts must cover every retained detailed receipt".into());
    }
    if source.mask_actions.as_array().map(Vec::len)
        != Some(conduit_presentation::MASK_JOURNEY_ACTIONS.len())
    {
        return Err("Body track requires every producer-owned Mask action".into());
    }
    let mut receipts = Vec::with_capacity(STEPS.len());
    for (index, ((step_id, assertion, rung), facts)) in
        STEPS.iter().zip(source.facts.iter()).enumerate()
    {
        let file = format!("{:02}-{step_id}.json", index + 1);
        let relative = format!("artifacts/{file}");
        let bytes = serde_json::to_vec_pretty(&Artifact {
            schema: "conduit.evidence/semantic-step-receipt@2",
            git_commit: &source.commit,
            track: source.track_id,
            step_id,
            assertion,
            source_facts: facts,
        })
        .map_err(|error| format!("encode {step_id} receipt: {error}"))?;
        write_new(&artifacts.join(file), &bytes)?;
        let host_added = *step_id == "host.added";
        let plan = matches!(
            *step_id,
            "plot.used" | "workload.revised" | "host.added" | "body.repaired"
        )
        .then(|| {
            if host_added {
                source
                    .identities
                    .distributed_plan
                    .as_ref()
                    .unwrap_or(&source.identities.plan)
            } else {
                &source.identities.plan
            }
        });
        let play = matches!(
            *step_id,
            "plot.used" | "body.repaired" | "body.long-running"
        )
        .then_some(&source.identities.play);
        let sign = source.identities.signs.get(step_id);
        receipts.push(serde_json::json!({
            "step_id": step_id,
            "assertion": assertion,
            "disposition": "established",
            "provenance": {
                "body_id": (index >= 2).then_some(&source.identities.body),
                "host_id": if index < 3 { Some(&source.identities.host) } else if host_added { Some(&source.identities.peer_host) } else { None },
                "boot_id": if index < 3 { Some(&source.identities.boot) } else if host_added { Some(&source.identities.peer_boot) } else { None },
                "plan_id": plan,
                "play_id": play,
                "face_id": (*step_id == "body.inspected").then_some(&source.identities.face),
                "show_id": (*step_id == "body.inspected").then_some(&source.identities.show),
                "line_id": (host_added).then_some(source.identities.line.as_ref()).flatten(),
                "sign_id": sign,
            },
            "evidence": [{
                "artifact_id": format!("{}/{step_id}", source.track_id),
                "evidence_class": "semantic-receipt",
                "assertion_rung": rung,
                "documentary_description": format!("{} producer receipt for {step_id}.", source.embodiment),
                "path": relative,
                "sha256": format!("sha256:{:x}", Sha256::digest(&bytes)),
            }],
        }));
    }
    let mask_actions = source
        .mask_actions
        .as_array_mut()
        .expect("Mask action length was checked above");
    for outcome in mask_actions.iter_mut() {
        let face = outcome
            .as_object_mut()
            .and_then(|object| object.remove("presentation_id"))
            .ok_or("Body track Mask action lacks its exact Face identity")?;
        outcome["face_id"] = face;
    }
    for (index, (expected, outcome)) in conduit_presentation::MASK_JOURNEY_ACTIONS
        .iter()
        .zip(mask_actions.iter_mut())
        .enumerate()
    {
        if outcome["action_id"].as_str() != Some(expected.id()) {
            return Err(format!(
                "Body track Mask action {} is absent or reordered",
                expected.id()
            ));
        }
        let step_id = format!("mask.action-{index}");
        let assertion = format!("mask-{}", expected.id());
        let file = format!("{:02}-{step_id}.json", STEPS.len() + index + 1);
        let relative = format!("artifacts/{file}");
        let bytes = serde_json::to_vec_pretty(&Artifact {
            schema: "conduit.evidence/semantic-step-receipt@2",
            git_commit: &source.commit,
            track: source.track_id,
            step_id: &step_id,
            assertion: &assertion,
            source_facts: outcome,
        })
        .map_err(|error| format!("encode {step_id} receipt: {error}"))?;
        write_new(&artifacts.join(file), &bytes)?;
        receipts.push(serde_json::json!({
            "step_id": step_id,
            "assertion": assertion,
            "disposition": "established",
            "provenance": {
                "body_id": &source.identities.body,
                "host_id": null,
                "boot_id": null,
                "plan_id": outcome["plan_id"],
                "play_id": null,
                "face_id": outcome["face_id"],
                "show_id": outcome["show_id"],
                "line_id": null,
                "sign_id": null,
            },
            "evidence": [{
                "artifact_id": format!("{}/mask-action-{index}", source.track_id),
                "evidence_class": "semantic-receipt",
                "assertion_rung": "semantic-face",
                "documentary_description": format!("{} producer outcome for {}.", source.embodiment, expected.id()),
                "path": relative,
                "sha256": format!("sha256:{:x}", Sha256::digest(&bytes)),
            }],
        }));
        outcome["receipt_ids"] = serde_json::json!([format!("mask.action-{index}")]);
    }
    let action_receipts = [
        (
            "journey.bootstrap",
            &["body.absent", "bootstrap.started"][..],
        ),
        ("journey.birth", &["body.born", "body.awake"][..]),
        ("journey.useful-work", &["plot.used"][..]),
        (
            "journey.break-recover",
            &["fault.observed", "body.repaired"][..],
        ),
        (
            "journey.rest-finish",
            &["body.lulled", "body.fulfilled"][..],
        ),
    ];
    let mut actions = action_receipts
        .iter()
        .map(|(action_id, receipt_ids)| {
            let concrete_event = source
                .action_events
                .get(action_id)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("Body track lacks concrete event for {action_id}"))?;
            Ok(serde_json::json!({
                "action_id": action_id,
                "concrete_event": concrete_event,
                "receipt_ids": receipt_ids,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mask_public_actions = source
        .mask_actions
        .as_array()
        .expect("Mask action length was checked above")
        .iter()
        .map(|outcome| {
            serde_json::json!({
                "action_id": outcome["action_id"],
                "concrete_event": outcome["concrete_event"],
                "receipt_ids": outcome["receipt_ids"],
            })
        })
        .collect::<Vec<_>>();
    actions.splice(3..3, mask_public_actions);
    let mut hosts = vec![serde_json::json!({
        "host_id": source.identities.host,
        "boot_id": source.identities.boot,
    })];
    hosts.push(serde_json::json!({
        "host_id": source.identities.peer_host,
        "boot_id": source.identities.peer_boot,
    }));
    let document = serde_json::json!({
        "schema": "conduit.evidence/body-journey-track@6",
        "journey_id": "orifina/tutorial@1",
        "git_commit": source.commit,
        "track_id": source.track_id,
        "embodiment": source.embodiment,
        "body_id": source.identities.body,
        "mask_plot_id": source.mask_plot_id,
        "construction": source.construction,
        "hosts": hosts,
        "line_ids": source.identities.line.into_iter().collect::<Vec<_>>(),
        "distributed_plan_ids": source.identities.distributed_plan.into_iter().collect::<Vec<_>>(),
        "receipts": receipts,
        "actions": actions,
        "mask_actions": source.mask_actions,
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("encode Body track: {error}"))?;
    write_new(&output.join("track.json"), &bytes)
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("create {}: {error}", path.display()))
}

/// Retain documentary pixels from the same producer run beside its semantic receipts.
pub(crate) fn attach_screenshots(
    output: &Path,
    frames: &Path,
    captures: &[(&str, &str, &str)],
) -> Result<(), String> {
    let manifest = output.join("track.json");
    let mut track: Value = serde_json::from_slice(
        &fs::read(&manifest).map_err(|error| format!("read track: {error}"))?,
    )
    .map_err(|error| format!("decode track: {error}"))?;
    let track_id = track["track_id"]
        .as_str()
        .ok_or("missing track id")?
        .to_owned();
    let receipts = track["receipts"]
        .as_array_mut()
        .ok_or("missing track receipts")?;
    for &(step_id, frame, caption) in captures {
        let step = receipts
            .iter_mut()
            .find(|step| step["step_id"] == step_id)
            .ok_or_else(|| format!("unknown captured step {step_id}"))?;
        let bytes = fs::read(frames.join(format!("{frame}.png")))
            .map_err(|error| format!("read captured frame {frame}: {error}"))?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(format!("captured frame {frame} is not PNG"));
        }
        let relative = format!("artifacts/{step_id}.png");
        write_new(&output.join(&relative), &bytes)?;
        step["evidence"]
            .as_array_mut()
            .ok_or("missing evidence")?
            .push(serde_json::json!({
                "artifact_id": format!("{track_id}/{step_id}/screen"),
                "evidence_class": "screenshot",
                "assertion_rung": "deterministic-observation",
                "documentary_description": caption,
                "path": relative,
                "sha256": format!("sha256:{:x}", Sha256::digest(&bytes)),
            }));
    }
    let bytes = serde_json::to_vec_pretty(&track).map_err(|error| error.to_string())?;
    fs::write(manifest, bytes).map_err(|error| format!("retain screenshot references: {error}"))
}
