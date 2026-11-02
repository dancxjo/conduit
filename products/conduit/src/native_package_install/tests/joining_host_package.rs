//! Existing native consumer accepts the generic producer's exact bytes.
use super::super::*;
#[path = "../../../../../make/body/tests/common/joining_host_package.rs"]
mod common;

#[test]
fn generic_producer_matches_existing_native_parser_and_release_aggregate() {
    let (release, spore, mut invitation, payloads) = common::fixture(&["conduit", "install.sh"]);
    common::routed(&mut invitation);
    let mut bytes = Vec::new();
    let result = conduit_body_make::write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(std::io::Cursor::new(payloads[p].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    let artifact = BodyBoundArtifactIdentity {
        target_id: spore.manifest.target.clone(),
        image_id: spore.manifest.image_id.clone(),
        image_content_sha256: result.image_content_sha256,
        spore_id: spore.manifest.spore_id.clone(),
        artifact_content_sha256: result.artifact_content_sha256,
        artifact_bytes: result.artifact_bytes,
    };
    assert_eq!(digest(&bytes), artifact.artifact_content_sha256);
    let mut entries = read_stored_zip(&bytes).unwrap();
    let index = entries
        .iter()
        .position(|e| e.name == PROVISION_PATH)
        .unwrap();
    let provision_bytes = entries.remove(index).bytes;
    let raw: serde_json::Value = serde_json::from_slice(&provision_bytes).unwrap();
    assert_eq!(
        raw["invitation_provision"]["rendezvous"],
        serde_json::to_value(&invitation.rendezvous).unwrap()
    );
    assert_eq!(
        raw["invitation_provision"]["rendezvous_candidates"],
        serde_json::json!([])
    );
    assert_eq!(
        raw["portable_invitation"],
        serde_json::to_value(&invitation).unwrap()
    );
    let provision: Provision = serde_json::from_slice(&provision_bytes).unwrap();
    validate_provision(&provision, &artifact).unwrap();
    for entry in &entries {
        assert_eq!(entry.bytes, payloads[&entry.name]);
    }
    let verified = verify_release_manifest(&provision, &artifact, &entries).unwrap();
    assert_eq!(
        verified.manifest().source_identity,
        spore.image.manifest.source_identity
    );
    assert_ne!(
        verified.manifest().source_identity,
        spore.manifest.source_identity
    );
    let root = super::test_root("ordinary-producer-make-source");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("spore.zip");
    fs::write(&source, &bytes).unwrap();
    let prepared = prepare(&source, &artifact, &root.join("state")).unwrap();
    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(&prepared.manifest).unwrap()).unwrap();
    assert_eq!(installed, serde_json::to_value(release.manifest()).unwrap());
    assert_eq!(installed["source_identity"], "source/prior-reviewed-make");
    assert_eq!(spore.manifest.source_identity, "sign/original-body-birth");
    fs::remove_dir_all(root).unwrap();
}
