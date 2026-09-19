//! Fixed-storage deterministic CBOR for the shared running-Host rendezvous descriptor.
//!
//! This module owns interchange bytes only. Decoding does not grant membership,
//! trust, Line readiness, planning eligibility, or effect authority.

use core::ptr;

use minicbor::{decode::Decoder, encode::write::Cursor, Encoder};

use crate::{
    rendezvous_validation::{validate_candidate_views, RendezvousCandidateView},
    RendezvousCandidate, RendezvousDescriptorRefusal, RendezvousLineFamily,
    RunningHostRendezvousDescriptor, MAX_RENDEZVOUS_CANDIDATES,
};

pub const RENDEZVOUS_CBOR_PROFILE: &str = "conduit.host/rendezvous-cbor@1";
pub const MAX_RENDEZVOUS_EXTENSIONS: usize = 4;
pub const MAX_RENDEZVOUS_EXTENSION_BYTES: usize = 64;

/// Exact largest canonical descriptor admitted by the v1 CDDL profile.
pub const MAX_RENDEZVOUS_CBOR_BYTES: usize = 3_656;

const WIRE_VERSION: u8 = 1;
const KEY_VERSION: u8 = 0;
const KEY_CANDIDATES: u8 = 1;
const KEY_SESSION_SECRET: u8 = 2;
const FIRST_EXTENSION_KEY: u8 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousCborRefusal {
    EncodedBound,
    Truncated,
    Malformed,
    NonCanonical,
    DuplicateField,
    MissingField,
    UnsupportedVersion,
    UnsupportedMandatoryField,
    UnsupportedLineFamily,
    ExtensionBound,
    Descriptor(RendezvousDescriptorRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRendezvousCandidate<'a> {
    pub candidate_id: &'a str,
    pub line_family: RendezvousLineFamily,
    pub reachability: &'a str,
    pub server_identity: &'a str,
    pub transport_binding_sha256: [u8; 32],
    pub expires_at_millis: u64,
    pub maximum_attempts: u8,
    pub attempt_timeout_millis: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedRendezvousExtension<'a> {
    pub key: u8,
    pub value: &'a [u8],
}

/// A no-allocation view whose text and extension bytes borrow caller storage.
/// The capability is private, redacted from Debug, and volatile-erased on drop.
pub struct BorrowedRunningHostRendezvousDescriptor<'a> {
    candidates: [Option<BorrowedRendezvousCandidate<'a>>; MAX_RENDEZVOUS_CANDIDATES],
    candidate_count: usize,
    session_secret: [u8; 32],
    extensions: [Option<BorrowedRendezvousExtension<'a>>; MAX_RENDEZVOUS_EXTENSIONS],
    extension_count: usize,
}

impl core::fmt::Debug for BorrowedRunningHostRendezvousDescriptor<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BorrowedRunningHostRendezvousDescriptor")
            .field("profile", &RENDEZVOUS_CBOR_PROFILE)
            .field("candidates", &&self.candidates[..self.candidate_count])
            .field("session_secret", &"[REDACTED]")
            .field("extensions", &&self.extensions[..self.extension_count])
            .finish()
    }
}

impl<'a> BorrowedRunningHostRendezvousDescriptor<'a> {
    pub fn candidates(&self) -> impl ExactSizeIterator<Item = &BorrowedRendezvousCandidate<'a>> {
        self.candidates[..self.candidate_count]
            .iter()
            .map(|candidate| candidate.as_ref().expect("populated prefix"))
    }

    pub fn extensions(&self) -> impl ExactSizeIterator<Item = &BorrowedRendezvousExtension<'a>> {
        self.extensions[..self.extension_count]
            .iter()
            .map(|extension| extension.as_ref().expect("populated prefix"))
    }

    pub fn validate(&self, now_millis: u64) -> Result<(), RendezvousDescriptorRefusal> {
        validate_candidate_views(
            self.candidates[..self.candidate_count]
                .iter()
                .map(|candidate| {
                    RendezvousCandidateView::from(candidate.expect("populated prefix"))
                }),
            now_millis,
        )?;
        if self.session_secret == [0; 32] {
            return Err(RendezvousDescriptorRefusal::WeakInvitationSecret);
        }
        Ok(())
    }

    /// Copy the bounded rendezvous capability for one admitted attempt.
    /// The caller must erase the returned buffer when that attempt terminates.
    pub fn copy_session_secret_for_attempt(&self) -> [u8; 32] {
        self.session_secret
    }
}

impl Drop for BorrowedRunningHostRendezvousDescriptor<'_> {
    fn drop(&mut self) {
        volatile_erase(&mut self.session_secret);
    }
}

impl<'a> From<BorrowedRendezvousCandidate<'a>> for RendezvousCandidateView<'a> {
    fn from(candidate: BorrowedRendezvousCandidate<'a>) -> Self {
        Self {
            candidate_id: candidate.candidate_id,
            line_family: candidate.line_family,
            reachability: candidate.reachability,
            server_identity: candidate.server_identity,
            transport_binding_sha256: candidate.transport_binding_sha256,
            expires_at_millis: candidate.expires_at_millis,
            maximum_attempts: candidate.maximum_attempts,
            attempt_timeout_millis: candidate.attempt_timeout_millis,
        }
    }
}

pub fn encode_running_host_rendezvous_cbor(
    descriptor: &RunningHostRendezvousDescriptor,
    output: &mut [u8],
) -> Result<usize, RendezvousCborRefusal> {
    let output_len = MAX_RENDEZVOUS_CBOR_BYTES.min(output.len());
    let mut secret = descriptor.copy_session_secret_for_attempt();
    let result = encode_fields(
        descriptor
            .candidates
            .iter()
            .map(BorrowedRendezvousCandidate::from),
        &secret,
        core::iter::empty(),
        &mut output[..output_len],
    );
    volatile_erase(&mut secret);
    result
}

pub fn decode_running_host_rendezvous_cbor<'a>(
    input: &'a [u8],
    now_millis: u64,
) -> Result<BorrowedRunningHostRendezvousDescriptor<'a>, RendezvousCborRefusal> {
    if input.len() > MAX_RENDEZVOUS_CBOR_BYTES {
        return Err(RendezvousCborRefusal::EncodedBound);
    }
    let decoded = decode_inner(input)?;
    decoded
        .validate(now_millis)
        .map_err(RendezvousCborRefusal::Descriptor)?;

    let mut canonical = [0; MAX_RENDEZVOUS_CBOR_BYTES];
    let canonical_len = encode_fields(
        decoded.candidates().copied(),
        &decoded.session_secret,
        decoded.extensions().copied(),
        &mut canonical,
    )?;
    if canonical_len != input.len() || canonical[..canonical_len] != *input {
        return Err(RendezvousCborRefusal::NonCanonical);
    }
    Ok(decoded)
}

fn encode_fields<'a>(
    candidates: impl ExactSizeIterator<Item = BorrowedRendezvousCandidate<'a>>,
    session_secret: &[u8; 32],
    extensions: impl ExactSizeIterator<Item = BorrowedRendezvousExtension<'a>>,
    output: &mut [u8],
) -> Result<usize, RendezvousCborRefusal> {
    if candidates.len() == 0 || candidates.len() > MAX_RENDEZVOUS_CANDIDATES {
        return Err(RendezvousCborRefusal::Descriptor(
            RendezvousDescriptorRefusal::CandidateBound,
        ));
    }
    if extensions.len() > MAX_RENDEZVOUS_EXTENSIONS {
        return Err(RendezvousCborRefusal::ExtensionBound);
    }
    let mut cursor = Cursor::new(output);
    let mut prior_extension_key = None;
    {
        let mut encoder = Encoder::new(&mut cursor);
        encoder
            .map((3 + extensions.len()) as u64)
            .and_then(|encoder| encoder.u8(KEY_VERSION))
            .and_then(|encoder| encoder.u8(WIRE_VERSION))
            .and_then(|encoder| encoder.u8(KEY_CANDIDATES))
            .and_then(|encoder| encoder.array(candidates.len() as u64))
            .map_err(|_| RendezvousCborRefusal::EncodedBound)?;
        for candidate in candidates {
            encode_candidate(&mut encoder, candidate)?;
        }
        encoder
            .u8(KEY_SESSION_SECRET)
            .and_then(|encoder| encoder.bytes(session_secret))
            .map_err(|_| RendezvousCborRefusal::EncodedBound)?;
        for extension in extensions {
            if extension.key < FIRST_EXTENSION_KEY
                || extension.value.len() > MAX_RENDEZVOUS_EXTENSION_BYTES
                || prior_extension_key.is_some_and(|prior| extension.key <= prior)
            {
                return Err(RendezvousCborRefusal::ExtensionBound);
            }
            prior_extension_key = Some(extension.key);
            encoder
                .u8(extension.key)
                .and_then(|encoder| encoder.bytes(extension.value))
                .map_err(|_| RendezvousCborRefusal::EncodedBound)?;
        }
    }
    Ok(cursor.position())
}

fn encode_candidate<W: minicbor::encode::Write>(
    encoder: &mut Encoder<W>,
    candidate: BorrowedRendezvousCandidate<'_>,
) -> Result<(), RendezvousCborRefusal> {
    encoder
        .map(7)
        .and_then(|encoder| encoder.u8(0))
        .and_then(|encoder| encoder.str(candidate.candidate_id))
        .and_then(|encoder| encoder.u8(1))
        .and_then(|encoder| encoder.u8(line_family_code(candidate.line_family)))
        .and_then(|encoder| encoder.u8(2))
        .and_then(|encoder| encoder.str(candidate.reachability))
        .and_then(|encoder| encoder.u8(3))
        .and_then(|encoder| encoder.map(2))
        .and_then(|encoder| encoder.u8(0))
        .and_then(|encoder| encoder.str(candidate.server_identity))
        .and_then(|encoder| encoder.u8(1))
        .and_then(|encoder| encoder.bytes(&candidate.transport_binding_sha256))
        .and_then(|encoder| encoder.u8(4))
        .and_then(|encoder| encoder.u64(candidate.expires_at_millis))
        .and_then(|encoder| encoder.u8(5))
        .and_then(|encoder| encoder.u8(candidate.maximum_attempts))
        .and_then(|encoder| encoder.u8(6))
        .and_then(|encoder| encoder.u32(candidate.attempt_timeout_millis))
        .map_err(|_| RendezvousCborRefusal::EncodedBound)?;
    Ok(())
}

fn decode_inner(
    input: &[u8],
) -> Result<BorrowedRunningHostRendezvousDescriptor<'_>, RendezvousCborRefusal> {
    let mut decoder = Decoder::new(input);
    let fields = definite(decoder.map().map_err(decode_error)?)?;
    if fields < 3 || fields > (3 + MAX_RENDEZVOUS_EXTENSIONS) as u64 {
        return Err(RendezvousCborRefusal::MissingField);
    }
    let mut version = None;
    let mut candidates = [None; MAX_RENDEZVOUS_CANDIDATES];
    let mut candidate_count = None;
    let mut session_secret = None;
    let mut extensions: [Option<BorrowedRendezvousExtension<'_>>; MAX_RENDEZVOUS_EXTENSIONS] =
        [None; MAX_RENDEZVOUS_EXTENSIONS];
    let mut extension_count = 0;
    let mut prior_extension_key = None;

    for _ in 0..fields {
        let key = decoder.u8().map_err(decode_error)?;
        match key {
            KEY_VERSION if version.is_none() => version = Some(decoder.u8().map_err(decode_error)?),
            KEY_CANDIDATES if candidate_count.is_none() => {
                let count = definite(decoder.array().map_err(decode_error)?)? as usize;
                if count == 0 || count > MAX_RENDEZVOUS_CANDIDATES {
                    return Err(RendezvousCborRefusal::Descriptor(
                        RendezvousDescriptorRefusal::CandidateBound,
                    ));
                }
                for slot in candidates.iter_mut().take(count) {
                    *slot = Some(decode_candidate(&mut decoder)?);
                }
                candidate_count = Some(count);
            }
            KEY_SESSION_SECRET if session_secret.is_none() => {
                let bytes = decoder.bytes().map_err(decode_error)?;
                session_secret = Some(
                    bytes
                        .try_into()
                        .map_err(|_| RendezvousCborRefusal::Malformed)?,
                );
            }
            0..=127 => {
                if matches!(key, KEY_VERSION | KEY_CANDIDATES | KEY_SESSION_SECRET) {
                    return Err(RendezvousCborRefusal::DuplicateField);
                }
                return Err(RendezvousCborRefusal::UnsupportedMandatoryField);
            }
            FIRST_EXTENSION_KEY..=u8::MAX => {
                if prior_extension_key.is_some_and(|prior| key <= prior) {
                    return Err(RendezvousCborRefusal::NonCanonical);
                }
                if extensions[..extension_count]
                    .iter()
                    .flatten()
                    .any(|extension| extension.key == key)
                {
                    return Err(RendezvousCborRefusal::DuplicateField);
                }
                let value = decoder.bytes().map_err(decode_error)?;
                if extension_count == MAX_RENDEZVOUS_EXTENSIONS
                    || value.len() > MAX_RENDEZVOUS_EXTENSION_BYTES
                {
                    return Err(RendezvousCborRefusal::ExtensionBound);
                }
                extensions[extension_count] = Some(BorrowedRendezvousExtension { key, value });
                extension_count += 1;
                prior_extension_key = Some(key);
            }
        }
    }
    if decoder.position() != input.len() {
        return Err(RendezvousCborRefusal::Malformed);
    }
    match version.ok_or(RendezvousCborRefusal::MissingField)? {
        WIRE_VERSION => {}
        _ => return Err(RendezvousCborRefusal::UnsupportedVersion),
    }
    Ok(BorrowedRunningHostRendezvousDescriptor {
        candidates,
        candidate_count: candidate_count.ok_or(RendezvousCborRefusal::MissingField)?,
        session_secret: session_secret.ok_or(RendezvousCborRefusal::MissingField)?,
        extensions,
        extension_count,
    })
}

fn decode_candidate<'a>(
    decoder: &mut Decoder<'a>,
) -> Result<BorrowedRendezvousCandidate<'a>, RendezvousCborRefusal> {
    if definite(decoder.map().map_err(decode_error)?)? != 7 {
        return Err(RendezvousCborRefusal::MissingField);
    }
    let mut candidate_id = None;
    let mut line_family = None;
    let mut reachability = None;
    let mut authentication = None;
    let mut expires_at_millis = None;
    let mut maximum_attempts = None;
    let mut attempt_timeout_millis = None;
    for _ in 0..7 {
        match decoder.u8().map_err(decode_error)? {
            0 if candidate_id.is_none() => {
                candidate_id = Some(decoder.str().map_err(decode_error)?)
            }
            1 if line_family.is_none() => {
                line_family = Some(decode_line_family(decoder.u8().map_err(decode_error)?)?)
            }
            2 if reachability.is_none() => {
                reachability = Some(decoder.str().map_err(decode_error)?)
            }
            3 if authentication.is_none() => {
                authentication = Some(decode_authentication(&mut *decoder)?)
            }
            4 if expires_at_millis.is_none() => {
                expires_at_millis = Some(decoder.u64().map_err(decode_error)?)
            }
            5 if maximum_attempts.is_none() => {
                maximum_attempts = Some(decoder.u8().map_err(decode_error)?)
            }
            6 if attempt_timeout_millis.is_none() => {
                attempt_timeout_millis = Some(decoder.u32().map_err(decode_error)?)
            }
            0..=6 => return Err(RendezvousCborRefusal::DuplicateField),
            _ => return Err(RendezvousCborRefusal::UnsupportedMandatoryField),
        }
    }
    let (server_identity, transport_binding_sha256) =
        authentication.ok_or(RendezvousCborRefusal::MissingField)?;
    Ok(BorrowedRendezvousCandidate {
        candidate_id: candidate_id.ok_or(RendezvousCborRefusal::MissingField)?,
        line_family: line_family.ok_or(RendezvousCborRefusal::MissingField)?,
        reachability: reachability.ok_or(RendezvousCborRefusal::MissingField)?,
        server_identity,
        transport_binding_sha256,
        expires_at_millis: expires_at_millis.ok_or(RendezvousCborRefusal::MissingField)?,
        maximum_attempts: maximum_attempts.ok_or(RendezvousCborRefusal::MissingField)?,
        attempt_timeout_millis: attempt_timeout_millis
            .ok_or(RendezvousCborRefusal::MissingField)?,
    })
}

fn decode_authentication<'a>(
    decoder: &mut Decoder<'a>,
) -> Result<(&'a str, [u8; 32]), RendezvousCborRefusal> {
    if definite(decoder.map().map_err(decode_error)?)? != 2 {
        return Err(RendezvousCborRefusal::MissingField);
    }
    let mut identity = None;
    let mut binding = None;
    for _ in 0..2 {
        match decoder.u8().map_err(decode_error)? {
            0 if identity.is_none() => identity = Some(decoder.str().map_err(decode_error)?),
            1 if binding.is_none() => {
                binding = Some(
                    decoder
                        .bytes()
                        .map_err(decode_error)?
                        .try_into()
                        .map_err(|_| RendezvousCborRefusal::Malformed)?,
                )
            }
            0..=1 => return Err(RendezvousCborRefusal::DuplicateField),
            _ => return Err(RendezvousCborRefusal::UnsupportedMandatoryField),
        }
    }
    Ok((
        identity.ok_or(RendezvousCborRefusal::MissingField)?,
        binding.ok_or(RendezvousCborRefusal::MissingField)?,
    ))
}

impl<'a> From<&'a RendezvousCandidate> for BorrowedRendezvousCandidate<'a> {
    fn from(candidate: &'a RendezvousCandidate) -> Self {
        Self {
            candidate_id: &candidate.candidate_id,
            line_family: candidate.line_family,
            reachability: &candidate.reachability,
            server_identity: &candidate.authentication.server_identity,
            transport_binding_sha256: candidate.authentication.transport_binding_sha256,
            expires_at_millis: candidate.expires_at_millis,
            maximum_attempts: candidate.maximum_attempts,
            attempt_timeout_millis: candidate.attempt_timeout_millis,
        }
    }
}

fn definite(length: Option<u64>) -> Result<u64, RendezvousCborRefusal> {
    length.ok_or(RendezvousCborRefusal::NonCanonical)
}

fn decode_error(error: minicbor::decode::Error) -> RendezvousCborRefusal {
    if error.is_end_of_input() {
        RendezvousCborRefusal::Truncated
    } else {
        RendezvousCborRefusal::Malformed
    }
}

fn decode_line_family(value: u8) -> Result<RendezvousLineFamily, RendezvousCborRefusal> {
    match value {
        0 => Ok(RendezvousLineFamily::AuthenticatedTlsStream),
        1 => Ok(RendezvousLineFamily::AuthenticatedConduitLine),
        2 => Ok(RendezvousLineFamily::LocalLoopbackWebSocket),
        3 => Ok(RendezvousLineFamily::AttendedSerial),
        _ => Err(RendezvousCborRefusal::UnsupportedLineFamily),
    }
}

const fn line_family_code(value: RendezvousLineFamily) -> u8 {
    match value {
        RendezvousLineFamily::AuthenticatedTlsStream => 0,
        RendezvousLineFamily::AuthenticatedConduitLine => 1,
        RendezvousLineFamily::LocalLoopbackWebSocket => 2,
        RendezvousLineFamily::AttendedSerial => 3,
    }
}

fn volatile_erase(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: every byte is a valid live mutable location, written exactly once.
        unsafe { ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
#[path = "rendezvous_cbor/tests.rs"]
mod tests;
