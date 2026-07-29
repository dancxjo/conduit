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

/// Immutable, verified bytes whose ownership prevents a path or caller buffer
/// substitution between verification and loader handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedArtifactBytes {
    bytes: Vec<u8>,
    digest: ArtifactDigest,
    manifest_identity: SemanticHash,
}

/// Bounded rejection evidence for one artifact-load attempt. It carries
/// identities and sizes, never candidate bytes or hostile manifest text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactRejectionEvidence {
    pub manifest_identity: SemanticHash,
    pub candidate_digest: ArtifactDigest,
    pub observed_size: u64,
    pub reason_code: &'static str,
    pub terminal: bool,
    pub reflected_payload_bytes: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidencedArtifactVerificationError {
    pub error: HostedArtifactVerificationError,
    pub evidence: ArtifactRejectionEvidence,
}

impl VerifiedArtifactBytes {
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub const fn digest(&self) -> ArtifactDigest {
        self.digest
    }

    #[must_use]
    pub const fn manifest_identity(&self) -> SemanticHash {
        self.manifest_identity
    }

    /// Consume the gate and give a loader the exact verified allocation.
    pub fn load_with<T, E>(self, loader: impl FnOnce(&[u8]) -> Result<T, E>) -> Result<T, E> {
        loader(&self.bytes)
    }
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

/// Verify an owned allocation and seal it for one exact loader handoff.
pub fn verify_artifact_owned(
    manifest: &ArtifactManifest<'_>,
    bytes: Vec<u8>,
    target: Option<conduit_core::Id<'_>>,
    abi: Option<conduit_core::Id<'_>>,
    policy: ArtifactTrustPolicy<'_>,
    signatures: &[SignatureVerification<'_>],
) -> Result<VerifiedArtifactBytes, HostedArtifactVerificationError> {
    verify_artifact_bytes(manifest, &bytes, target, abi, policy, signatures)?;
    Ok(VerifiedArtifactBytes {
        bytes,
        digest: manifest.digest,
        manifest_identity: manifest.identity,
    })
}

/// Verify and seal owned bytes, emitting bounded terminal evidence on every
/// rejection before any loader can observe the candidate.
pub fn verify_artifact_owned_evidenced(
    manifest: &ArtifactManifest<'_>,
    bytes: Vec<u8>,
    target: Option<conduit_core::Id<'_>>,
    abi: Option<conduit_core::Id<'_>>,
    policy: ArtifactTrustPolicy<'_>,
    signatures: &[SignatureVerification<'_>],
) -> Result<VerifiedArtifactBytes, EvidencedArtifactVerificationError> {
    let digest = Sha256::digest(&bytes);
    let mut digest_bytes = [0_u8; 32];
    digest_bytes.copy_from_slice(&digest);
    let candidate_digest = ArtifactDigest::from_bytes(digest_bytes);
    let observed_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    verify_artifact_owned(manifest, bytes, target, abi, policy, signatures).map_err(|error| {
        let reason_code = match error {
            HostedArtifactVerificationError::InvalidManifest(reason) => reason.code(),
            HostedArtifactVerificationError::Artifact(reason) => reason.code(),
            HostedArtifactVerificationError::SizeOverflow => "CND-SEC-003",
        };
        EvidencedArtifactVerificationError {
            error,
            evidence: ArtifactRejectionEvidence {
                manifest_identity: manifest.identity,
                candidate_digest,
                observed_size,
                reason_code,
                terminal: true,
                reflected_payload_bytes: 0,
            },
        }
    })
}
