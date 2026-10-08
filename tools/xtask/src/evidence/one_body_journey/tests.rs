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
        EvidenceKind::Document => "html",
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
        let event_id = if repeat_action && index == 1 {
            "event-birth".to_owned()
        } else {
            format!("event-{chapter}")
        };
        let chapter_receipt = format!("receipt-{chapter}");
        let event_source = format!("event-source-{chapter}");
        let face_revision = if *chapter == "hear" {
            "5".to_owned()
        } else {
            format!("face-{chapter}")
        };
        json_output(
            &root,
            &mut manifest,
            &event_source,
            &json!({
                "source_commit":commit,"run_id":"test-run","body_id":"test-body",
                "event_kind":"typed-interaction","event_id":event_id,
                "observed_at_unix_ms":index + 1,
                "resulting_face_revision":face_revision,"outcome":"completed"
            }),
        );
        json_output(
            &root,
            &mut manifest,
            &chapter_receipt,
            &json!({
                "schema":"conduit.journey/chapter-receipt@2", "source_commit":commit,
                "run_id":"test-run", "body_id":"test-body", "chapter_id":chapter,
                "events":[{"kind":"typed-interaction", "id":event_id,
                    "observed_at_unix_ms":index + 1,
                    "face_revision":face_revision,"source_receipt_id":event_source}],
                "resulting_face_revision":face_revision,
                "outcome":"completed"
            }),
        );
        let sources: &[&str] = match *chapter {
            "birth" | "lull" => &["terminal"],
            "join" => &["qmp", "chromium"],
            "see" if omit_terminal_show => &["chromium"],
            "see" => &["chromium", "terminal"],
            "start" => &["chromium", "qmp"],
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
            let media_source = format!("media-source-{chapter}-{offset}");
            json_output(
                &root,
                &mut manifest,
                &media_source,
                &json!({
                    "source_commit":commit,"run_id":"test-run","body_id":"test-body",
                    "event_kind":"typed-interaction","event_id":event_id,
                    "face_revision":face_revision,"media_path":format!("{artifact}.{}", if kind == EvidenceKind::Audio { "wav" } else if kind == EvidenceKind::Screenshot { "png" } else { "txt" }),
                    "media_sha256":sha,
                    "capture_source":match *source { "direct" if guest_audio => "qemu-audio", "direct" | "llm-assisted" => "speaker-play", other => other }
                }),
            );
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
                            "schema":"conduit.conduitos/speech-audio-proof@1",
                            "status":"completed", "boot_id":"guest-boot",
                            "plan_id":format!("plan-{offset}"),
                            "active_play_id":format!("play-{offset}"),
                            "source_show_id":show, "face_revision_decimal":"5",
                            "spoken_text_sha256":digest(words.as_bytes()),
                            "voice_id":"synthetic-voice-v1",
                            "qemu_audio":{"source":"same-run-qemu-wav-output",
                                "sha256":sha, "bytes":bytes.len(), "nonzero_samples":1}
                        })
                    } else if *source == "llm-assisted" {
                        json!({
                            "schema":"conduit.body/owner-spoken-terminal@1",
                            "outcome":"available", "mode":"llm-assisted",
                            "show_id":show, "active_play_id":format!("model-play-{offset}"),
                            "accepted_wording":words, "speaker_played":true,
                            "generation_evidence":{
                                "provider_identity":"synthetic-model",
                                "model_identity":"synthetic-model-v1",
                                "candidate_digest":"synthetic-candidate-digest",
                                "validation_receipt_identity":"synthetic-validation-receipt",
                                "original_model_output":words
                            },
                            "speaker_playback":{
                                "schema":"conduit.body/owner-spoken-speaker-play@1",
                                "source_show_id":show,
                                "source_face_revision_decimal":"5",
                                "host_id":"synthetic-owner", "boot_id":"synthetic-boot",
                                "provider_sha256":"a".repeat(64),
                                "plan_id":format!("plan-{offset}"),
                                "play_id":format!("play-{offset}"), "outcome":"completed",
                                "wav_sha256":sha, "wav_bytes":bytes.len(),
                                "pcm_sha256":digest(&bytes[44..]), "pcm_bytes":bytes.len()-44,
                                "pcm_blocks":1, "speaker_blocks_committed":1,
                                "speaker_frames_committed":1, "spoken_segments":[words]
                            }
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
                    "schema":"conduit.journey/capture-receipt@2", "source_commit":commit,
                    "run_id":if mixed_run && index == 4 && offset == 0 { "other-run" } else { "test-run" },
                    "body_id":"test-body", "chapter_id":chapter, "event_id":event_id,
                    "event_kind":"typed-interaction", "source_receipt_id":media_source,
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

fn rewrite_json_output(fixture: &Fixture, output_id: &str, value: &Value) {
    let path = fixture.root.join(format!("{output_id}.json"));
    let bytes = serde_json::to_vec(value).unwrap();
    fs::write(path, &bytes).unwrap();
    let manifest_path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let declared = manifest["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| output["id"] == output_id)
        .unwrap();
    declared["sha256"] = json!(digest(&bytes));
    declared["bytes"] = json!(bytes.len());
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

fn append_json_output(fixture: &Fixture, output_id: &str, value: &Value) {
    let path = format!("{output_id}.json");
    let bytes = serde_json::to_vec(value).unwrap();
    fs::write(fixture.root.join(&path), &bytes).unwrap();
    let manifest_path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let mut declared = manifest["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|output| output["id"] == "event-source-join")
        .unwrap()
        .clone();
    declared["id"] = json!(output_id);
    declared["path"] = json!(path);
    declared["sha256"] = json!(digest(&bytes));
    declared["bytes"] = json!(bytes.len());
    manifest["outputs"].as_array_mut().unwrap().push(declared);
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

fn direct_owner_delivery(fixture: &Fixture) -> Value {
    let selected: Value =
        serde_json::from_slice(&fs::read(fixture.root.join("delivery-hear-0.json")).unwrap())
            .unwrap();
    json!({
        "schema":"conduit.body/owner-spoken-terminal@1",
        "mode":"direct", "outcome":"available", "show_id":"show-0",
        "direct_reading_complete":true, "speaker_played":true,
        "speaker_playback":{
            "schema":"conduit.body/selected-speech-terminal@1",
            "outcome":"completed", "source_show_id":"show-0",
            "face_revision":5, "face_revision_decimal":"5",
            "host_id":"synthetic-owner", "boot_id":"synthetic-boot",
            "provider_sha256":"a".repeat(64),
            "batches":selected["batches"]
        }
    })
}

#[test]
fn accepts_only_the_completed_direct_mask_listener_batch() {
    let accepted = fixture(false, false, false, false);
    let direct = direct_owner_delivery(&accepted);
    rewrite_json_output(&accepted, "delivery-hear-0", &direct);
    run(&accepted).unwrap();

    let incomplete = fixture(false, false, false, false);
    let mut direct = direct_owner_delivery(&incomplete);
    direct["direct_reading_complete"] = json!(false);
    rewrite_json_output(&incomplete, "delivery-hear-0", &direct);
    assert!(run(&incomplete)
        .unwrap_err()
        .contains("direct reading lacks its completed same-Play speaker batch"));

    let wrong_audio = fixture(false, false, false, false);
    let mut direct = direct_owner_delivery(&wrong_audio);
    direct["speaker_playback"]["batches"][0]["speaker_frames_committed"] = json!(2);
    rewrite_json_output(&wrong_audio, "delivery-hear-0", &direct);
    assert!(run(&wrong_audio)
        .unwrap_err()
        .contains("audio WAV differs from the selected speaker Play"));
}

#[test]
fn captures_can_follow_successive_typed_event_face_revisions() {
    let fixture = fixture(false, false, false, false);
    let first_source = json!({"source_commit":fixture.commit,"run_id":"test-run",
        "body_id":"test-body","event_kind":"membership","event_id":"event-join-first",
        "observed_at_unix_ms":2,
        "resulting_face_revision":"face-join-first","outcome":"completed"});
    let last_source = json!({"source_commit":fixture.commit,"run_id":"test-run",
        "body_id":"test-body","event_kind":"acknowledged-show","event_id":"event-join",
        "observed_at_unix_ms":2,
        "resulting_face_revision":"face-join","outcome":"completed"});
    rewrite_json_output(&fixture, "event-source-join", &first_source);
    append_json_output(&fixture, "event-source-join-last", &last_source);
    let receipt_path = fixture.root.join("receipt-join.json");
    let mut receipt: Value = serde_json::from_slice(&fs::read(receipt_path).unwrap()).unwrap();
    receipt["events"] = json!([
        {"kind":"membership","id":"event-join-first","face_revision":"face-join-first","observed_at_unix_ms":2,"source_receipt_id":"event-source-join"},
        {"kind":"acknowledged-show","id":"event-join","face_revision":"face-join","observed_at_unix_ms":2,"source_receipt_id":"event-source-join-last"}
    ]);
    rewrite_json_output(&fixture, "receipt-join", &receipt);

    let capture_path = fixture.root.join("capture-join-0.json");
    let mut capture: Value = serde_json::from_slice(&fs::read(capture_path).unwrap()).unwrap();
    capture["event_id"] = json!("event-join-first");
    capture["event_kind"] = json!("membership");
    capture["face_revision"] = json!("face-join-first");
    rewrite_json_output(&fixture, "capture-join-0", &capture);
    let media_source_path = fixture.root.join("media-source-join-0.json");
    let mut media_source: Value =
        serde_json::from_slice(&fs::read(media_source_path).unwrap()).unwrap();
    media_source["event_id"] = json!("event-join-first");
    media_source["event_kind"] = json!("membership");
    media_source["face_revision"] = json!("face-join-first");
    rewrite_json_output(&fixture, "media-source-join-0", &media_source);
    let media_source_path = fixture.root.join("media-source-join-1.json");
    let mut media_source: Value =
        serde_json::from_slice(&fs::read(media_source_path).unwrap()).unwrap();
    media_source["event_kind"] = json!("acknowledged-show");
    rewrite_json_output(&fixture, "media-source-join-1", &media_source);
    let capture_path = fixture.root.join("capture-join-1.json");
    let mut capture: Value = serde_json::from_slice(&fs::read(capture_path).unwrap()).unwrap();
    capture["event_kind"] = json!("acknowledged-show");
    rewrite_json_output(&fixture, "capture-join-1", &capture);
    run(&fixture).unwrap();
}

#[test]
fn rejects_capture_at_the_wrong_event_face_revision() {
    let fixture = fixture(false, false, false, false);
    let capture_path = fixture.root.join("capture-join-0.json");
    let mut capture: Value = serde_json::from_slice(&fs::read(capture_path).unwrap()).unwrap();
    capture["face_revision"] = json!("stale-face");
    rewrite_json_output(&fixture, "capture-join-0", &capture);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("does not match its run, event, Face, or digest"));
}

#[test]
fn rejects_relabelled_producer_event_and_media_sources() {
    let event = fixture(false, false, false, false);
    let path = event.root.join("event-source-join.json");
    let mut source: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    source["event_kind"] = json!("acknowledged-show");
    rewrite_json_output(&event, "event-source-join", &source);
    assert!(run(&event)
        .unwrap_err()
        .contains("event source receipt is not correlated"));

    let media = fixture(false, false, false, false);
    let path = media.root.join("media-source-join-0.json");
    let mut source: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    source["media_sha256"] = json!("b".repeat(64));
    rewrite_json_output(&media, "media-source-join-0", &source);
    assert!(run(&media)
        .unwrap_err()
        .contains("source receipt is not correlated"));
}

#[test]
fn rejects_reordered_or_retimestamped_producer_events() {
    let reordered = fixture(false, false, false, false);
    let path = reordered.root.join("receipt-join.json");
    let mut receipt: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    receipt["events"][0]["observed_at_unix_ms"] = json!(1);
    rewrite_json_output(&reordered, "receipt-join", &receipt);
    assert!(run(&reordered)
        .unwrap_err()
        .contains("event source receipt is not correlated"));

    let reversed = fixture(false, false, false, false);
    let path = reversed.root.join("receipt-join.json");
    let mut receipt: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    receipt["events"][0]["observed_at_unix_ms"] = json!(1);
    rewrite_json_output(&reversed, "receipt-join", &receipt);
    let path = reversed.root.join("event-source-join.json");
    let mut source: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    source["observed_at_unix_ms"] = json!(1);
    rewrite_json_output(&reversed, "event-source-join", &source);
    let path = reversed.root.join("receipt-birth.json");
    let mut birth: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    birth["events"][0]["observed_at_unix_ms"] = json!(2);
    rewrite_json_output(&reversed, "receipt-birth", &birth);
    let path = reversed.root.join("event-source-birth.json");
    let mut source: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    source["observed_at_unix_ms"] = json!(2);
    rewrite_json_output(&reversed, "event-source-birth", &source);
    assert!(run(&reversed)
        .unwrap_err()
        .contains("event chronology is reversed"));
}

#[test]
fn renders_only_complete_correlated_synthetic_fixture() {
    let fixture = fixture(false, false, false, false);
    run(&fixture).unwrap();
    let page = fs::read_to_string(fixture.output.join("index.html")).unwrap();
    assert!(page.contains("Chapter 8 of 8"));
    assert!(page.contains("Start an interval ticker"));
    assert!(page.contains("it does not tell time of day"));
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
fn rejects_event_reused_as_a_later_chapter() {
    let fixture = fixture(false, false, false, true);
    assert!(run(&fixture)
        .unwrap_err()
        .contains("repeats an event identity"));
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
fn rejects_model_artifact_when_the_listener_play_did_not_complete() {
    let fixture = fixture(false, false, false, false);
    let path = fixture.root.join("delivery-hear-1.json");
    let mut terminal: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    terminal["speaker_played"] = json!(false);
    let bytes = serde_json::to_vec(&terminal).unwrap();
    fs::write(&path, &bytes).unwrap();
    let manifest_path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let declared = manifest["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| output["id"] == "delivery-hear-1")
        .unwrap();
    declared["sha256"] = json!(digest(&bytes));
    declared["bytes"] = json!(bytes.len());
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(run(&fixture)
        .unwrap_err()
        .contains("model wording lacks its completed selected speaker Play"));
    assert!(!fixture.output.exists());
}

#[test]
fn rejects_model_transcript_from_a_different_owner_generation() {
    let fixture = fixture(false, false, false, false);
    let path = fixture.root.join("delivery-hear-1.json");
    let mut terminal: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    terminal["generation_evidence"]["original_model_output"] = json!("Other model output");
    let bytes = serde_json::to_vec(&terminal).unwrap();
    fs::write(&path, &bytes).unwrap();
    let manifest_path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let declared = manifest["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| output["id"] == "delivery-hear-1")
        .unwrap();
    declared["sha256"] = json!(digest(&bytes));
    declared["bytes"] = json!(bytes.len());
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(run(&fixture)
        .unwrap_err()
        .contains("model wording lacks its completed selected speaker Play"));
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
fn guest_opl2_sound_cannot_masquerade_as_spoken_audio() {
    let fixture =
        fixture_with_source_gap(false, false, false, false, false, false, false, false, true);
    let path = fixture.root.join("delivery-hear-0.json");
    let mut receipt: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    receipt["schema"] = json!("conduit.conduitos.opl2-proof/v1");
    let bytes = serde_json::to_vec(&receipt).unwrap();
    fs::write(&path, &bytes).unwrap();
    let manifest_path = fixture.root.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let declared = manifest["outputs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|output| output["id"] == "delivery-hear-0")
        .unwrap();
    declared["sha256"] = json!(digest(&bytes));
    declared["bytes"] = json!(bytes.len());
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(run(&fixture)
        .unwrap_err()
        .contains("audio WAV differs from the guest run's QEMU output"));
    assert!(!fixture.output.exists());
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
