//! These synthetic bytes test the publication gate only; they are never #4807 run evidence.

use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{digest, render_one_body_journey, OneBodyJourneyRequest, CHAPTERS};
use crate::evidence::{
    EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    output: PathBuf,
    commit: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn add(
    root: &Path,
    manifest: &mut EvidenceManifest,
    id: &str,
    kind: EvidenceKind,
    media_type: &str,
    bytes: &[u8],
) -> String {
    let extension = match kind {
        EvidenceKind::Screenshot => "png",
        EvidenceKind::Audio => "wav",
        EvidenceKind::ConsoleTranscript => "txt",
        EvidenceKind::MachineReadableManifest => "json",
    };
    let path = format!("{id}.{extension}");
    fs::write(root.join(&path), bytes).unwrap();
    manifest
        .declare(EvidenceOutput {
            id: id.into(),
            kind,
            path: path.into(),
            media_type: media_type.into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "test-run".into(),
                ..Default::default()
            },
        })
        .unwrap();
    digest(bytes)
}

fn json_output(root: &Path, manifest: &mut EvidenceManifest, id: &str, value: &Value) -> String {
    add(
        root,
        manifest,
        id,
        EvidenceKind::MachineReadableManifest,
        "application/json",
        &serde_json::to_vec(value).unwrap(),
    )
}

fn fixture(
    mixed_run: bool,
    stale_transcript: bool,
    omit_chapter: bool,
    repeat_action: bool,
) -> Fixture {
    fixture_with_media(
        mixed_run,
        stale_transcript,
        omit_chapter,
        repeat_action,
        false,
        false,
    )
}

fn fixture_with_media(
    mixed_run: bool,
    stale_transcript: bool,
    omit_chapter: bool,
    repeat_action: bool,
    invalid_png: bool,
    silent_wav: bool,
) -> Fixture {
    fixture_with_source_gap(
        mixed_run,
        stale_transcript,
        omit_chapter,
        repeat_action,
        invalid_png,
        silent_wav,
        false,
        false,
        false,
    )
}

#[allow(clippy::too_many_arguments)] // Independent malformed-evidence switches keep each case explicit.
fn fixture_with_source_gap(
    mixed_run: bool,
    stale_transcript: bool,
    omit_chapter: bool,
    repeat_action: bool,
    invalid_png: bool,
    silent_wav: bool,
    omit_terminal_show: bool,
    omit_audio_delivery: bool,
    guest_audio: bool,
) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "conduit-one-body-render-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manifest = EvidenceManifest::new(
        &root,
        &workspace,
        "journey-one-body-five-masks",
        "journey-gallery",
    )
    .unwrap();
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace)
        .output()
        .unwrap();
    let commit = String::from_utf8(commit.stdout).unwrap().trim().to_owned();
    let mut chapters = Vec::new();
    for (index, chapter) in CHAPTERS.iter().enumerate() {
        if omit_chapter && index == 6 {
            continue;
        }
        let action = if repeat_action && index == 1 {
            "action-birth".to_owned()
        } else {
            format!("action-{chapter}")
        };
        let chapter_receipt = format!("receipt-{chapter}");
        let face_revision = if *chapter == "hear" {
            "5".to_owned()
        } else {
            format!("face-{chapter}")
        };
        json_output(
            &root,
            &mut manifest,
            &chapter_receipt,
            &json!({
                "schema":"conduit.journey/chapter-receipt@1", "source_commit":commit,
                "run_id":"test-run", "body_id":"test-body", "chapter_id":chapter,
                "action_ids":[action], "resulting_face_revision":face_revision,
                "outcome":"completed"
            }),
        );
        let sources: &[&str] = match *chapter {
            "birth" | "lull" => &["terminal"],
            "join" => &["qmp", "chromium"],
            "see" if omit_terminal_show => &["qmp"],
            "see" => &["qmp", "terminal"],
            "return" => &["qmp"],
            "hear" => &["direct", "llm-assisted"],
            _ => &["chromium"],
        };
        let mut media = Vec::new();
        for (offset, source) in sources.iter().enumerate() {
            let artifact = format!("media-{chapter}-{offset}");
            let capture_id = format!("capture-{chapter}-{offset}");
            let (kind, media_type, bytes): (EvidenceKind, &str, Vec<u8>) = match *source {
                "terminal" => (
                    EvidenceKind::ConsoleTranscript,
                    "text/plain; charset=utf-8",
                    b"synthetic terminal fixture".to_vec(),
                ),
                "direct" | "llm-assisted" => {
                    let mut bytes = Vec::from(&b"RIFF\x28\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data\x04\0\0\0\x01\0\0\0"[..]);
                    if silent_wav {
                        bytes[44] = 0;
                    }
                    (EvidenceKind::Audio, "audio/wav", bytes)
                }
                _ => (
                    EvidenceKind::Screenshot,
                    "image/png",
                    if invalid_png {
                        b"\x89PNG\r\n\x1a\nsynthetic".to_vec()
                    } else {
                        resvg::tiny_skia::Pixmap::new(1, 1)
                            .unwrap()
                            .encode_png()
                            .unwrap()
                    },
                ),
            };
            let sha = add(&root, &mut manifest, &artifact, kind, media_type, &bytes);
            let mut transcript_id = None;
            let mut transcript_sha256 = None;
            let mut validation_id = None;
            let mut audio_provenance_id = None;
            if kind == EvidenceKind::Audio {
                let transcript = format!("transcript-{offset}");
                let words = "The clock is running.";
                let show = format!("show-{offset}");
                let transcript_sha = json_output(
                    &root,
                    &mut manifest,
                    &transcript,
                    &json!({
                        "schema":"conduit.journey/speech-transcript@1", "source_commit":commit,
                        "run_id":"test-run", "body_id":"test-body", "chapter_id":chapter,
                        "show_id":show, "face_revision": if stale_transcript && offset == 0 { "old-face".to_owned() } else { face_revision.clone() },
                        "text":words, "original_model_output":if *source == "llm-assisted" { Some(words) } else { None }
                    }),
                );
                transcript_id = Some(transcript);
                transcript_sha256 = Some(transcript_sha);
                if !(omit_audio_delivery && offset == 0) {
                    let delivery = format!("delivery-{chapter}-{offset}");
                    let delivery_receipt = if guest_audio && offset == 0 {
                        json!({
                            "schema":"conduit.conduitos.opl2-proof/v1",
                            "status":"completed", "boot_id":"guest-boot",
                            "plan_id":format!("plan-{offset}"),
                            "active_play_id":format!("play-{offset}"),
                            "qemu_audio":{"source":"same-run-qemu-wav-output",
                                "sha256":sha, "bytes":bytes.len(), "nonzero_samples":1}
                        })
                    } else {
                        json!({
                            "schema":"conduit.body/selected-speech-terminal@1",
                            "outcome":"completed", "face_revision":5,
                            "face_revision_decimal":"5",
                            "source_show_id":show, "source_show_still_current":true,
                            "host_id":"synthetic-owner", "boot_id":"synthetic-boot",
                            "provider_sha256":"a".repeat(64),
                            "batches":[{
                                "plan_id":format!("plan-{offset}"),
                                "play_id":format!("play-{offset}"), "outcome":"completed",
                                "wav_sha256":sha, "wav_bytes":bytes.len(),
                                "pcm_sha256":digest(&bytes[44..]), "pcm_bytes":bytes.len()-44,
                                "pcm_blocks":1, "speaker_blocks_committed":1,
                                "speaker_frames_committed":1, "spoken_segments":[words]
                            }]
                        })
                    };
                    json_output(&root, &mut manifest, &delivery, &delivery_receipt);
                    audio_provenance_id = Some(delivery);
                }
                if *source == "llm-assisted" {
                    let validation = "model-validation";
                    json_output(
                        &root,
                        &mut manifest,
                        validation,
                        &json!({
                            "schema":"conduit.journey/model-validation@1", "source_commit":commit,
                            "run_id":"test-run", "body_id":"test-body", "show_id":show,
                        "face_revision":face_revision, "provider_id":"synthetic-model",
                        "model_id":"synthetic-model-v1",
                            "original_output_sha256":digest(words.as_bytes()),
                            "validated_text_sha256":digest(words.as_bytes()), "accepted":true
                        }),
                    );
                    validation_id = Some(validation);
                }
            }
            json_output(
                &root,
                &mut manifest,
                &capture_id,
                &json!({
                    "schema":"conduit.journey/capture-receipt@1", "source_commit":commit,
                    "run_id":if mixed_run && index == 4 && offset == 0 { "other-run" } else { "test-run" },
                    "body_id":"test-body", "chapter_id":chapter, "action_id":action,
                    "face_revision":face_revision, "media_output_id":artifact,
                    "media_sha256":sha, "capture_source":match *source { "direct" if guest_audio => "qemu-audio", "direct" | "llm-assisted" => "speaker-play", other => other },
                    "show_id":if kind == EvidenceKind::Audio { Some(format!("show-{offset}")) } else { None },
                    "plan_id":if kind == EvidenceKind::Audio { Some(format!("plan-{offset}")) } else { None },
                    "play_id":if kind == EvidenceKind::Audio { Some(format!("play-{offset}")) } else { None },
                    "transcript_id":transcript_id, "transcript_sha256":transcript_sha256,
                "speech_mode":if kind == EvidenceKind::Audio { Some(*source) } else { None },
                "voice_id":if kind == EvidenceKind::Audio { Some("synthetic-voice-v1") } else { None },
                "provider_id":if *source == "llm-assisted" { Some("synthetic-model") } else { None },
                "model_id":if *source == "llm-assisted" { Some("synthetic-model-v1") } else { None },
                    "validation_id":validation_id,
                    "audio_provenance_id":audio_provenance_id,
                    "qemu_boot_id":if *source == "direct" && guest_audio { Some("guest-boot") } else { None }
                }),
            );
            media.push(json!({"output_id":artifact,"receipt_id":capture_id,"alt":format!("Synthetic {source} capture") }));
        }
        chapters.push(json!({
            "id":chapter, "title":format!("Chapter {index}"), "intention":"Know what is happening",
            "action":"Use the current control", "result":"The view changed", "why":"The Body retained its identity",
            "next":"Continue", "receipt_id":chapter_receipt, "media":media,
            "limitations":["Synthetic test fixture only; no live execution claim"]
        }));
    }
    json_output(
        &root,
        &mut manifest,
        "journey",
        &json!({
            "schema":"conduit.journey/one-body-five-masks@1", "source_commit":commit,
            "run_id":"test-run", "body_id":"test-body", "chapters":chapters
        }),
    );
    manifest.finish(EvidenceResult::Complete).unwrap();
    let output = root.join("rendered");
    Fixture {
        root,
        output,
        commit,
    }
}

fn run(fixture: &Fixture) -> Result<(), String> {
    render_one_body_journey(&OneBodyJourneyRequest {
        evidence_root: fixture.root.clone(),
        output: fixture.output.clone(),
        commit: fixture.commit.clone(),
    })
}

#[test]
fn renders_only_complete_correlated_synthetic_fixture() {
    let fixture = fixture(false, false, false, false);
    run(&fixture).unwrap();
    let page = fs::read_to_string(fixture.output.join("index.html")).unwrap();
    assert!(page.contains("Chapter 8 of 8"));
    assert!(page.contains("Words in recorded audio (llm-assisted)"));
    assert!(page.contains("Captured from the selected speaker Play"));
    assert!(page.contains("class=\"chapter-run\""));
    assert!(page.contains("Open full-size capture"));
    assert!(page.contains("Direct mechanical reading"));
    assert!(page.contains("Finite model-assisted wording"));
    assert!(page.contains("aria-label=\"Main navigation\""));
    assert!(run(&fixture).unwrap_err().contains("already exists"));
}

#[test]
fn rejects_mixed_run_before_rendering() {
    let fixture = fixture(true, false, false, false);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("does not match its run"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_action_reused_as_a_later_chapter() {
    let fixture = fixture(false, false, false, true);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("repeats an action from an earlier chapter"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_stale_speech_and_missing_chapter() {
    let stale = fixture(false, true, false, false);
    assert!(run(&stale)
        .unwrap_err()
        .contains("speech/Show/Plan/Play correlation"));
    let missing = fixture(false, false, true, false);
    assert!(run(&missing).unwrap_err().contains("chapter count"));
    assert!(!stale.output.exists() && !missing.output.exists());
}

#[test]
fn rejects_tampered_media_hash() {
    let fixture = fixture(false, false, false, false);
    let path = fixture.root.join("media-join-0.png");
    let mut bytes = fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(path, bytes).unwrap();
    assert!(run(&fixture)
        .unwrap_err()
        .contains("evidence digest does not match"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_nondecodable_png_even_with_matching_digest() {
    let fixture = fixture_with_media(false, false, false, false, true, false);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("is not a supported real capture"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_silent_wav_even_with_matching_digest() {
    let fixture = fixture_with_media(false, false, false, false, false, true);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("is not a supported real capture"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_playable_wav_without_the_same_speaker_play() {
    let fixture =
        fixture_with_source_gap(false, false, false, false, false, false, false, true, false);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("lacks the selected speaker or QEMU delivery receipt"));
    assert!(!fixture.output.exists());
}

#[test]
fn renders_guest_audio_only_with_the_same_run_qemu_receipt() {
    let fixture =
        fixture_with_source_gap(false, false, false, false, false, false, false, false, true);
    run(&fixture).unwrap();
    let page = fs::read_to_string(fixture.output.join("index.html")).unwrap();
    assert!(page.contains("Captured from the same-run QEMU output"));
}

#[test]
fn rejects_missing_media_before_creating_a_page() {
    let fixture = fixture(false, false, false, false);
    fs::remove_file(fixture.root.join("media-join-0.png")).unwrap();
    assert!(run(&fixture).is_err());
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_visual_chapter_without_the_terminal_mask_capture() {
    let fixture =
        fixture_with_source_gap(false, false, false, false, false, false, true, false, false);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("chapter 'see' lacks its required user-visible capture source"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_optional_capture_in_complete_manifest() {
    let fixture = fixture(false, false, false, false);
    let path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let outputs = manifest["outputs"].as_array_mut().unwrap();
    outputs
        .iter_mut()
        .find(|output| output["id"] == "media-join-0")
        .unwrap()["required"] = json!(false);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(run(&fixture)
        .unwrap_err()
        .contains("optional or foreign-run"));
    assert!(!fixture.output.exists());
}
