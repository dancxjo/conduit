//! Authentication of an immutable local reading. This profile does not admit
//! collectors, erase epochs, advance a remote ACK, or create membership.
use conduit_environment_runtime::observed_record::{self, ObservedRecord};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
#[path = "native_observed_wire.rs"]
mod wire;
pub const SIGNED_BYTES: usize = observed_record::BYTES + 64;

pub fn sign(
    key: &SigningKey,
    scope: &[u8; 96],
    forms: &[u8; 32],
    record: &[u8],
) -> Result<[u8; 64], &'static str> {
    ObservedRecord::decode(record).map_err(|_| "observed-history-record")?;
    Ok(key
        .sign(&wire::record_transcript(scope, forms, record))
        .to_bytes())
}
pub fn verify(
    key: &VerifyingKey,
    scope: &[u8; 96],
    forms: &[u8; 32],
    record: &[u8],
    signature: &[u8; 64],
) -> Result<(), &'static str> {
    ObservedRecord::decode(record).map_err(|_| "observed-history-record")?;
    key.verify_strict(
        &wire::record_transcript(scope, forms, record),
        &Signature::from_bytes(signature),
    )
    .map_err(|_| "observed-history-record-signature")
}
