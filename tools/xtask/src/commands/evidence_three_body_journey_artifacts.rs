//! Root-confined, bounded documentary artifact verification.
use super::*;

pub(super) fn require_documentary(
    tracks: &[BodyTrack],
    recording: Option<&BodyTrack>,
) -> Result<(), String> {
    for track in tracks {
        if matches!(
            track.track_id.as_str(),
            "native-graphical" | "browser-graphical"
        ) {
            for step in &track.steps {
                if !step
                    .evidence
                    .iter()
                    .any(|item| item.evidence_class == "screenshot")
                {
                    return Err(format!(
                        "{} lacks a screenshot at {}",
                        track.track_id, step.step_id
                    ));
                }
            }
        } else if track.track_id == "hosted-generative" && recording.is_none() {
            return Err(
                "human-facing publication requires retained live conversational media".into(),
            );
        }
    }
    Ok(())
}

pub(super) fn read_recording(
    source: &Path,
    tracks: &[BodyTrack],
    sources: &[PathBuf],
) -> Result<BodyTrack, String> {
    let recording: BodyTrack = read_bounded_json(source)?;
    let position = tracks
        .iter()
        .position(|track| track.track_id == "hosted-generative")
        .ok_or("live recording requires a generative release track")?;
    let current = &tracks[position];
    if recording.track_id != "hosted-generative"
        || recording.embodiment != "hosted-open-weight-model-body"
        || recording.journey_id != current.journey_id
        || recording.body_id != current.body_id
        || !valid_commit(&recording.git_commit)
        || recording.steps.len() != current.steps.len()
    {
        return Err("live documentary has a different journey, Body or proof class".into());
    }
    verify_artifacts(&recording, source)?;
    let root = source.parent().ok_or("recording lacks parent")?;
    let current_root = sources[position]
        .parent()
        .ok_or("current track lacks parent")?;
    for (recorded, release) in recording.steps.iter().zip(&current.steps) {
        if recorded.step_id != release.step_id || recorded.assertion != release.assertion {
            return Err("live documentary step does not match release semantics".into());
        }
        if matches!(
            recorded.step_id.as_str(),
            "body.absent" | "bootstrap.started"
        ) {
            continue;
        }
        let original = recorded
            .evidence
            .iter()
            .find(|item| item.evidence_class == "presenter-receipt")
            .ok_or("live documentary lacks its original Presenter request")?;
        let observed = release
            .evidence
            .iter()
            .find(|item| item.evidence_class == "presenter-receipt")
            .ok_or("release must retain every documentary Presenter request")?;
        let original: serde_json::Value = read_bounded_json(&root.join(&original.path))?;
        let observed: serde_json::Value = read_bounded_json(&current_root.join(&observed.path))?;
        if original["proof_class"] != "live-local-model"
            || original["request"].is_null()
            || original["request"] != observed["request"]
        {
            return Err(format!(
                "{} live recording has stale Presenter inputs",
                recorded.step_id
            ));
        }
        for class in ["transcript", "audio", "waveform"] {
            if !recorded
                .evidence
                .iter()
                .any(|item| item.evidence_class == class)
            {
                return Err(format!("{} lacks documentary {class}", recorded.step_id));
            }
        }
    }
    Ok(recording)
}

pub(super) fn verify_artifacts(track: &BodyTrack, source: &Path) -> Result<(), String> {
    let root = source
        .parent()
        .ok_or("Body track manifest has no parent")?
        .canonicalize()
        .map_err(|error| format!("resolve Body track root: {error}"))?;
    let mut artifact_ids = BTreeSet::new();
    let mut artifact_paths = BTreeSet::new();
    for evidence in track.steps.iter().flat_map(|step| &step.evidence) {
        validate_relative_path(&evidence.path)?;
        if !artifact_ids.insert(evidence.artifact_id.as_str())
            || !artifact_paths.insert(&evidence.path)
            || !valid_identity(&evidence.artifact_id)
            || !valid_identity(&evidence.evidence_class)
            || !valid_narrative(&evidence.documentary_description)
            || !valid_sha256(&evidence.sha256)
        {
            return Err(format!(
                "{} has duplicate or invalid artifact evidence",
                track.track_id
            ));
        }
        let candidate = root.join(&evidence.path);
        let metadata = std::fs::symlink_metadata(&candidate)
            .map_err(|error| format!("inspect {}: {error}", evidence.path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!("{} artifact is not a regular file", track.track_id));
        }
        let resolved = candidate
            .canonicalize()
            .map_err(|error| format!("resolve {}: {error}", evidence.path.display()))?;
        if !resolved.starts_with(&root) {
            return Err(format!(
                "{} artifact escaped its track root",
                track.track_id
            ));
        }
        let limit = if matches!(
            evidence.evidence_class.as_str(),
            "screenshot" | "waveform" | "audio" | "audio-source" | "video"
        ) {
            MAXIMUM_MEDIA_BYTES
        } else {
            MAXIMUM_DOCUMENT_BYTES as u64
        };
        if metadata.len() == 0 || metadata.len() > limit {
            return Err(format!(
                "{} artifact violates its byte bound",
                track.track_id
            ));
        }
        let bytes = std::fs::read(&resolved)
            .map_err(|error| format!("read {}: {error}", evidence.path.display()))?;
        if bytes.is_empty() || bytes.len() as u64 > limit {
            return Err(format!(
                "{} artifact violates its byte bound",
                track.track_id
            ));
        }
        if format!("sha256:{:x}", Sha256::digest(&bytes)) != evidence.sha256 {
            return Err(format!("{} artifact digest changed", evidence.artifact_id));
        }
        let signature_ok = match evidence.evidence_class.as_str() {
            "screenshot" | "waveform" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "audio" => {
                bytes.starts_with(b"ID3")
                    || (bytes.first() == Some(&255)
                        && bytes.get(1).is_some_and(|value| value & 224 == 224))
            }
            "audio-source" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE"),
            "video" => bytes.starts_with(b"\x1a\x45\xdf\xa3") || bytes.get(4..8) == Some(b"ftyp"),
            "transcript" => std::str::from_utf8(&bytes)
                .is_ok_and(|text| text.chars().any(char::is_alphanumeric)),
            _ => true,
        };
        if !signature_ok {
            return Err(format!(
                "{} has invalid documentary media",
                evidence.artifact_id
            ));
        }
    }
    Ok(())
}
