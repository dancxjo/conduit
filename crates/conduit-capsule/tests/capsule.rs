use conduit_capsule::{ArtifactReference, CapsuleDocument, InlineDocument};
use conduit_patchbay::Workspace;

const SOURCE: &str = "panel 0\nnode message : std/literal { value = \"capsule\" }\nnode output : display/text\ncord message.value -> output.text { capacity = 1 max_value_bytes = 64 max_queued_bytes = 64 low_watermark = 0 high_watermark = 1 pressure = block }\n";

#[test]
fn source_lock_and_presentation_keep_distinct_identities() {
    let lock = InlineDocument::new(
        "application/vnd.conduit.contract-lock+json",
        "{\"schema\":\"conduit.contract-package-lock\"}".to_owned(),
        "public",
    );
    let first = CapsuleDocument::new(
        SOURCE.to_owned(),
        Some(lock.clone()),
        Some(InlineDocument::new(
            "application/vnd.conduit.presentation+json",
            "{\"positions\":{\"message\":[0,0]}}".to_owned(),
            "public",
        )),
        Vec::new(),
    )
    .unwrap();
    let second = CapsuleDocument::new(
        SOURCE.to_owned(),
        Some(lock),
        Some(InlineDocument::new(
            "application/vnd.conduit.presentation+json",
            "{\"positions\":{\"message\":[10,20]}}".to_owned(),
            "public",
        )),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(first.program_identity, second.program_identity);
    assert_ne!(first.identity, second.identity);
    assert_eq!(
        conduit_panel::parse(&first.source).unwrap(),
        conduit_panel::parse(SOURCE).unwrap()
    );
}

#[test]
fn references_are_bounded_integrity_pinned_and_never_fetched() {
    let reference = ArtifactReference {
        role: "model".to_owned(),
        digest: format!("sha256:{}", "ab".repeat(32)),
        byte_size: 1024,
        media_type: "application/octet-stream".to_owned(),
        license: "MIT".to_owned(),
        provenance: "fixture/build".to_owned(),
        sensitivity: "restricted".to_owned(),
        acquisition: "explicit".to_owned(),
        executable: false,
        embedded_hex: None,
    };
    let capsule = CapsuleDocument::new(SOURCE.to_owned(), None, None, vec![reference]).unwrap();
    capsule.validate().unwrap();

    let mut substituted = capsule;
    substituted.artifact_references[0].digest = format!("sha256:{}", "cd".repeat(32));
    assert_eq!(substituted.validate().unwrap_err().code(), "CND-CAP-006");
}

#[test]
fn embedded_fixture_is_bounded_and_integrity_checked() {
    let bytes = b"small fixture";
    let reference = ArtifactReference {
        role: "fixture".to_owned(),
        digest: "sha256:10ce97a741d31a41b2f5cc63523b8fe9aa4d3109a74c993fe873aa984485f176"
            .to_owned(),
        byte_size: bytes.len() as u64,
        media_type: "text/plain".to_owned(),
        license: "CC0-1.0".to_owned(),
        provenance: "checked-in test fixture".to_owned(),
        sensitivity: "public".to_owned(),
        acquisition: "embedded".to_owned(),
        executable: false,
        embedded_hex: Some("736d616c6c2066697874757265".to_owned()),
    };
    let capsule = CapsuleDocument::new(SOURCE.to_owned(), None, None, vec![reference]).unwrap();
    capsule.validate().unwrap();

    let mut secret = capsule.clone();
    secret.artifact_references[0].sensitivity = "secret".to_owned();
    secret.seal().unwrap_err();

    let mut executable = capsule;
    executable.artifact_references[0].executable = true;
    executable.seal().unwrap_err();
}

#[test]
fn source_edit_creates_a_new_program_revision() {
    let first = CapsuleDocument::new(SOURCE.to_owned(), None, None, Vec::new()).unwrap();
    let second = CapsuleDocument::new(
        SOURCE.replace("capsule", "new capsule"),
        None,
        None,
        Vec::new(),
    )
    .unwrap();
    assert_ne!(first.source_revision, second.source_revision);
    assert_ne!(
        first.source_semantic_identity,
        second.source_semantic_identity
    );
    assert_ne!(first.program_identity, second.program_identity);
}

#[test]
fn source_document_capsule_and_patchbay_share_one_source_identity() {
    let document = conduit_panel::parse_document(SOURCE);
    assert_eq!(document.round_trip(), SOURCE);
    let capsule =
        CapsuleDocument::new(document.round_trip().to_owned(), None, None, Vec::new()).unwrap();
    let workspace = Workspace::new("capsule/test", document.round_trip()).unwrap();
    assert_eq!(
        workspace.semantic().source_semantic_hash.as_deref(),
        Some(capsule.source_semantic_identity.as_str())
    );
    assert_eq!(workspace.source().source, capsule.source);
    assert_ne!(workspace.presentation().identity, capsule.program_identity);
}

#[test]
fn checked_conformance_inventory_names_every_capsule_boundary() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/c4/panel-capsules.json")).unwrap();
    assert_eq!(fixture["schema"], "conduit.panel-capsule-conformance");
    assert_eq!(fixture["schema_version"], 0);
    let negatives = fixture["negative"].as_array().unwrap();
    for required in [
        "unknown-field",
        "noncanonical-reference-order",
        "artifact-digest-substitution",
        "embedded-secret",
        "embedded-executable",
        "implicit-fetch-denied",
        "archive-path-traversal-not-an-accepted-encoding",
    ] {
        assert!(
            negatives.iter().any(|value| value == required),
            "{required}"
        );
    }
}
