//! Publication of retained documentary evidence, without fresh execution claims.
use super::*;

pub(crate) fn stage(
    publication_root: PathBuf,
    site_root: PathBuf,
    expected_git_commit: String,
) -> Result<(), String> {
    if !valid_commit(&expected_git_commit) {
        return Err("three-Body Journey staging requires an exact commit".into());
    }
    let index_path = publication_root.join("index.json");
    let index: ThreeBodyJourneyIndex = read_bounded_json(&index_path)?;
    if index.schema != INDEX_SCHEMA
        || index.disposition != "complete"
        || index.git_commit != expected_git_commit
    {
        return Err("three-Body Journey index is malformed or stale".into());
    }
    let contract = contract::canonical(&expected_git_commit);
    let projected_tracks = validate_index(&index, &contract)?;
    let mut tracks = Vec::with_capacity(projected_tracks.len());
    for projected in &projected_tracks {
        let track_path = publication_root
            .join(&projected.track_id)
            .join("track.json");
        let retained: BodyTrack = read_bounded_json(&track_path)?;
        if retained.actions != projected.actions
            || projected.receipts.iter().any(|receipt| {
                !retained.receipts.iter().any(|candidate| {
                    serde_json::to_value(candidate).ok() == serde_json::to_value(receipt).ok()
                })
            })
        {
            return Err(format!(
                "retained track '{}' diverges from its index",
                projected.track_id
            ));
        }
        verify_artifacts(&retained, &track_path)?;
        tracks.push(retained);
    }
    validate(&contract, &tracks, &expected_git_commit)?;
    if !publication_root.join("index.html").is_file() {
        return Err("three-Body Journey publication lacks index.html".into());
    }
    let journeys = site_root.join("journeys");
    let gallery_index = journeys.join("index.html");
    let gallery_json = journeys.join("gallery.json");
    if !gallery_index.is_file() || !gallery_json.is_file() {
        return Err("Pages root lacks a built journeys gallery".into());
    }
    let gallery: serde_json::Value = read_bounded_json(&gallery_json)?;
    if gallery
        .get("current_commit")
        .and_then(serde_json::Value::as_str)
        != Some(expected_git_commit.as_str())
    {
        return Err("journeys gallery belongs to a different commit".into());
    }
    let current = journeys.join("current/three-bodies");
    let historic = journeys
        .join("commits")
        .join(&expected_git_commit)
        .join("three-bodies");
    if current.exists() || historic.exists() {
        return Err("three-Body Journey staging refuses overwrite".into());
    }
    copy_publication(&publication_root, &current)?;
    copy_publication(&publication_root, &historic)?;
    let mut html = std::fs::read_to_string(&gallery_index)
        .map_err(|error| format!("read journeys gallery entrance: {error}"))?;
    let start_marker = "<!-- conduit-three-body-flagship@2 -->";
    let end_marker = "<!-- conduit-three-body-flagship:end -->";
    let insertion = r#"<!-- conduit-three-body-flagship@2 --><section class="flagship admitted" aria-labelledby="flagship-title"><h2 id="flagship-title">Start, use, inspect, and stop a body</h2><p>Follow recorded steps across three independent bodies, with the original receipts alongside each step.</p><p><a class="primary" href="current/three-bodies/">Explore the recorded journey</a></p></section><!-- conduit-three-body-flagship:end -->"#;
    let start = html
        .find(start_marker)
        .ok_or("journeys gallery entrance lacks its flagship marker")?;
    let end = html[start..]
        .find(end_marker)
        .map(|offset| start + offset + end_marker.len())
        .ok_or("journeys gallery entrance lacks its flagship end marker")?;
    html.replace_range(start..end, insertion);
    let history_marker = format!("<li><code>{expected_git_commit}</code>");
    let history_position = html
        .find(&history_marker)
        .and_then(|start| html[start..].find("</li>").map(|offset| start + offset))
        .ok_or("journeys gallery entrance lacks exact-commit history")?;
    html.insert_str(
        history_position,
        &format!(" · <a href=\"commits/{expected_git_commit}/three-bodies/\">Three Bodies</a>"),
    );
    std::fs::write(&gallery_index, html)
        .map_err(|error| format!("write journeys gallery entrance: {error}"))?;
    println!("STAGED three-Body Journey for {expected_git_commit}");
    Ok(())
}

/// This is a presentation refresh, not admission under today's journey contract.
/// The retained source, index, track manifests, and artifact bytes stay unchanged.
pub(crate) fn render_recorded(root: PathBuf) -> Result<(), String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let index_path = confined_file(&root, Path::new("index.json"))?;
    let mut document: serde_json::Value = read_bounded_json(&index_path)?;
    normalize_recorded_fields(&mut document)?;
    let index: ThreeBodyJourneyIndex =
        serde_json::from_value(document).map_err(|error| error.to_string())?;
    if index.schema != INDEX_SCHEMA
        || index.disposition != "complete"
        || !valid_commit(&index.git_commit)
        || index.actions.is_empty()
        || index.actions.len() > MAXIMUM_STEPS
    {
        return Err("recorded three-body index is incomplete or malformed".into());
    }
    for action in &index.actions {
        if action.bodies.len() != REQUIRED_TRACKS {
            return Err("recorded action must retain three bodies".into());
        }
        let mut tracks = BTreeSet::new();
        for body in &action.bodies {
            let track = Path::new(&body.track_id);
            validate_relative_path(track)?;
            if track.components().count() != 1 || !tracks.insert(&body.track_id) {
                return Err("recorded action has invalid or duplicate track identities".into());
            }
            if body.receipts.is_empty() {
                return Err("recorded body has no receipts".into());
            }
            for receipt in &body.receipts {
                if receipt.evidence.is_empty() {
                    return Err("recorded receipt has no evidence".into());
                }
                for evidence in &receipt.evidence {
                    verify_recorded_artifact(&root, track, evidence)?;
                }
            }
        }
    }
    let output = root.join("index.html");
    if output.exists() || output.is_symlink() {
        confined_file(&root, Path::new("index.html"))?;
    }
    std::fs::write(output, page::render(&index)).map_err(|error| error.to_string())?;
    println!(
        "Refreshed recorded journey presentation; retained source {}",
        index.git_commit
    );
    Ok(())
}

fn normalize_recorded_fields(document: &mut serde_json::Value) -> Result<(), String> {
    match document {
        serde_json::Value::Object(fields) => {
            for (old, current) in [
                ("mask_form_id", "mask_plot_id"),
                ("selected_mask_form_id", "selected_mask_plot_id"),
            ] {
                if fields.contains_key(old) && fields.contains_key(current) {
                    return Err(format!(
                        "recorded document contains both {old} and {current}"
                    ));
                }
                if let Some(value) = fields.remove(old) {
                    fields.insert(current.into(), value);
                }
            }
            for value in fields.values_mut() {
                normalize_recorded_fields(value)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_recorded_fields(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn confined_file(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    validate_relative_path(relative)?;
    let mut candidate = root.to_path_buf();
    for component in relative.components() {
        candidate.push(component);
        let metadata = std::fs::symlink_metadata(&candidate).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("recorded evidence must not contain symlinks".into());
        }
    }
    if !candidate.is_file() {
        return Err("recorded evidence must be a regular file".into());
    }
    Ok(candidate)
}

fn verify_recorded_artifact(
    root: &Path,
    track: &Path,
    evidence: &StepEvidence,
) -> Result<(), String> {
    validate_relative_path(&evidence.path)?;
    if !valid_sha256(&evidence.sha256) {
        return Err("recorded artifact has an invalid digest".into());
    }
    let file = confined_file(root, &track.join(&evidence.path))?;
    let length = std::fs::metadata(&file)
        .map_err(|error| error.to_string())?
        .len();
    if length == 0 || length > MAXIMUM_MEDIA_BYTES {
        return Err("recorded artifact violates its byte bound".into());
    }
    let bytes = std::fs::read(file).map_err(|error| error.to_string())?;
    if format!("sha256:{:x}", Sha256::digest(&bytes)) != evidence.sha256 {
        return Err(format!(
            "recorded artifact digest changed: {}",
            evidence.artifact_id
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_fields_are_translated_only_in_memory_and_ambiguity_is_refused() {
        let mut value = serde_json::json!({"cells": [{"mask_form_id": "mask-1",
            "selected_mask_form_id": null}], "other_form_id": "untouched"});
        normalize_recorded_fields(&mut value).unwrap();
        assert_eq!(value["cells"][0]["mask_plot_id"], "mask-1");
        assert!(value["cells"][0].get("selected_mask_plot_id").is_some());
        assert_eq!(value["other_form_id"], "untouched");
        let mut ambiguous = serde_json::json!({"mask_form_id": "old", "mask_plot_id": "new"});
        assert!(normalize_recorded_fields(&mut ambiguous).is_err());
    }

    #[test]
    fn changed_recorded_artifact_is_refused_before_presentation_replacement() {
        let root = std::env::temp_dir().join(format!(
            "conduit-recorded-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("body")).unwrap();
        std::fs::write(root.join("body/receipt.json"), "original").unwrap();
        let evidence = StepEvidence {
            artifact_id: "receipt-1".into(),
            evidence_class: "semantic-receipt".into(),
            assertion_rung: EvidenceRung::RuntimeReceipt,
            documentary_description: "Retained receipt".into(),
            path: "receipt.json".into(),
            sha256: format!("sha256:{:x}", Sha256::digest(b"original")),
        };
        verify_recorded_artifact(&root, Path::new("body"), &evidence).unwrap();
        std::fs::write(root.join("body/receipt.json"), "tampered").unwrap();
        assert!(
            verify_recorded_artifact(&root, Path::new("body"), &evidence)
                .unwrap_err()
                .contains("digest changed")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refresh_preserves_recorded_inputs_and_refuses_tampering_without_replacing_html() {
        let root = std::env::temp_dir().join(format!(
            "conduit-recorded-render-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = "a".repeat(40);
        let action = contract::canonical(&source).actions.remove(0);
        let bodies = (0..3).map(|n| {
            let track = format!("track-{n}");
            std::fs::create_dir(root.join(&track)).unwrap();
            std::fs::write(root.join(&track).join("receipt.json"), "original").unwrap();
            std::fs::write(root.join(&track).join("track.json"), "retained manifest bytes").unwrap();
            serde_json::json!({
                "track_id":track,"embodiment":"recorded","body_id":format!("body-{n}"),
                "mask_form_id":"legacy-mask","construction":[],"hosts":[],"line_ids":[],"distributed_plan_ids":[],
                "observed":{"action_id":action.action_id,"concrete_event":"Recorded action","receipt_ids":["receipt"]},
                "receipts":[{"step_id":"receipt","assertion":"retained","disposition":"established","provenance":{},
                    "evidence":[{"artifact_id":"receipt","evidence_class":"semantic-receipt","assertion_rung":"runtime-receipt",
                        "documentary_description":"Retained record","path":"receipt.json","sha256":format!("sha256:{:x}",Sha256::digest(b"original"))}]}]
            })
        }).collect::<Vec<_>>();
        let document = serde_json::to_vec(&serde_json::json!({"schema":INDEX_SCHEMA,"disposition":"complete",
            "journey_id":"recorded","git_commit":source,"actions":[{"action":action,"bodies":bodies}]})).unwrap();
        std::fs::write(root.join("index.json"), &document).unwrap();
        render_recorded(root.clone()).unwrap();
        let rendered = std::fs::read(root.join("index.html")).unwrap();
        assert!(String::from_utf8_lossy(&rendered).contains(&source));
        assert_eq!(std::fs::read(root.join("index.json")).unwrap(), document);
        assert_eq!(
            std::fs::read_to_string(root.join("track-0/track.json")).unwrap(),
            "retained manifest bytes"
        );
        std::fs::write(root.join("track-2/receipt.json"), "tampered").unwrap();
        assert!(render_recorded(root.clone())
            .unwrap_err()
            .contains("digest changed"));
        assert_eq!(std::fs::read(root.join("index.html")).unwrap(), rendered);
        assert_eq!(std::fs::read(root.join("index.json")).unwrap(), document);
        std::fs::remove_dir_all(root).unwrap();
    }
}
