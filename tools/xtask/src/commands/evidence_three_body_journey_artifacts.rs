//! Root-confined, bounded documentary artifact verification.
use super::*;

pub(super) fn verify_artifacts(track: &BodyTrack, source: &Path) -> Result<(), String> {
    let root = source
        .parent()
        .ok_or("Body track manifest has no parent")?
        .canonicalize()
        .map_err(|error| format!("resolve Body track root: {error}"))?;
    let mut artifact_ids = BTreeSet::new();
    let mut artifact_paths = BTreeSet::new();
    for evidence in track.receipts.iter().flat_map(|step| &step.evidence) {
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
