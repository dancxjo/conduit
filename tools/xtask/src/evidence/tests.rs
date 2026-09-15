use super::*;

fn temporary_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("conduit-evidence-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn manifest(root: &Path) -> EvidenceManifest {
    EvidenceManifest::new(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "proof",
        "suite",
    )
    .unwrap()
}

fn output(id: &str, path: &str, required: bool) -> EvidenceOutput {
    EvidenceOutput {
        id: id.into(),
        kind: EvidenceKind::Screenshot,
        path: path.into(),
        media_type: "image/png".into(),
        required,
        provenance: EvidenceProvenance {
            scenario_id: "scenario".into(),
            asserted_semantic_disposition: Some("delivered".into()),
            ..Default::default()
        },
    }
}

#[test]
fn complete_manifest_digest_binds_exact_bytes() {
    let root = temporary_root("digest");
    fs::write(root.join("capture.png"), b"canonical bytes").unwrap();
    let mut evidence = manifest(&root);
    evidence
        .declare(output("capture", "capture.png", true))
        .unwrap();
    evidence.finish(EvidenceResult::Complete).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(document["schema"], EVIDENCE_SCHEMA);
    assert_eq!(document["result"], "complete");
    assert_eq!(document["outputs"][0]["bytes"], 15);
    assert_eq!(
        document["outputs"][0]["sha256"],
        "a62cbfa5ab07ca2085092bb00488c2256b93dedcd2a8bd88e65b6ee055d7a499"
    );
    assert_eq!(document["outputs"][0]["scenario_id"], "scenario");
    assert_eq!(
        document["outputs"][0]["asserted_semantic_disposition"],
        "delivered"
    );
    assert!(document.get("timestamp").is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_hears_speaks_manifest_requires_typed_audio_and_receipts() {
    let root = temporary_root("hears-speaks");
    let wav =
        b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80>\0\0\0}\0\0\x02\0\x10\0data\0\0\0\0";
    let declarations = [
        (
            "hears-speaks.input-pcm",
            EvidenceKind::Audio,
            "input.pcm",
            "audio/L16; rate=16000; channels=1",
            b"\0\0".as_slice(),
        ),
        (
            "hears-speaks.input-wav",
            EvidenceKind::Audio,
            "input.wav",
            "audio/wav",
            wav.as_slice(),
        ),
        (
            "hears-speaks.recognition",
            EvidenceKind::MachineReadableManifest,
            "recognition.json",
            "application/json",
            b"{}".as_slice(),
        ),
        (
            "hears-speaks.response",
            EvidenceKind::MachineReadableManifest,
            "response.json",
            "application/json",
            b"{}".as_slice(),
        ),
        (
            "hears-speaks.output-wav",
            EvidenceKind::Audio,
            "output.wav",
            "audio/wav",
            wav.as_slice(),
        ),
        (
            "hears-speaks.receipt",
            EvidenceKind::MachineReadableManifest,
            "receipt.json",
            "application/json",
            b"{}".as_slice(),
        ),
    ];
    let mut evidence = EvidenceManifest::new(
        &root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "journey-hears-speaks",
        "journey-gallery",
    )
    .unwrap();
    for (id, kind, path, media_type, bytes) in declarations {
        fs::write(root.join(path), bytes).unwrap();
        evidence
            .declare(EvidenceOutput {
                id: id.into(),
                kind,
                path: path.into(),
                media_type: media_type.into(),
                required: true,
                provenance: EvidenceProvenance {
                    scenario_id: "hears-speaks.recorded-addressed-house@1".into(),
                    plan_id: Some("plan".into()),
                    active_play_id: Some("play".into()),
                    asserted_semantic_disposition: Some("completed".into()),
                    proof_class: Some("hosted-recorded-audio-plan-play".into()),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    evidence.finish(EvidenceResult::Complete).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    let commit = document["git_commit"].as_str().unwrap().to_owned();
    verify(&VerificationRequest {
        root: root.clone(),
        commit,
        result: ExpectedEvidenceResult::Complete,
        proof_id: "journey-hears-speaks".into(),
        suite_id: "journey-gallery".into(),
    })
    .unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn complete_little_life_manifest_requires_exact_checkpoints_and_completed_play() {
    let root = temporary_root("little-life");
    complete_little_life_evidence(&root);
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    verify(&VerificationRequest {
        root: root.clone(),
        commit: document["git_commit"].as_str().unwrap().into(),
        result: ExpectedEvidenceResult::Complete,
        proof_id: "journey-little-life".into(),
        suite_id: "journey-gallery".into(),
    })
    .unwrap();
    fs::remove_dir_all(root).unwrap();
}

fn complete_little_life_evidence(root: &Path) {
    let plan = "little-life-plan";
    let play = "little-life-play";
    for (generation, width, height) in [(0, 32, 32), (1, 640, 320), (8, 640, 320), (32, 640, 320)] {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&u32::to_be_bytes(width));
        png.extend_from_slice(&u32::to_be_bytes(height));
        fs::write(root.join(format!("t{generation:03}.png")), png).unwrap();
    }
    let mut transcript = String::new();
    for generation in 1..=32 {
        transcript.push_str(&format!(
            "SCALAR-FIELD title=\"Orbium evolution\" generation={generation} width=32 height=32 profile=fixed-q16.16\n"
        ));
        for _ in 0..16 {
            transcript.push_str("                                \n");
        }
    }
    fs::write(root.join("presentation.txt"), transcript).unwrap();
    fs::write(
        root.join("execution.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.observatory.snapshot/v2",
            "plans": [{"plan_id": plan}],
            "observations": [{
                "active_play_id": play,
                "kind": {"PlanTerminal": {"disposition": "Completed"}}
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let mut evidence = EvidenceManifest::new(
        &root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "journey-little-life",
        "journey-gallery",
    )
    .unwrap();
    for generation in [0, 1, 8, 32] {
        let seed = generation == 0;
        evidence
            .declare(EvidenceOutput {
                id: format!("little-life.t{generation}"),
                kind: EvidenceKind::Screenshot,
                path: format!("t{generation:03}.png").into(),
                media_type: "image/png".into(),
                required: true,
                provenance: EvidenceProvenance {
                    scenario_id: "little-life.orbium-lenia@1".into(),
                    step_id: Some(format!("generation-{generation}")),
                    plan_id: Some(plan.into()),
                    active_play_id: Some(play.into()),
                    renderer_id: Some(if seed {
                        "presentation/gray8-bitmap@1".into()
                    } else {
                        "std/kernel-present-scalar-field@1".into()
                    }),
                    asserted_semantic_disposition: Some("completed".into()),
                    proof_class: Some(if seed {
                        "deterministic-orbium-seed-bitmap".into()
                    } else {
                        "std-scalar-field-terminal-presentation".into()
                    }),
                    image_width: Some(if seed { 32 } else { 640 }),
                    image_height: Some(if seed { 32 } else { 320 }),
                    physical_evidence: Some(false),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    for (id, kind, path, media_type, step, renderer, proof_class) in [
        (
            "little-life.presentation",
            EvidenceKind::ConsoleTranscript,
            "presentation.txt",
            "text/plain; charset=utf-8",
            "generation-32",
            Some("std/kernel-present-scalar-field@1"),
            "std-scalar-field-terminal-presentation",
        ),
        (
            "little-life.execution",
            EvidenceKind::MachineReadableManifest,
            "execution.json",
            "application/json",
            "plan-terminal",
            None,
            "ordinary-plan-play-execution-report",
        ),
    ] {
        evidence
            .declare(EvidenceOutput {
                id: id.into(),
                kind,
                path: path.into(),
                media_type: media_type.into(),
                required: true,
                provenance: EvidenceProvenance {
                    scenario_id: "little-life.orbium-lenia@1".into(),
                    step_id: Some(step.into()),
                    plan_id: Some(plan.into()),
                    active_play_id: Some(play.into()),
                    renderer_id: renderer.map(str::to_owned),
                    asserted_semantic_disposition: Some("completed".into()),
                    proof_class: Some(proof_class.into()),
                    physical_evidence: Some(false),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    evidence.finish(EvidenceResult::Complete).unwrap();
}

#[test]
fn missing_required_output_is_manifested_as_incomplete_and_refused() {
    let root = temporary_root("missing");
    let mut evidence = manifest(&root);
    evidence
        .declare(output("required", "missing.png", true))
        .unwrap();
    assert!(evidence
        .finish(EvidenceResult::Complete)
        .unwrap_err()
        .contains("required"));
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(document["result"], "diagnostic-incomplete");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicate_ids_and_escaping_paths_are_rejected() {
    let root = temporary_root("bounds");
    let mut evidence = manifest(&root);
    evidence.declare(output("one", "one.png", false)).unwrap();
    assert!(evidence.declare(output("one", "two.png", false)).is_err());
    assert!(evidence
        .declare(output("escape", "../escape.png", false))
        .is_err());
    assert!(evidence
        .declare(output("alias", "./one.png", false))
        .is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn duplicate_paths_and_count_overflow_are_rejected() {
    let root = temporary_root("count");
    let mut evidence = manifest(&root);
    evidence.declare(output("one", "same.png", false)).unwrap();
    evidence.declare(output("two", "same.png", false)).unwrap();
    assert!(evidence.finish(EvidenceResult::Complete).is_err());
    let mut evidence = manifest(&root);
    for index in 0..MAX_EVIDENCE_OUTPUTS {
        evidence
            .declare(output(
                &format!("output-{index}"),
                &format!("output-{index}.png"),
                false,
            ))
            .unwrap();
    }
    assert!(evidence
        .declare(output("overflow", "overflow.png", false))
        .is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn diagnostic_artifacts_cannot_look_complete() {
    let root = temporary_root("diagnostic");
    fs::write(root.join("capture.png"), b"diagnostic").unwrap();
    let mut evidence = manifest(&root);
    evidence
        .declare(output("capture", "capture.png", false))
        .unwrap();
    evidence
        .finish(EvidenceResult::DiagnosticIncomplete)
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(document["result"], "diagnostic-incomplete");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pre_capture_diagnostic_verifies_without_invented_outputs() {
    let root = temporary_root("pre-capture-diagnostic");
    let mut evidence = manifest(&root);
    evidence
        .finish(EvidenceResult::DiagnosticIncomplete)
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(document["result"], "diagnostic-incomplete");
    assert_eq!(document["outputs"].as_array().unwrap().len(), 0);
    let request = VerificationRequest {
        root: root.clone(),
        commit: document["git_commit"].as_str().unwrap().to_owned(),
        result: ExpectedEvidenceResult::DiagnosticIncomplete,
        proof_id: "proof".into(),
        suite_id: "suite".into(),
    };
    verify(&request).unwrap();
    fs::write(root.join("undeclared.png"), b"not admitted").unwrap();
    assert!(verify(&request).unwrap_err().contains("undeclared"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actions_checkout_sha_is_validated_without_invoking_git() {
    let sha = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";
    assert_eq!(
        exact_git_commit(Path::new("/not/a/checkout"), Some(sha)).unwrap(),
        sha.to_ascii_lowercase()
    );
    assert!(exact_git_commit(Path::new("/not/a/checkout"), Some("floating-main")).is_err());
}

#[test]
fn bounded_capture_declarations_import_exact_provenance() {
    let root = temporary_root("capture-import");
    fs::write(root.join("overview.png"), b"png").unwrap();
    fs::write(root.join("captures.json"), br#"{
      "schema":"conduit.capture-declarations/v1",
      "outputs":[{
        "id":"patchbay.overview","kind":"screenshot","path":"overview.png",
        "media_type":"image/png","required":true,
        "provenance":{"scenario_id":"patchbay-html.overview@1","step_id":"prove.browser-host.patchbay-html-matrix",
          "browser_engine":"chromium","browser_version":"1","viewport":"1440x1000","device_scale_factor":"1",
          "locale":"en-US","timezone":"UTC","presentation_id":"presentation","presentation_revision":"1",
          "plan_id":"plan","active_play_id":"play","manifestation_id":"manifestation",
          "renderer_id":"patchbay-html/dom-svg@1","asserted_semantic_disposition":"available"}
      }]}
    "#).unwrap();
    let mut evidence = manifest(&root);
    evidence
        .import_capture_declarations(Path::new("captures.json"), &["patchbay.overview"])
        .unwrap();
    evidence.finish(EvidenceResult::Complete).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(document["outputs"][0]["plan_id"], "plan");
    assert_eq!(document["outputs"][0]["browser_engine"], "chromium");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verifier_recomputes_complete_browser_evidence() {
    let root = temporary_root("verify-complete");
    let commit = complete_browser_evidence(&root);
    verify(&VerificationRequest {
        root: root.clone(),
        commit,
        result: ExpectedEvidenceResult::Complete,
        proof_id: "browser-host".into(),
        suite_id: "prove.browser-host".into(),
    })
    .unwrap();
    fs::remove_dir_all(root).unwrap();
}

fn complete_browser_evidence(root: &Path) -> String {
    let mut evidence = EvidenceManifest::new(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "browser-host",
        "prove.browser-host",
    )
    .unwrap();
    for (index, id) in [
        "patchbay.capture-declarations",
        "patchbay.overview",
        "patchbay.selected-gear",
        "patchbay.plan-lens",
        "patchbay.play-lens",
        "patchbay.signs-lens",
        "patchbay.route-recovery",
        "patchbay.interaction",
        "patchbay.high-contrast",
        "patchbay.disconnected",
        "patchbay.responsive",
    ]
    .into_iter()
    .enumerate()
    {
        let path = format!("output-{index}");
        fs::write(root.join(&path), id.as_bytes()).unwrap();
        let mut declaration = output(id, &path, true);
        if id == "patchbay.capture-declarations" {
            declaration.kind = EvidenceKind::MachineReadableManifest;
            declaration.media_type = "application/json".into();
        } else {
            declaration.provenance = EvidenceProvenance {
                scenario_id: format!("scenario-{index}"),
                step_id: Some("step".into()),
                browser_engine: Some("chromium".into()),
                browser_version: Some("1".into()),
                viewport: Some("1440x1000".into()),
                device_scale_factor: Some("1".into()),
                locale: Some("en-US".into()),
                timezone: Some("UTC".into()),
                presentation_id: Some("presentation".into()),
                presentation_revision: Some("1".into()),
                plan_id: Some("plan".into()),
                active_play_id: Some("play".into()),
                manifestation_id: Some("manifestation".into()),
                renderer_id: Some("renderer".into()),
                asserted_semantic_disposition: Some("asserted".into()),
                ..Default::default()
            };
        }
        evidence.declare(declaration).unwrap();
    }
    evidence.finish(EvidenceResult::Complete).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    document["git_commit"].as_str().unwrap().to_owned()
}

fn complete_conduitos_evidence(root: &Path, commit: &str) {
    let transcript = concat!(
        "CONDUIT_BOOT_SIGN {}\n",
        "CONDUIT_KERNEL_SIGN {}\n",
        "CONDUIT_OBSERVATORY_SNAPSHOT {}\n",
        "CONDUIT_SERIAL_PRESENT HELLO, CONDUITOS\n",
    );
    fs::write(root.join("x86_64-console.txt"), transcript).unwrap();
    let mut evidence = EvidenceManifest::new(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "conduitos-x86_64",
        "conduitos.prove.x86_64",
    )
    .unwrap();
    evidence
        .declare(EvidenceOutput {
            id: "conduitos.x86_64.console".into(),
            kind: EvidenceKind::ConsoleTranscript,
            path: "x86_64-console.txt".into(),
            media_type: "text/plain; charset=utf-8".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "conduitos.x86_64.p5-console@1".into(),
                step_id: Some("conduitos.prove.x86_64.semantic-terminal".into()),
                plan_id: Some("plan".into()),
                active_play_id: Some("play".into()),
                asserted_semantic_disposition: Some("terminal-validated".into()),
                proof_class: Some("freestanding-emulator".into()),
                architecture: Some("x86_64".into()),
                architecture_rung: Some("conduitos/x86_64/P5-observatory-patchbay".into()),
                emulator: Some("qemu-system-x86_64".into()),
                emulator_version: Some("QEMU emulator version 10.0.0".into()),
                machine: Some(
                    "q35-single-cpu-64m-headless-xhci-usb-kbd-usb-mouse-usb-ftdi-adlib".into(),
                ),
                firmware: Some("limine".into()),
                host_id: Some("a".repeat(64)),
                boot_id: Some("b".repeat(64)),
                kernel_artifact_id: Some(format!("conduitos-build/{commit}")),
                kernel_artifact_sha256: Some("c".repeat(64)),
                capture_trigger: Some("semantic-result-and-terminal-signs".into()),
                capture_byte_limit: Some(256 * 1024),
                physical_evidence: Some(false),
                ..Default::default()
            },
        })
        .unwrap();
    evidence.finish(EvidenceResult::Complete).unwrap();
}

fn complete_hears_speaks_evidence(root: &Path) {
    let wav =
        b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80>\0\0\0}\0\0\x02\0\x10\0data\0\0\0\0";
    let files = [
        (
            "hears-speaks.input-pcm",
            EvidenceKind::Audio,
            "input.pcm",
            "audio/L16; rate=16000; channels=1",
            b"\0\0".as_slice(),
        ),
        (
            "hears-speaks.input-wav",
            EvidenceKind::Audio,
            "input.wav",
            "audio/wav",
            wav.as_slice(),
        ),
        (
            "hears-speaks.recognition",
            EvidenceKind::MachineReadableManifest,
            "recognition.json",
            "application/json",
            b"{}".as_slice(),
        ),
        (
            "hears-speaks.response",
            EvidenceKind::MachineReadableManifest,
            "response.json",
            "application/json",
            b"{}".as_slice(),
        ),
        (
            "hears-speaks.output-wav",
            EvidenceKind::Audio,
            "output.wav",
            "audio/wav",
            wav.as_slice(),
        ),
        (
            "hears-speaks.receipt",
            EvidenceKind::MachineReadableManifest,
            "receipt.json",
            "application/json",
            b"{}".as_slice(),
        ),
    ];
    let mut evidence = EvidenceManifest::new(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "journey-hears-speaks",
        "journey-gallery",
    )
    .unwrap();
    for (id, kind, path, media_type, bytes) in files {
        fs::write(root.join(path), bytes).unwrap();
        evidence
            .declare(EvidenceOutput {
                id: id.into(),
                kind,
                path: path.into(),
                media_type: media_type.into(),
                required: true,
                provenance: EvidenceProvenance {
                    scenario_id: "hears-speaks.recorded-addressed-house@1".into(),
                    plan_id: Some("plan".into()),
                    active_play_id: Some("play".into()),
                    asserted_semantic_disposition: Some("completed".into()),
                    proof_class: Some("hosted-recorded-audio-plan-play".into()),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    evidence.finish(EvidenceResult::Complete).unwrap();
}

fn complete_two_faces_evidence(root: &Path) {
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01";
    let presentation = "presentation/two-faces";
    let files = [
        (
            "two-faces.native-frame",
            EvidenceKind::Screenshot,
            "native.png",
            png.as_slice(),
        ),
        (
            "two-faces.native-receipt",
            EvidenceKind::MachineReadableManifest,
            "native.json",
            br#"{"presentation_id":"presentation/two-faces","presentation_revision":1,"renderer_plan_id":"native-plan","renderer_play_id":"native-play","manifestation_id":"native-manifestation","renderer_implementation":"presentation/renderer-wayland@1","lifecycle":"available","pixel_equality_claimed":false}"#.as_slice(),
        ),
        (
            "two-faces.browser-frame",
            EvidenceKind::Screenshot,
            "browser.png",
            png.as_slice(),
        ),
        (
            "two-faces.browser-receipt",
            EvidenceKind::MachineReadableManifest,
            "browser.json",
            br#"{"presentation_id":"presentation/two-faces","presentation_revision":1,"renderer_plan_id":"browser-plan","renderer_play_id":"browser-play","manifestation_id":"browser-manifestation","renderer_implementation":"presentation/renderer-dom-svg@1","lifecycle":"available","browser_engine":"chromium","browser_version":"151.0","viewport":"1366x768","device_scale_factor":"1","locale":"en-US","timezone":"UTC","pixel_equality_claimed":false}"#.as_slice(),
        ),
    ];
    let mut evidence = EvidenceManifest::new(
        root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "journey-one-form-two-faces",
        "journey-gallery",
    )
    .unwrap();
    for (id, kind, path, bytes) in files {
        fs::write(root.join(path), bytes).unwrap();
        let native = id.contains("native");
        evidence
            .declare(EvidenceOutput {
                id: id.into(),
                kind,
                path: path.into(),
                media_type: if kind == EvidenceKind::Screenshot {
                    "image/png".into()
                } else {
                    "application/json".into()
                },
                required: true,
                provenance: EvidenceProvenance {
                    scenario_id: "one-form-two-faces.front-door@1".into(),
                    presentation_id: Some(presentation.into()),
                    presentation_revision: Some("1".into()),
                    plan_id: Some(
                        if native {
                            "native-plan"
                        } else {
                            "browser-plan"
                        }
                        .into(),
                    ),
                    active_play_id: Some(
                        if native {
                            "native-play"
                        } else {
                            "browser-play"
                        }
                        .into(),
                    ),
                    manifestation_id: Some(
                        if native {
                            "native-manifestation"
                        } else {
                            "browser-manifestation"
                        }
                        .into(),
                    ),
                    renderer_id: Some(
                        if native {
                            "presentation/renderer-wayland@1"
                        } else {
                            "presentation/renderer-dom-svg@1"
                        }
                        .into(),
                    ),
                    asserted_semantic_disposition: Some("manifestation-available".into()),
                    proof_class: Some(
                        if native {
                            "native-software-renderer"
                        } else {
                            "live-browser"
                        }
                        .into(),
                    ),
                    browser_engine: (!native).then(|| "chromium".into()),
                    browser_version: (!native).then(|| "151.0".into()),
                    viewport: (!native).then(|| "1366x768".into()),
                    device_scale_factor: (!native).then(|| "1".into()),
                    locale: (!native).then(|| "en-US".into()),
                    timezone: (!native).then(|| "UTC".into()),
                    ..Default::default()
                },
            })
            .unwrap();
    }
    evidence.finish(EvidenceResult::Complete).unwrap();
}

#[test]
fn gallery_publishes_current_history_and_provenance() {
    let evidence_root = temporary_root("gallery-evidence");
    let conduitos_root = temporary_root("gallery-conduitos-evidence");
    let hears_speaks_root = temporary_root("gallery-hears-speaks-evidence");
    let two_faces_root = temporary_root("gallery-two-faces-evidence");
    let little_life_root = temporary_root("gallery-little-life-evidence");
    let site_root = temporary_root("gallery-site");
    let commit = complete_browser_evidence(&evidence_root);
    complete_conduitos_evidence(&conduitos_root, &commit);
    complete_hears_speaks_evidence(&hears_speaks_root);
    complete_two_faces_evidence(&two_faces_root);
    complete_little_life_evidence(&little_life_root);
    publish_gallery(&GalleryRequest {
        evidence_root: Some(evidence_root.clone()),
        conduitos_evidence_root: Some(conduitos_root.clone()),
        hears_speaks_evidence_root: Some(hears_speaks_root.clone()),
        two_faces_evidence_root: Some(two_faces_root.clone()),
        little_life_evidence_root: Some(little_life_root.clone()),
        site_root: site_root.clone(),
        commit: commit.clone(),
    })
    .unwrap();
    let index = fs::read_to_string(site_root.join("index.html")).unwrap();
    let scenario =
        fs::read_to_string(site_root.join("current/patchbay/overview/index.html")).unwrap();
    assert!(index.contains(&commit));
    assert!(index.contains("latest 32 published main commits"));
    assert!(index.contains("Current x86_64 ConduitOS emulator console evidence"));
    assert!(scenario.contains("1440x1000"));
    assert!(scenario.contains("Exact provenance"));
    assert!(site_root
        .join(format!("commits/{commit}/patchbay/overview.png"))
        .is_file());
    assert_eq!(
        fs::read(site_root.join("current/patchbay/overview.png")).unwrap(),
        fs::read(site_root.join(format!("commits/{commit}/patchbay/overview.png"))).unwrap()
    );
    let two_faces_page =
        fs::read_to_string(site_root.join("current/one-form-two-faces/index.html")).unwrap();
    assert!(two_faces_page.contains("One Form, Two Faces"));
    assert!(two_faces_page.contains("Pixel equality is neither expected nor claimed"));
    assert!(two_faces_page.contains("presentation/two-faces"));
    assert_eq!(
        fs::read(site_root.join("current/one-form-two-faces/native.png")).unwrap(),
        fs::read(site_root.join(format!("commits/{commit}/one-form-two-faces/native.png")))
            .unwrap()
    );
    assert!(site_root
        .join(format!("commits/{commit}/manifest.json"))
        .is_file());
    let console_page =
        fs::read_to_string(site_root.join("current/conduitos/x86_64/index.html")).unwrap();
    assert!(console_page.contains("NOT PHYSICAL HARDWARE EVIDENCE"));
    assert!(console_page.contains("freestanding-emulator"));
    assert!(console_page.contains(&commit));
    let audio_page = fs::read_to_string(site_root.join("current/hears-speaks/index.html")).unwrap();
    assert!(audio_page.contains("<audio controls"));
    assert!(audio_page.contains("not a live microphone"));
    assert_eq!(
        fs::read(site_root.join("current/hears-speaks/output.wav")).unwrap(),
        fs::read(site_root.join(format!("commits/{commit}/hears-speaks/output.wav"))).unwrap()
    );
    let life_page = fs::read_to_string(site_root.join("current/little-life/index.html")).unwrap();
    assert!(life_page.contains("t = 0"));
    assert!(life_page.contains("t = 32"));
    assert!(life_page.contains("not a native graphical renderer"));
    assert_eq!(
        fs::read(site_root.join("current/little-life/t032.png")).unwrap(),
        fs::read(site_root.join(format!("commits/{commit}/little-life/t032.png"))).unwrap()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    verify_documentation_references(&DocumentationReferenceRequest {
        workspace_root: workspace.to_path_buf(),
        site_root: Some(site_root.clone()),
        commit: Some(commit.clone()),
    })
    .unwrap();
    fs::write(
        site_root.join("current/patchbay/overview.png"),
        b"stale current image",
    )
    .unwrap();
    assert!(
        verify_documentation_references(&DocumentationReferenceRequest {
            workspace_root: workspace.to_path_buf(),
            site_root: Some(site_root.clone()),
            commit: Some(commit.clone()),
        })
        .unwrap_err()
        .contains("drifted")
    );
    fs::copy(
        site_root.join(format!("commits/{commit}/patchbay/overview.png")),
        site_root.join("current/patchbay/overview.png"),
    )
    .unwrap();
    fs::write(evidence_root.join("output-1"), b"tampered-evidence!!").unwrap();
    assert!(publish_gallery(&GalleryRequest {
        evidence_root: Some(evidence_root.clone()),
        conduitos_evidence_root: None,
        hears_speaks_evidence_root: None,
        two_faces_evidence_root: None,
        little_life_evidence_root: None,
        site_root: site_root.clone(),
        commit,
    })
    .is_err());
    assert_eq!(
        fs::read_to_string(site_root.join("index.html")).unwrap(),
        index
    );
    fs::remove_dir_all(evidence_root).unwrap();
    fs::remove_dir_all(conduitos_root).unwrap();
    fs::remove_dir_all(hears_speaks_root).unwrap();
    fs::remove_dir_all(two_faces_root).unwrap();
    fs::remove_dir_all(little_life_root).unwrap();
    fs::remove_dir_all(site_root).unwrap();
}

#[test]
fn gallery_accepts_a_verified_sibling_without_paused_patchbay_evidence() {
    let little_life_root = temporary_root("gallery-sibling-only-evidence");
    let site_root = temporary_root("gallery-sibling-only-site");
    complete_little_life_evidence(&little_life_root);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(little_life_root.join(MANIFEST_FILE)).unwrap()).unwrap();
    let commit = manifest["git_commit"].as_str().unwrap().to_owned();

    publish_gallery(&GalleryRequest {
        evidence_root: None,
        conduitos_evidence_root: None,
        hears_speaks_evidence_root: None,
        two_faces_evidence_root: None,
        little_life_evidence_root: Some(little_life_root.clone()),
        site_root: site_root.clone(),
        commit: commit.clone(),
    })
    .unwrap();

    let index = fs::read_to_string(site_root.join("index.html")).unwrap();
    assert!(index.contains(&commit));
    assert!(index.contains("Little Life"));
    assert!(!index.contains("Current Patchbay evidence"));
    assert!(!site_root.join("current/patchbay").exists());
    assert!(site_root.join("current/little-life/t032.png").is_file());
    assert!(site_root
        .join(format!("commits/{commit}/little-life/index.html"))
        .is_file());

    fs::remove_dir_all(little_life_root).unwrap();
    fs::remove_dir_all(site_root).unwrap();
}

#[test]
fn gallery_refuses_publication_without_any_evidence_input() {
    let site_root = temporary_root("gallery-no-input-site");
    assert!(publish_gallery(&GalleryRequest {
        evidence_root: None,
        conduitos_evidence_root: None,
        hears_speaks_evidence_root: None,
        two_faces_evidence_root: None,
        little_life_evidence_root: None,
        site_root: site_root.clone(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
    })
    .unwrap_err()
    .contains("at least one verified evidence input"));
    fs::remove_dir_all(site_root).unwrap();
}

#[test]
fn verifier_rejects_tampering_and_undeclared_files() {
    let root = temporary_root("verify-tamper");
    fs::write(root.join("capture.png"), b"diagnostic").unwrap();
    let mut evidence = manifest(&root);
    evidence
        .declare(output("capture", "capture.png", false))
        .unwrap();
    evidence
        .finish(EvidenceResult::DiagnosticIncomplete)
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_FILE)).unwrap()).unwrap();
    let request = VerificationRequest {
        root: root.clone(),
        commit: document["git_commit"].as_str().unwrap().to_owned(),
        result: ExpectedEvidenceResult::DiagnosticIncomplete,
        proof_id: "proof".into(),
        suite_id: "suite".into(),
    };
    fs::write(root.join("capture.png"), b"tampering!").unwrap();
    assert!(verify(&request).unwrap_err().contains("digest"));
    fs::write(root.join("capture.png"), b"diagnostic").unwrap();
    fs::write(root.join("undeclared.log"), b"not admitted").unwrap();
    assert!(verify(&request).unwrap_err().contains("undeclared"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conduitos_console_requires_emulator_provenance_and_semantic_markers() {
    let root = temporary_root("conduitos-console");
    let transcript = concat!(
        "CONDUIT_BOOT_SIGN {}\n",
        "CONDUIT_KERNEL_SIGN {}\n",
        "CONDUIT_OBSERVATORY_SNAPSHOT {}\n",
        "CONDUIT_SERIAL_PRESENT HELLO, CONDUITOS\n",
    );
    fs::write(root.join("x86_64-console.txt"), transcript).unwrap();
    let commit = exact_git_commit(Path::new(env!("CARGO_MANIFEST_DIR")), None).unwrap();
    let mut evidence = EvidenceManifest::new(
        &root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "conduitos-x86_64",
        "conduitos.prove.x86_64",
    )
    .unwrap();
    evidence
        .declare(EvidenceOutput {
            id: "conduitos.x86_64.console".into(),
            kind: EvidenceKind::ConsoleTranscript,
            path: "x86_64-console.txt".into(),
            media_type: "text/plain; charset=utf-8".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "conduitos.x86_64.p5-console@1".into(),
                step_id: Some("conduitos.prove.x86_64.semantic-terminal".into()),
                plan_id: Some("plan".into()),
                active_play_id: Some("play".into()),
                asserted_semantic_disposition: Some("terminal-validated".into()),
                proof_class: Some("freestanding-emulator".into()),
                architecture: Some("x86_64".into()),
                architecture_rung: Some("conduitos/x86_64/P5-observatory-patchbay".into()),
                emulator: Some("qemu-system-x86_64".into()),
                emulator_version: Some("QEMU emulator version 10.0.0".into()),
                machine: Some(
                    "q35-single-cpu-64m-headless-xhci-usb-kbd-usb-mouse-usb-ftdi-adlib".into(),
                ),
                firmware: Some("limine".into()),
                host_id: Some("a".repeat(64)),
                boot_id: Some("b".repeat(64)),
                kernel_artifact_id: Some(format!("conduitos-build/{commit}")),
                kernel_artifact_sha256: Some("c".repeat(64)),
                capture_trigger: Some("semantic-result-and-terminal-signs".into()),
                capture_byte_limit: Some(256 * 1024),
                physical_evidence: Some(false),
                ..Default::default()
            },
        })
        .unwrap();
    evidence.finish(EvidenceResult::Complete).unwrap();
    let request = VerificationRequest {
        root: root.clone(),
        commit,
        result: ExpectedEvidenceResult::Complete,
        proof_id: "conduitos-x86_64".into(),
        suite_id: "conduitos.prove.x86_64".into(),
    };
    verify(&request).unwrap();
    let manifest_path = root.join(MANIFEST_FILE);
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    document["outputs"][0]["physical_evidence"] = serde_json::Value::Bool(true);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    assert!(verify(&request)
        .unwrap_err()
        .contains("emulator/rung/artifact provenance"));
    fs::remove_dir_all(root).unwrap();
}
