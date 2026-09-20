//! Bounded COSE_Sign1 protection for canonical rendezvous descriptor bytes.
//!
//! The signer and verifier are application policy. A successful verification
//! establishes only the returned attribution; it grants no membership,
//! authority, freshness, admission, planning eligibility, or Line readiness.

use core::ptr;

use minicbor::{data::Tag, decode::Decoder, encode::write::Cursor, Encoder};

use crate::{
    decode_running_host_rendezvous_cbor, encode_running_host_rendezvous_cbor,
    BorrowedRunningHostRendezvousDescriptor, RendezvousCborRefusal,
    RunningHostRendezvousDescriptor, MAX_RENDEZVOUS_CBOR_BYTES,
};

pub const RENDEZVOUS_COSE_SIGN1_PROFILE: &str = "conduit.host/rendezvous-cose-sign1@1";
pub const MAX_RENDEZVOUS_COSE_KEY_ID_BYTES: usize = 32;
pub const RENDEZVOUS_COSE_SIGNATURE_BYTES: usize = 64;
/// Exact largest tagged Sign1 object admitted by this profile.
pub const MAX_RENDEZVOUS_COSE_SIGN1_BYTES: usize = 3_768;
pub const MAX_RENDEZVOUS_COSE_SIG_STRUCTURE_BYTES: usize = 3_728;

const COSE_SIGN1_TAG: u64 = 18;
const COSE_HEADER_ALGORITHM: u8 = 1;
const COSE_HEADER_KEY_ID: u8 = 4;
const COSE_ALGORITHM_EDDSA: i8 = -8;
const SIGNATURE_CONTEXT: &str = "Signature1";
const MAX_PROTECTED_HEADER_BYTES: usize = 38;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousCoseRefusal {
    EncodedBound,
    Truncated,
    Malformed,
    NonCanonical,
    UnsupportedProfile,
    UnsupportedAlgorithm,
    KeyIdBound,
    SignatureBound,
    SigningRejected,
    VerificationRejected,
    Descriptor(RendezvousCborRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendezvousCosePolicyRefusal;

/// Application-owned signing policy. Secret key material never crosses this API.
pub trait RendezvousCoseSigner {
    fn key_id(&self) -> &[u8];

    fn sign(
        &self,
        sig_structure: &[u8],
        signature: &mut [u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
    ) -> Result<(), RendezvousCosePolicyRefusal>;
}

/// Application-owned verification policy and attribution semantics.
pub trait RendezvousCoseVerifier {
    type Attribution;

    fn verify(
        &self,
        key_id: &[u8],
        sig_structure: &[u8],
        signature: &[u8; RENDEZVOUS_COSE_SIGNATURE_BYTES],
    ) -> Result<Self::Attribution, RendezvousCosePolicyRefusal>;
}

pub struct VerifiedRendezvousCoseSign1<'a, Attribution> {
    pub descriptor: BorrowedRunningHostRendezvousDescriptor<'a>,
    pub attribution: Attribution,
    pub key_id: &'a [u8],
}

struct Sensitive<const N: usize>([u8; N]);

impl<const N: usize> Drop for Sensitive<N> {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: `byte` is a valid exclusive reference. Volatile writes
            // prevent the compiler from eliding this best-effort stack erase.
            unsafe { ptr::write_volatile(byte, 0) };
        }
    }
}

pub fn encode_running_host_rendezvous_cose_sign1(
    descriptor: &RunningHostRendezvousDescriptor,
    signer: &impl RendezvousCoseSigner,
    output: &mut [u8],
) -> Result<usize, RendezvousCoseRefusal> {
    let key_id = signer.key_id();
    validate_key_id(key_id)?;

    let mut payload = Sensitive([0; MAX_RENDEZVOUS_CBOR_BYTES]);
    let payload_len = encode_running_host_rendezvous_cbor(descriptor, &mut payload.0)
        .map_err(RendezvousCoseRefusal::Descriptor)?;
    let mut protected = [0; MAX_PROTECTED_HEADER_BYTES];
    let protected_len = encode_protected(key_id, &mut protected)?;
    let mut sig_structure = Sensitive([0; MAX_RENDEZVOUS_COSE_SIG_STRUCTURE_BYTES]);
    let sig_structure_len = encode_sig_structure(
        &protected[..protected_len],
        &payload.0[..payload_len],
        &mut sig_structure.0,
    )?;
    let mut signature = Sensitive([0; RENDEZVOUS_COSE_SIGNATURE_BYTES]);
    signer
        .sign(&sig_structure.0[..sig_structure_len], &mut signature.0)
        .map_err(|_| RendezvousCoseRefusal::SigningRejected)?;

    let output_len = output.len().min(MAX_RENDEZVOUS_COSE_SIGN1_BYTES);
    let mut cursor = Cursor::new(&mut output[..output_len]);
    Encoder::new(&mut cursor)
        .tag(Tag::new(COSE_SIGN1_TAG))
        .and_then(|encoder| encoder.array(4))
        .and_then(|encoder| encoder.bytes(&protected[..protected_len]))
        .and_then(|encoder| encoder.map(0))
        .and_then(|encoder| encoder.bytes(&payload.0[..payload_len]))
        .and_then(|encoder| encoder.bytes(&signature.0))
        .map_err(|_| RendezvousCoseRefusal::EncodedBound)?;
    Ok(cursor.position())
}

pub fn decode_running_host_rendezvous_cose_sign1<'a, V: RendezvousCoseVerifier>(
    input: &'a [u8],
    now_millis: u64,
    verifier: &V,
) -> Result<VerifiedRendezvousCoseSign1<'a, V::Attribution>, RendezvousCoseRefusal> {
    if input.len() > MAX_RENDEZVOUS_COSE_SIGN1_BYTES {
        return Err(RendezvousCoseRefusal::EncodedBound);
    }
    let mut decoder = Decoder::new(input);
    let tag = decoder.tag().map_err(decode_error)?;
    if tag.as_u64() != COSE_SIGN1_TAG {
        return Err(RendezvousCoseRefusal::UnsupportedProfile);
    }
    if decoder.array().map_err(decode_error)? != Some(4) {
        return Err(RendezvousCoseRefusal::Malformed);
    }
    let protected = decoder.bytes().map_err(decode_error)?;
    let key_id = decode_protected(protected)?;
    if decoder.map().map_err(decode_error)? != Some(0) {
        return Err(RendezvousCoseRefusal::NonCanonical);
    }
    let payload = decoder.bytes().map_err(decode_error)?;
    let signature: &[u8; RENDEZVOUS_COSE_SIGNATURE_BYTES] = decoder
        .bytes()
        .map_err(decode_error)?
        .try_into()
        .map_err(|_| RendezvousCoseRefusal::SignatureBound)?;
    if decoder.position() != input.len() {
        return Err(RendezvousCoseRefusal::Malformed);
    }

    let mut canonical_protected = [0; MAX_PROTECTED_HEADER_BYTES];
    let canonical_len = encode_protected(key_id, &mut canonical_protected)?;
    if protected != &canonical_protected[..canonical_len] {
        return Err(RendezvousCoseRefusal::NonCanonical);
    }
    let descriptor = decode_running_host_rendezvous_cbor(payload, now_millis)
        .map_err(RendezvousCoseRefusal::Descriptor)?;
    let mut sig_structure = Sensitive([0; MAX_RENDEZVOUS_COSE_SIG_STRUCTURE_BYTES]);
    let sig_structure_len = encode_sig_structure(protected, payload, &mut sig_structure.0)?;
    let attribution = verifier
        .verify(key_id, &sig_structure.0[..sig_structure_len], signature)
        .map_err(|_| RendezvousCoseRefusal::VerificationRejected)?;
    Ok(VerifiedRendezvousCoseSign1 {
        descriptor,
        attribution,
        key_id,
    })
}

fn validate_key_id(key_id: &[u8]) -> Result<(), RendezvousCoseRefusal> {
    if key_id.is_empty() || key_id.len() > MAX_RENDEZVOUS_COSE_KEY_ID_BYTES {
        Err(RendezvousCoseRefusal::KeyIdBound)
    } else {
        Ok(())
    }
}

fn encode_protected(key_id: &[u8], output: &mut [u8]) -> Result<usize, RendezvousCoseRefusal> {
    validate_key_id(key_id)?;
    let mut cursor = Cursor::new(output);
    Encoder::new(&mut cursor)
        .map(2)
        .and_then(|encoder| encoder.u8(COSE_HEADER_ALGORITHM))
        .and_then(|encoder| encoder.i8(COSE_ALGORITHM_EDDSA))
        .and_then(|encoder| encoder.u8(COSE_HEADER_KEY_ID))
        .and_then(|encoder| encoder.bytes(key_id))
        .map_err(|_| RendezvousCoseRefusal::EncodedBound)?;
    Ok(cursor.position())
}

fn decode_protected(protected: &[u8]) -> Result<&[u8], RendezvousCoseRefusal> {
    let mut decoder = Decoder::new(protected);
    if decoder.map().map_err(decode_error)? != Some(2)
        || decoder.u8().map_err(decode_error)? != COSE_HEADER_ALGORITHM
    {
        return Err(RendezvousCoseRefusal::UnsupportedProfile);
    }
    if decoder.i8().map_err(decode_error)? != COSE_ALGORITHM_EDDSA {
        return Err(RendezvousCoseRefusal::UnsupportedAlgorithm);
    }
    if decoder.u8().map_err(decode_error)? != COSE_HEADER_KEY_ID {
        return Err(RendezvousCoseRefusal::UnsupportedProfile);
    }
    let key_id = decoder.bytes().map_err(decode_error)?;
    validate_key_id(key_id)?;
    if decoder.position() != protected.len() {
        return Err(RendezvousCoseRefusal::Malformed);
    }
    Ok(key_id)
}

fn encode_sig_structure(
    protected: &[u8],
    payload: &[u8],
    output: &mut [u8],
) -> Result<usize, RendezvousCoseRefusal> {
    let mut cursor = Cursor::new(output);
    Encoder::new(&mut cursor)
        .array(4)
        .and_then(|encoder| encoder.str(SIGNATURE_CONTEXT))
        .and_then(|encoder| encoder.bytes(protected))
        .and_then(|encoder| encoder.bytes(&[]))
        .and_then(|encoder| encoder.bytes(payload))
        .map_err(|_| RendezvousCoseRefusal::EncodedBound)?;
    Ok(cursor.position())
}

fn decode_error(error: minicbor::decode::Error) -> RendezvousCoseRefusal {
    if error.is_end_of_input() {
        RendezvousCoseRefusal::Truncated
    } else {
        RendezvousCoseRefusal::Malformed
    }
}

#[cfg(test)]
mod tests;
