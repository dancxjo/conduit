//! Producer-side retention for one lived Body tutorial track.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const STEPS: [(&str, &str, &str); 13] = [
    ("body.absent", "body-absent", "runtime-receipt"),
    ("bootstrap.started", "bootstrap-started", "runtime-receipt"),
    ("body.born", "body-born", "body-biography"),
    ("body.awake", "body-awake", "body-biography"),
    ("form.used", "standing-form-used", "runtime-receipt"),
    ("body.inspected", "body-inspected", "semantic-presentation"),
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
    pub presentation: String,
    pub manifestation: String,
    pub line: Option<String>,
    pub signs: [Option<String>; 13],
}

pub(crate) struct TrackSource {
    pub commit: String,
    pub track_id: &'static str,
    pub embodiment: &'static str,
    pub presenter_id: &'static str,
    pub identities: TrackIdentities,
    pub facts: [Value; 13],
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

pub(crate) fn write(source: TrackSource, output: &Path) -> Result<(), String> {
    if source.commit.len() != 40 || !source.commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Body track requires an exact commit".into());
    }
    let artifacts = output.join("artifacts");
    fs::create_dir_all(&artifacts)
        .map_err(|error| format!("create Body track artifacts: {error}"))?;
    let mut steps = Vec::with_capacity(STEPS.len());
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
            "form.used" | "workload.revised" | "host.added" | "body.repaired"
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
            "form.used" | "body.repaired" | "body.long-running"
        )
        .then_some(&source.identities.play);
        let sign = source.identities.signs[index].as_ref();
        steps.push(serde_json::json!({
            "step_id": step_id,
            "assertion": assertion,
            "disposition": "established",
            "provenance": {
                "body_id": (index >= 2).then_some(&source.identities.body),
                "host_id": if index < 3 { Some(&source.identities.host) } else if host_added { Some(&source.identities.peer_host) } else { None },
                "boot_id": if index < 3 { Some(&source.identities.boot) } else if host_added { Some(&source.identities.peer_boot) } else { None },
                "plan_id": plan,
                "play_id": play,
                "presentation_id": (*step_id == "body.inspected").then_some(&source.identities.presentation),
                "manifestation_id": (*step_id == "body.inspected").then_some(&source.identities.manifestation),
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
    let mut hosts = vec![serde_json::json!({
        "host_id": source.identities.host,
        "boot_id": source.identities.boot,
    })];
    hosts.push(serde_json::json!({
        "host_id": source.identities.peer_host,
        "boot_id": source.identities.peer_boot,
    }));
    let document = serde_json::json!({
        "schema": "conduit.evidence/body-journey-track@2",
        "journey_id": "orifina/tutorial@1",
        "git_commit": source.commit,
        "track_id": source.track_id,
        "embodiment": source.embodiment,
        "body_id": source.identities.body,
        "presenter_id": source.presenter_id,
        "hosts": hosts,
        "line_ids": source.identities.line.into_iter().collect::<Vec<_>>(),
        "distributed_plan_ids": source.identities.distributed_plan.into_iter().collect::<Vec<_>>(),
        "steps": steps,
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("encode Body track: {error}"))?;
    write_new(&output.join("track.json"), &bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("create {}: {error}", path.display()))
}
