use super::*;

#[test]
fn expanded_capacity_keeps_retained_manifests_verifiable() {
    let root =
        std::env::temp_dir().join(format!("conduit-evidence-capacity-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("receipt.txt"), b"observed").unwrap();
    let mut evidence = EvidenceManifest::new(
        &root,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "capacity-proof",
        "capacity-suite",
    )
    .unwrap();
    evidence
        .declare(EvidenceOutput {
            id: "receipt".into(),
            kind: EvidenceKind::ConsoleTranscript,
            path: "receipt.txt".into(),
            media_type: "text/plain".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "capacity/retained@1".into(),
                ..Default::default()
            },
        })
        .unwrap();
    evidence.finish(EvidenceResult::Complete).unwrap();
    let path = root.join(MANIFEST_FILE);
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(document["limits"]["maximum_outputs"], MAX_EVIDENCE_OUTPUTS);
    let request = VerificationRequest {
        root: root.clone(),
        commit: document["git_commit"].as_str().unwrap().into(),
        result: ExpectedEvidenceResult::Complete,
        proof_id: "capacity-proof".into(),
        suite_id: "capacity-suite".into(),
    };
    verify(&request).unwrap();
    document["limits"]["maximum_outputs"] = LEGACY_MAX_EVIDENCE_OUTPUTS.into();
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    verify(&request).unwrap();
    document["limits"]["maximum_outputs"] = (MAX_EVIDENCE_OUTPUTS + 1).into();
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(verify(&request).is_err());
    fs::remove_dir_all(root).unwrap();
}
