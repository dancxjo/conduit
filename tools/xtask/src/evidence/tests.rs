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
                machine: Some("q35-single-cpu-64m-headless-xhci-usb-kbd-adlib".into()),
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

#[test]
fn gallery_publishes_current_history_and_provenance() {
    let evidence_root = temporary_root("gallery-evidence");
    let conduitos_root = temporary_root("gallery-conduitos-evidence");
    let site_root = temporary_root("gallery-site");
    let commit = complete_browser_evidence(&evidence_root);
    complete_conduitos_evidence(&conduitos_root, &commit);
    publish_gallery(&GalleryRequest {
        evidence_root: evidence_root.clone(),
        conduitos_evidence_root: Some(conduitos_root.clone()),
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
    assert!(site_root
        .join(format!("commits/{commit}/manifest.json"))
        .is_file());
    let console_page =
        fs::read_to_string(site_root.join("current/conduitos/x86_64/index.html")).unwrap();
    assert!(console_page.contains("NOT PHYSICAL HARDWARE EVIDENCE"));
    assert!(console_page.contains("freestanding-emulator"));
    assert!(console_page.contains(&commit));
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
        evidence_root: evidence_root.clone(),
        conduitos_evidence_root: None,
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
                machine: Some("q35-single-cpu-64m-headless-xhci-usb-kbd-adlib".into()),
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
