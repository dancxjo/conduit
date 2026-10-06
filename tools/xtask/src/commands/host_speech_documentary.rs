//! Retain the actual ordinary spoken Mask output for one current Presenter result.
use conduit_presentation::{GeneratedManifestationCandidate, Presentation};
use conduit_std_host::{hosted_speech_synthesis::EspeakDiscovery, spoken_mask_journey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) fn produce(
    output: &Path,
    step_id: &str,
    face: &Presentation,
    manifestation: &GeneratedManifestationCandidate,
    discovery: &EspeakDiscovery,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let stem = format!("{step_id}-spoken");
    let path = format!("artifacts/{stem}.wav");
    let retained = spoken_mask_journey::execute_retained_manifestation_mask_with_streaming_espeak(
        "documentary-spoken",
        &format!("documentary/{stem}"),
        face.clone(),
        manifestation.clone(),
        discovery.clone(),
        &output.join(&path),
        &conduit_tongues::specimen_language_request(),
    )?;
    let receipt_path = format!("artifacts/{stem}.json");
    let receipt = serde_json::to_vec_pretty(&json!({
        "schema": "conduit.documentary/runtime-speech@1",
        "presenter_candidate": manifestation.candidate_identity,
        "mechanism": "Retained current Presenter result through an ordinary streaming eSpeak speech Mask Plan and Play",
        "provider_sha256": discovery.provider_sha256,
        "plan": retained.execution.plan,
        "show": retained.execution.shown,
        "artifact": retained.artifact,
        "playback_observed": false,
        "human_hearing_observed": false,
    }))?;
    crate::commands::body_journey_track::write_new(&output.join(&receipt_path), &receipt)?;
    Ok(vec![
        json!({
            "artifact_id": format!("hosted-generative/{stem}/audio"),
            "evidence_class": "audio-source", "assertion_rung": "generated-show",
            "documentary_description": "Runtime-produced eSpeak speech from this retained Presenter result. Audio artifact; no speaker or human-listening claim.",
            "path": path, "sha256": format!("sha256:{}", retained.artifact.wav_sha256),
        }),
        json!({
            "artifact_id": format!("hosted-generative/{stem}/speech-receipt"),
            "evidence_class": "speech-receipt", "assertion_rung": "generated-show",
            "documentary_description": "Exact speech provider, Mask Plan, Play, acknowledged Show, and WAV identity.",
            "path": receipt_path, "sha256": format!("sha256:{:x}", Sha256::digest(&receipt)),
        }),
    ])
}

/// Several lifecycle observations refer to the same current Presenter request.
/// Copy its exact retained evidence, with distinct per-step paths required by
/// the track contract, without presenting another synthesis as having occurred.
pub(super) fn reuse(
    output: &Path,
    step_id: &str,
    original: &[Value],
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    use std::{fs, io::Read};
    let mut evidence = Vec::with_capacity(original.len());
    for item in original {
        let class = item["evidence_class"]
            .as_str()
            .ok_or("missing speech evidence class")?;
        let suffix = match class {
            "audio-source" => "wav",
            "speech-receipt" => "json",
            _ => return Err("unexpected speech evidence class".into()),
        };
        let original_path = item["path"]
            .as_str()
            .ok_or("missing original speech path")?;
        let expected = item["sha256"]
            .as_str()
            .ok_or("missing original speech digest")?;
        let source = output.join(original_path);
        if !fs::symlink_metadata(&source)?.file_type().is_file() {
            return Err("retained speech evidence is not a regular file".into());
        }
        let file = fs::File::open(source)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
            return Err("retained speech evidence violates its byte bound".into());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(metadata.len() + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != metadata.len()
            || format!("sha256:{:x}", Sha256::digest(&bytes)) != expected
        {
            return Err("retained speech evidence changed before reuse".into());
        }
        let path = format!("artifacts/{step_id}-spoken.{suffix}");
        crate::commands::body_journey_track::write_new(&output.join(&path), &bytes)?;
        evidence.push(json!({
            "artifact_id": format!("hosted-generative/{step_id}/{class}"),
            "evidence_class": class, "assertion_rung": "generated-show",
            "documentary_description": format!("Exact reused evidence from {original_path} for the same current Presenter request; no additional inference or synthesis. Original receipt metadata is preserved."),
            "path": path, "sha256": expected,
        }));
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    #[test]
    fn reused_audio_has_distinct_step_identity_and_exact_original_bytes() {
        let root = std::env::temp_dir().join(format!(
            "conduit-speech-reuse-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("artifacts")).unwrap();
        let bytes = b"RIFFfixture-only-audio-bytes";
        fs::write(root.join("artifacts/original.wav"), bytes).unwrap();
        let original = vec![
            json!({"path":"artifacts/original.wav", "artifact_id":"original", "evidence_class":"audio-source", "sha256":format!("sha256:{:x}",Sha256::digest(bytes))}),
        ];
        let copied = reuse(&root, "body.inspected", &original).unwrap();
        assert_ne!(copied[0]["path"], original[0]["path"]);
        assert_ne!(copied[0]["artifact_id"], original[0]["artifact_id"]);
        assert_eq!(copied[0]["sha256"], original[0]["sha256"]);
        assert_eq!(
            fs::read(root.join(copied[0]["path"].as_str().unwrap())).unwrap(),
            bytes
        );
        fs::write(root.join("artifacts/original.wav"), b"tampered").unwrap();
        assert!(reuse(&root, "body.repaired", &original).is_err());
        assert!(!root.join("artifacts/body.repaired-spoken.wav").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
