use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, ArtifactTrustPolicy,
    ArtifactVerificationReason, Id, SemanticHash,
};
use conduit_runtime::{HostedArtifactVerificationError, verify_artifact_bytes};
use sha2::{Digest as _, Sha256};

fn manifest(bytes: &[u8]) -> ArtifactManifest<'static> {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    let mut manifest = ArtifactManifest {
        schema_version: 0,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id("fixture/blob"),
        digest: ArtifactDigest::from_bytes(digest),
        media_type: "application/octet-stream",
        byte_size: bytes.len() as u64,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([1; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([2; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &[],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    };
    let mut scratch = [];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

#[test]
fn hosted_bytes_are_hashed_before_loader_handoff() {
    let bytes = b"fixture blob";
    let manifest = manifest(bytes);
    let policy = ArtifactTrustPolicy {
        require_signature: false,
        require_provenance_evidence: false,
        require_known_license: false,
        require_sbom: false,
        trusted_signers: &[],
    };
    assert_eq!(
        verify_artifact_bytes(&manifest, bytes, None, None, policy, &[]),
        Ok(())
    );
    assert_eq!(
        verify_artifact_bytes(&manifest, b"substituted", None, None, policy, &[]),
        Err(HostedArtifactVerificationError::Artifact(
            ArtifactVerificationReason::DigestMismatch
        ))
    );
}
