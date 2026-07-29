use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactTrustPolicy, ArtifactVerificationReason,
    ManifestReason, SemanticHash, SignatureVerification, validate_artifact_manifest,
    verify_artifact_candidate,
};
use sha2::{Digest as _, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedArtifactVerificationError {
    InvalidManifest(ManifestReason),
    Artifact(ArtifactVerificationReason),
    SizeOverflow,
}

/// Verifies immutable bytes and their trust observations without loading or
/// executing them. Callers must complete this check before invoking a loader.
pub fn verify_artifact_bytes(
    manifest: &ArtifactManifest<'_>,
    bytes: &[u8],
    target: Option<conduit_core::Id<'_>>,
    abi: Option<conduit_core::Id<'_>>,
    policy: ArtifactTrustPolicy<'_>,
    signatures: &[SignatureVerification<'_>],
) -> Result<(), HostedArtifactVerificationError> {
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    validate_artifact_manifest(manifest, &mut scratch)
        .map_err(HostedArtifactVerificationError::InvalidManifest)?;
    let observed_size =
        u64::try_from(bytes.len()).map_err(|_| HostedArtifactVerificationError::SizeOverflow)?;
    let digest = Sha256::digest(bytes);
    let mut digest_bytes = [0_u8; 32];
    digest_bytes.copy_from_slice(&digest);
    verify_artifact_candidate(
        manifest,
        ArtifactDigest::from_bytes(digest_bytes),
        observed_size,
        target,
        abi,
        policy,
        signatures,
    )
    .map_err(HostedArtifactVerificationError::Artifact)
}
