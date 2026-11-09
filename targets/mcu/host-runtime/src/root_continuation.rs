//! Original Root acceptance of owner permission, not current-Boot membership.
//! Local generations are acceptance counters, never global Body revisions.
use super::*;
use crate::continuation_artifact::{
    self as artifact, ACCEPTANCE_BYTES, ARTIFACT_BYTES, PREFIX_BYTES,
};
use conduit_body as b;
use ed25519_dalek::{Signature, Signer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "root_continuation_storage.rs"]
mod storage;
const MAGIC: &[u8; 16] = b"CONDUIT_CACK@1\0\0";
const COMMIT: &[u8; 16] = b"CONT_ACCEPTED@1\0";
const HEADER: usize = 20;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationAcceptanceRefusal {
    Scope,
    Carrier,
    Generation,
    Signature,
    Encoding,
    Bounds,
    Media,
    Uncertain,
    RequiresErase,
    Artifact(artifact::Refusal),
}
use ContinuationAcceptanceRefusal as E;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Anchor {
    schema: u16,
    credential: b::MembershipCredential,
    receipt_digest: [u8; 32],
    artifact_digest: [u8; 32],
    membership_generation: u64,
    intent_generation: u64,
    signature: [[u8; 32]; 2],
}
/// A Root signature verified against independently retained Seed key/owner/body.
/// This preserves a previously accepted credential, never a new-Boot receipt.
/// No anti-rollback hardware or knowledge of remote revocations is claimed.
pub struct InstalledContinuationAcceptance {
    anchor: Anchor,
    prefix_digest: [u8; 32],
}
impl InstalledContinuationAcceptance {
    pub fn credential(&self) -> &b::MembershipCredential {
        &self.anchor.credential
    }
    pub fn membership_generation(&self) -> u64 {
        self.anchor.membership_generation
    }
    pub fn intent_generation(&self) -> u64 {
        self.anchor.intent_generation
    }
}
/// Owner permission selected by this original Root. This is not installed until
/// the cold transaction completes and reads back all exact physical bytes.
pub struct PreparedContinuationInstall {
    anchor: Anchor,
    previous: Option<[u8; 32]>,
}
fn signature(bytes: [[u8; 32]; 2]) -> Signature {
    let mut out = [0; 64];
    out[..32].copy_from_slice(&bytes[0]);
    out[32..].copy_from_slice(&bytes[1]);
    Signature::from_bytes(&out)
}
fn split(bytes: [u8; 64]) -> [[u8; 32]; 2] {
    [
        bytes[..32].try_into().expect("fixed signature"),
        bytes[32..].try_into().expect("fixed signature"),
    ]
}
fn update(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}
fn transcript(seed: &b::MemberBootstrap<'_>, anchor: &Anchor) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"conduit.root/accepted-continuation@1\0");
    for id in [
        seed.body_id.as_str(),
        seed.host_id.as_str(),
        seed.owner.host_id.as_str(),
        anchor.credential.body_id.as_str(),
        anchor.credential.part_id.as_str(),
        anchor.credential.credential_id.as_str(),
        anchor.credential.host_id.as_str(),
        anchor.credential.boot_id.as_str(),
    ] {
        update(&mut h, id);
    }
    h.update(seed.owner.verifying_key);
    h.update(anchor.schema.to_le_bytes());
    h.update(anchor.credential.issued_at_millis.to_le_bytes());
    h.update(anchor.receipt_digest);
    h.update(anchor.artifact_digest);
    h.update(anchor.membership_generation.to_le_bytes());
    h.update(anchor.intent_generation.to_le_bytes());
    h.finalize().into()
}
fn encode_anchor(anchor: &Anchor, out: &mut [u8; ACCEPTANCE_BYTES]) -> Result<(), E> {
    out.fill(0xff);
    let n = serde_json_core::to_slice(anchor, &mut out[HEADER..ACCEPTANCE_BYTES - COMMIT.len()])
        .map_err(|_| E::Bounds)?;
    out[..16].copy_from_slice(MAGIC);
    out[16..HEADER].copy_from_slice(&(n as u32).to_le_bytes());
    out[ACCEPTANCE_BYTES - COMMIT.len()..].copy_from_slice(COMMIT);
    Ok(())
}
fn decode_anchor(input: &[u8], scratch: &mut [u8; ACCEPTANCE_BYTES]) -> Result<Anchor, E> {
    if input.len() != ACCEPTANCE_BYTES
        || &input[..16] != MAGIC
        || &input[ACCEPTANCE_BYTES - 16..] != COMMIT
    {
        return Err(E::Uncertain);
    }
    let n = u32::from_le_bytes(input[16..HEADER].try_into().map_err(|_| E::Encoding)?) as usize;
    if n == 0 || n > ACCEPTANCE_BYTES - HEADER - 16 {
        return Err(E::Bounds);
    }
    let (anchor, used) = serde_json_core::from_slice::<Anchor>(&input[HEADER..HEADER + n])
        .map_err(|_| E::Encoding)?;
    if used != n {
        return Err(E::Encoding);
    }
    encode_anchor(&anchor, scratch)?;
    if scratch.as_slice() != input {
        return Err(E::Encoding);
    }
    Ok(anchor)
}
impl<const N: usize> Root<'_, N> {
    fn check_anchor(&self, anchor: &Anchor) -> Result<(), E> {
        let credential = &anchor.credential;
        if anchor.schema != 1
            || anchor.membership_generation == 0
            || anchor.intent_generation == 0
            || anchor.receipt_digest == [0; 32]
            || anchor.artifact_digest == [0; 32]
            || credential.body_id != self.seed.body_id
            || credential.host_id != self.resources.host().host_id
            || [
                credential.part_id.as_str(),
                credential.credential_id.as_str(),
                credential.boot_id.as_str(),
            ]
            .iter()
            .any(|s| s.is_empty() || s.len() > b::MAX_LIFECYCLE_ID_BYTES)
            || self
                .seed
                .admission
                .credential()
                .is_some_and(|old| old.part_id != credential.part_id)
        {
            return Err(E::Scope);
        }
        self.key
            .verifying_key()
            .verify_strict(
                &transcript(&self.seed, anchor),
                &signature(anchor.signature),
            )
            .map_err(|_| E::Signature)
    }
    /// Verify the physically read prefix. Only the opaque result can restore
    /// acceptance facts; decoding an artifact or copying its counters cannot.
    pub fn read_continuation_acceptance(
        &self,
        prefix: &[u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
    ) -> Result<InstalledContinuationAcceptance, E> {
        let anchor = decode_anchor(
            &prefix[ARTIFACT_BYTES..],
            (&mut scratch[..ACCEPTANCE_BYTES])
                .try_into()
                .map_err(|_| E::Bounds)?,
        )?;
        self.check_anchor(&anchor)?;
        if Sha256::digest(&prefix[..ARTIFACT_BYTES]).as_slice() != anchor.artifact_digest {
            return Err(E::Signature);
        }
        let candidate = artifact::decode_installed(prefix, scratch).map_err(E::Artifact)?;
        self.check_artifact_credential(&candidate, &anchor.credential)?;
        if candidate.intent.membership_generation != anchor.membership_generation
            || candidate.intent.intent_generation != anchor.intent_generation
        {
            return Err(E::Generation);
        }
        let owner = ed25519_dalek::VerifyingKey::from_bytes(&self.seed.owner.verifying_key)
            .map_err(|_| E::Signature)?;
        owner
            .verify_strict(
                &candidate.intent.signing_transcript(),
                &Signature::from_bytes(&candidate.signature_bytes()),
            )
            .map_err(|_| E::Signature)?;
        Ok(InstalledContinuationAcceptance {
            anchor,
            prefix_digest: Sha256::digest(prefix).into(),
        })
    }
    fn check_artifact_credential(
        &self,
        candidate: &artifact::Artifact<'_>,
        credential: &b::MembershipCredential,
    ) -> Result<(), E> {
        let i = &candidate.intent;
        if i.body_id != credential.body_id.as_str()
            || i.part_id != credential.part_id.as_str()
            || i.credential_id != credential.credential_id.as_str()
            || i.host_id != credential.host_id.as_str()
            || i.issuer_host != self.seed.owner.host_id.as_str()
            || i.issuer_key != self.seed.owner.verifying_key
            || credential.body_id != self.seed.body_id
            || credential.host_id != self.resources.host().host_id
        {
            return Err(E::Scope);
        }
        Ok(())
    }
    /// Restore ONLY the exact committed retained credential. A current-Boot
    /// online receipt is deliberately not created. Root owns the same original
    /// continuity secret; no identity is imported from an unrelated Host.
    pub fn restore_continuation_acceptance(
        &mut self,
        accepted: &InstalledContinuationAcceptance,
    ) -> Result<(), E> {
        self.check_anchor(&accepted.anchor)?;
        if self
            .receipt
            .as_ref()
            .is_some_and(|r| r != &accepted.anchor.credential)
            || self.input.is_retained()
            || !self.resources.admissions().is_empty()
            || !self.resources.assignments().is_empty()
        {
            return Err(E::Scope);
        }
        self.seed
            .replace_retained_credential(accepted.anchor.credential.clone());
        Ok(())
    }
    /// Select owner permission against an ACTUAL accepted return, independently
    /// checked Source/recipes, and the prior locally verified acceptance state.
    /// No copied candidate generation is promoted into a trusted installed fact.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_continuation_install(
        &self,
        candidate: &[u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
        source: &[u8],
        pins: &[b::ContinuationBundleEntry<'_>],
        previous: Option<&InstalledContinuationAcceptance>,
    ) -> Result<PreparedContinuationInstall, E> {
        self.prepare_continuation_install_digest(
            candidate,
            scratch,
            Sha256::digest(source).into(),
            pins,
            previous,
        )
    }
    /// Actual Moon entry: source identity is the independent generated Make pin,
    /// not an intent field supplied by the caller's incoming artifact.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_continuation_install_digest(
        &self,
        candidate: &[u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
        source_digest: [u8; 32],
        pins: &[b::ContinuationBundleEntry<'_>],
        previous: Option<&InstalledContinuationAcceptance>,
    ) -> Result<PreparedContinuationInstall, E> {
        let preparation = self.config.preparation.ok_or(E::Scope)?;
        let execution = self.config.execution.ok_or(E::Scope)?;
        let max = execution.continuation.ok_or(E::Scope)?;
        if !preparation.has_finite_bounds()
            || !max.has_finite_bounds()
            || execution.maximum_active_worksets == 0
            || !matches!(
                preparation.authorizer,
                b::HostPreparationAuthorizer::SeedOwner
            )
            || !matches!(execution.authorizer, b::HostExecutionAuthorizer::SeedOwner)
        {
            return Err(E::Scope);
        }

        if self.input.is_retained()
            || !self.resources.admissions().is_empty()
            || !self.resources.assignments().is_empty()
        {
            return Err(E::Scope);
        }
        let credential = self.receipt.as_ref().ok_or(E::Scope)?;
        let receipt_digest = self.receipt_digest.ok_or(E::Scope)?;
        if credential.body_id != self.seed.body_id
            || credential.host_id != self.resources.host().host_id
            || credential.boot_id != self.resources.host().boot_id
        {
            return Err(E::Scope);
        }
        if let Some(previous) = previous {
            self.check_anchor(&previous.anchor)?;
        }
        let membership_generation = match previous {
            None => 1,
            Some(p) if p.anchor.credential == *credential => p.anchor.membership_generation,
            Some(p) => p
                .anchor
                .membership_generation
                .checked_add(1)
                .ok_or(E::Generation)?,
        };
        let intent_generation = match previous {
            None => 1,
            Some(p) => p
                .anchor
                .intent_generation
                .checked_add(1)
                .ok_or(E::Generation)?,
        };
        let artifact = artifact::decode(candidate, scratch).map_err(E::Artifact)?;
        self.check_artifact_credential(&artifact, credential)?;
        if artifact.intent.ceilings.assignment_bytes > preparation.maximum_assignment_bytes
            || artifact.intent.ceilings.runtime_bytes > preparation.maximum_runtime_state_bytes
            || artifact.intent.ceilings.active_worksets
                > u32::from(execution.maximum_active_worksets)
        {
            return Err(E::Bounds);
        }

        if artifact.intent.image_id != self.config.image_id
            || artifact.intent.configuration_id != self.config.configuration_id
        {
            return Err(E::Scope);
        }

        let workset_digest =
            b::continuation_bundle_digest(source_digest, pins).map_err(|_| E::Scope)?;
        let verified = b::verify_continuation_intent(
            artifact.intent,
            &artifact.signature_bytes(),
            Some(b::ContinuationPolicy {
                issuer_host: self.seed.owner.host_id.as_str(),
                issuer_key: self.seed.owner.verifying_key,
                maximum: max,
            }),
            b::TrustedInstalledContinuationFacts {
                body_id: self.seed.body_id.as_str(),
                part_id: credential.part_id.as_str(),
                host_id: self.resources.host().host_id.as_str(),
                credential_id: credential.credential_id.as_str(),
                membership_generation,
                intent_generation,
                source_digest,
                workset_digest,
                image_id: self.config.image_id,
                configuration_id: self.config.configuration_id,
                actual_boot: self.resources.host().boot_id.as_str(),
            },
        )
        .map_err(|e| match e {
            b::ContinuationIntentRefusal::Generation => E::Generation,
            b::ContinuationIntentRefusal::Signature => E::Signature,
            b::ContinuationIntentRefusal::Bounds => E::Bounds,
            _ => E::Scope,
        })?;
        b::check_installed_continuation_bundle(&verified, source_digest, pins)
            .map_err(|_| E::Scope)?;
        let mut anchor = Anchor {
            schema: 1,
            credential: credential.clone(),
            receipt_digest,
            artifact_digest: Sha256::digest(&candidate[..ARTIFACT_BYTES]).into(),
            membership_generation,
            intent_generation,
            signature: [[0; 32]; 2],
        };
        anchor.signature = split(self.key.sign(&transcript(&self.seed, &anchor)).to_bytes());
        Ok(PreparedContinuationInstall {
            anchor,
            previous: previous.map(|p| p.prefix_digest),
        })
    }
}

impl PreparedContinuationInstall {
    /// Export the original Root-selected public sealed image for a reviewed cold
    /// ROM-loader install. Bytes are NOT a durable commit receipt or flash grant.
    /// New Boot must verify the actually read media before adopting its facts.
    pub fn encode_selected_prefix(
        &self,
        candidate: &[u8; PREFIX_BYTES],
        output: &mut [u8; PREFIX_BYTES],
    ) -> Result<(), E> {
        if Sha256::digest(&candidate[..ARTIFACT_BYTES]).as_slice() != self.anchor.artifact_digest {
            return Err(E::Scope);
        }
        output[..ARTIFACT_BYTES].copy_from_slice(&candidate[..ARTIFACT_BYTES]);
        encode_anchor(
            &self.anchor,
            (&mut output[ARTIFACT_BYTES..])
                .try_into()
                .map_err(|_| E::Bounds)?,
        )
    }
}
#[path = "root_continuation_boot.rs"]
mod boot;
#[path = "root_continuation_handoff.rs"]
mod handoff;
pub use boot::ContinuationParts;
