//! Explicit cross-Boot recipe permission, never membership or Start authority.
//!
//! Verification relies on trusted, independently installed receiver facts. It
//! does not read media, attest rollback resistance, or discover remote revocation.
//! A disconnected Host cannot infer revocation it has not received. Before any
//! effect, Root must still inspect current inventory, prepare/admit the complete
//! workset and issue fresh local capabilities and Plan/Play/session identities.
use crate::{ContinuationCeilings, MAX_LIFECYCLE_ID_BYTES};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONTINUATION_INTENT_SCHEMA: &str = "conduit.body/continuation-intent@1";

/// Public signed recipe identity. No Boot command, reservation or Started token
/// is persisted here. The detached signature covers the complete transcript.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationIntent<'a> {
    pub schema: &'a str,
    pub body_id: &'a str,
    pub part_id: &'a str,
    pub host_id: &'a str,
    pub credential_id: &'a str,
    /// Owner's retained membership revision at authorization, not OfferGeneration.
    pub membership_generation: u64,
    pub intent_generation: u64,
    pub issuer_host: &'a str,
    pub issuer_key: [u8; 32],
    pub source_digest: [u8; 32],
    pub workset_digest: [u8; 32],
    pub image_id: &'a str,
    pub configuration_id: &'a str,
    pub ceilings: ContinuationCeilings,
}
impl ContinuationIntent<'_> {
    pub fn signing_transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/continuation-intent@1\0");
        for text in [
            self.schema,
            self.body_id,
            self.part_id,
            self.host_id,
            self.credential_id,
            self.issuer_host,
            self.image_id,
            self.configuration_id,
        ] {
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
        hash.update(self.membership_generation.to_le_bytes());
        hash.update(self.intent_generation.to_le_bytes());
        hash.update(self.issuer_key);
        hash.update(self.source_digest);
        hash.update(self.workset_digest);
        for word in self.ceilings.words() {
            hash.update(word.to_le_bytes());
        }
        hash.finalize().into()
    }
}

/// Explicit separately installed opt-in. Existing Seed membership ownership and
/// Boot-scoped execution delegation alone cannot supply this permission.
#[derive(Clone, Copy)]
pub struct ContinuationPolicy<'a> {
    pub issuer_host: &'a str,
    pub issuer_key: [u8; 32],
    pub maximum: ContinuationCeilings,
}

/// TRUST BOUNDARY: caller-owned installed facts, NOT verified media evidence.
/// Root must establish these through its authenticated durable-state lifecycle,
/// independently of the candidate intent. Copying fields from that intent makes
/// the comparison meaningless. This DTO certifies no flash read, durable commit,
/// current owner-confirmed membership, or protection against rollback.
/// Generations are the latest *locally accepted* authorization state; unknown
/// remote revocations remain unknown. IMAGE/configuration and workset digests
/// must originate in independently checked installed construction/Source.
pub struct TrustedInstalledContinuationFacts<'a> {
    pub body_id: &'a str,
    pub part_id: &'a str,
    pub host_id: &'a str,
    pub credential_id: &'a str,
    pub membership_generation: u64,
    pub intent_generation: u64,
    pub source_digest: [u8; 32],
    pub workset_digest: [u8; 32],
    pub image_id: &'a str,
    pub configuration_id: &'a str,
    pub actual_boot: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuationIntentRefusal {
    MissingPolicy,
    Bounds,
    Scope,
    Generation,
    Signature,
}

/// Opaque validation result tied to the actual Boot used for this check.
/// It is not accepted by existing membership, preparation, capability or Start
/// APIs and cannot be deserialized. Reconstructing execution remains separate.
/// ```compile_fail
/// use conduit_body::VerifiedContinuationIntent;
/// // Neither a decoded intent nor copied identity can construct this result.
/// let forged = VerifiedContinuationIntent { intent: todo!(), actual_boot: "boot/new" };
/// ```
pub struct VerifiedContinuationIntent<'a> {
    intent: ContinuationIntent<'a>,
    actual_boot: &'a str,
}
impl<'a> VerifiedContinuationIntent<'a> {
    pub fn intent(&self) -> &ContinuationIntent<'a> {
        &self.intent
    }
    pub fn actual_boot(&self) -> &'a str {
        self.actual_boot
    }
}

pub fn verify_continuation_intent<'a>(
    intent: ContinuationIntent<'a>,
    signature: &[u8; 64],
    policy: Option<ContinuationPolicy<'_>>,
    installed: TrustedInstalledContinuationFacts<'a>,
) -> Result<VerifiedContinuationIntent<'a>, ContinuationIntentRefusal> {
    use ContinuationIntentRefusal as E;
    let policy = policy.ok_or(E::MissingPolicy)?;
    let valid_id = |s: &str| !s.is_empty() && s.len() <= MAX_LIFECYCLE_ID_BYTES;
    if ![
        intent.body_id,
        intent.part_id,
        intent.host_id,
        intent.credential_id,
        intent.issuer_host,
        intent.image_id,
        intent.configuration_id,
        installed.actual_boot,
        policy.issuer_host,
    ]
    .into_iter()
    .all(valid_id)
        || !intent.ceilings.has_finite_bounds()
        || !policy.maximum.has_finite_bounds()
        || !intent.ceilings.within(policy.maximum)
        || intent.source_digest == [0; 32]
        || intent.workset_digest == [0; 32]
        || policy.issuer_key == [0; 32]
    {
        return Err(E::Bounds);
    }
    if intent.schema != CONTINUATION_INTENT_SCHEMA
        || intent.body_id != installed.body_id
        || intent.part_id != installed.part_id
        || intent.host_id != installed.host_id
        || intent.credential_id != installed.credential_id
        || intent.source_digest != installed.source_digest
        || intent.workset_digest != installed.workset_digest
        || intent.image_id != installed.image_id
        || intent.configuration_id != installed.configuration_id
        || intent.issuer_host != policy.issuer_host
        || intent.issuer_key != policy.issuer_key
    {
        return Err(E::Scope);
    }
    if intent.membership_generation == 0
        || intent.intent_generation == 0
        || intent.membership_generation != installed.membership_generation
        || intent.intent_generation != installed.intent_generation
    {
        return Err(E::Generation);
    }
    VerifyingKey::from_bytes(&policy.issuer_key)
        .map_err(|_| E::Signature)?
        .verify_strict(
            &intent.signing_transcript(),
            &Signature::from_bytes(signature),
        )
        .map_err(|_| E::Signature)?;
    Ok(VerifiedContinuationIntent {
        intent,
        actual_boot: installed.actual_boot,
    })
}
