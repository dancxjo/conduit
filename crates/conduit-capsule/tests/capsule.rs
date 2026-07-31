use conduit_capsule::{ArtifactReference, CapsuleDocument, InlineDocument};

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
        digest: format!("sha256:{}", "ab".repeat(32)),
        byte_size: 1024,
        media_type: "application/octet-stream".to_owned(),
        license: "MIT".to_owned(),
        provenance: "fixture/build".to_owned(),
        sensitivity: "restricted".to_owned(),
        acquisition: "explicit".to_owned(),
        embedded: false,
    };
    let capsule = CapsuleDocument::new(SOURCE.to_owned(), None, None, vec![reference]).unwrap();
    capsule.validate().unwrap();

    let mut substituted = capsule;
    substituted.artifact_references[0].digest = format!("sha256:{}", "cd".repeat(32));
    assert_eq!(substituted.validate().unwrap_err().code(), "CND-CAP-006");
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
