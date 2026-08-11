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
