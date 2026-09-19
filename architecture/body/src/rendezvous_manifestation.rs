//! Reversible human-carrier manifestation of the canonical rendezvous CBOR.
//!
//! This is an envelope for the binary descriptor, not another descriptor
//! grammar. The same text may be placed in a file, QR payload, or URL fragment.

use crate::{
    decode_running_host_rendezvous_cbor, encode_running_host_rendezvous_cbor,
    BorrowedRunningHostRendezvousDescriptor, RendezvousCborRefusal,
    RunningHostRendezvousDescriptor, MAX_RENDEZVOUS_CBOR_BYTES,
};

pub const RENDEZVOUS_TEXT_PREFIX: &str = "conduit-rendezvous-v1:";
pub const MAX_RENDEZVOUS_ENVELOPE_BYTES: usize =
    RENDEZVOUS_TEXT_PREFIX.len() + MAX_RENDEZVOUS_CBOR_BYTES.div_ceil(3) * 4;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendezvousManifestationRefusal {
    OutputBound,
    UnsupportedEnvelope,
    InvalidAlphabet,
    NonCanonical,
    EncodedBound,
    Descriptor(RendezvousCborRefusal),
}

/// Encode the canonical descriptor bytes as unpadded base64url with one
/// versioned prefix. The returned bytes are UTF-8/ASCII and use caller storage.
pub fn encode_running_host_rendezvous_text(
    descriptor: &RunningHostRendezvousDescriptor,
    output: &mut [u8],
) -> Result<usize, RendezvousManifestationRefusal> {
    let mut binary = [0_u8; MAX_RENDEZVOUS_CBOR_BYTES];
    let binary_len = encode_running_host_rendezvous_cbor(descriptor, &mut binary)
        .map_err(RendezvousManifestationRefusal::Descriptor)?;
    let encoded_len = encoded_len(binary_len);
    let total = RENDEZVOUS_TEXT_PREFIX.len() + encoded_len;
    if total > output.len() {
        return Err(RendezvousManifestationRefusal::OutputBound);
    }
    output[..RENDEZVOUS_TEXT_PREFIX.len()].copy_from_slice(RENDEZVOUS_TEXT_PREFIX.as_bytes());
    encode_base64url(
        &binary[..binary_len],
        &mut output[RENDEZVOUS_TEXT_PREFIX.len()..total],
    );
    Ok(total)
}

/// Decode a text, QR, file, or URL-fragment payload into caller-owned binary
/// storage. The returned semantic view borrows that storage.
pub fn decode_running_host_rendezvous_text<'a>(
    input: &str,
    now_millis: u64,
    binary_storage: &'a mut [u8],
) -> Result<BorrowedRunningHostRendezvousDescriptor<'a>, RendezvousManifestationRefusal> {
    let encoded = input
        .strip_prefix(RENDEZVOUS_TEXT_PREFIX)
        .ok_or(RendezvousManifestationRefusal::UnsupportedEnvelope)?;
    if encoded.len() > encoded_len(MAX_RENDEZVOUS_CBOR_BYTES) {
        return Err(RendezvousManifestationRefusal::EncodedBound);
    }
    let decoded_len = decoded_len(encoded.as_bytes())?;
    if decoded_len > binary_storage.len() || decoded_len > MAX_RENDEZVOUS_CBOR_BYTES {
        return Err(RendezvousManifestationRefusal::OutputBound);
    }
    decode_base64url(encoded.as_bytes(), &mut binary_storage[..decoded_len])?;
    decode_running_host_rendezvous_cbor(&binary_storage[..decoded_len], now_millis)
        .map_err(RendezvousManifestationRefusal::Descriptor)
}

const fn encoded_len(input_len: usize) -> usize {
    (input_len / 3) * 4
        + match input_len % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

fn decoded_len(input: &[u8]) -> Result<usize, RendezvousManifestationRefusal> {
    if input.len() % 4 == 1 {
        return Err(RendezvousManifestationRefusal::NonCanonical);
    }
    if input.iter().any(|byte| decode_digit(*byte).is_none()) {
        return Err(RendezvousManifestationRefusal::InvalidAlphabet);
    }
    Ok((input.len() / 4) * 3
        + match input.len() % 4 {
            0 => 0,
            2 => 1,
            3 => 2,
            _ => unreachable!(),
        })
}

fn encode_base64url(input: &[u8], output: &mut [u8]) {
    let mut source = 0;
    let mut target = 0;
    while source + 3 <= input.len() {
        let word = u32::from_be_bytes([0, input[source], input[source + 1], input[source + 2]]);
        output[target] = ALPHABET[((word >> 18) & 63) as usize];
        output[target + 1] = ALPHABET[((word >> 12) & 63) as usize];
        output[target + 2] = ALPHABET[((word >> 6) & 63) as usize];
        output[target + 3] = ALPHABET[(word & 63) as usize];
        source += 3;
        target += 4;
    }
    let remainder = input.len() - source;
    if remainder > 0 {
        let word = (u32::from(input[source]) << 16)
            | if remainder == 2 {
                u32::from(input[source + 1]) << 8
            } else {
                0
            };
        output[target] = ALPHABET[((word >> 18) & 63) as usize];
        output[target + 1] = ALPHABET[((word >> 12) & 63) as usize];
        if remainder == 2 {
            output[target + 2] = ALPHABET[((word >> 6) & 63) as usize];
        }
    }
}

fn decode_base64url(input: &[u8], output: &mut [u8]) -> Result<(), RendezvousManifestationRefusal> {
    let mut source = 0;
    let mut target = 0;
    while source + 4 <= input.len() {
        let word = (u32::from(digit(input[source])?) << 18)
            | (u32::from(digit(input[source + 1])?) << 12)
            | (u32::from(digit(input[source + 2])?) << 6)
            | u32::from(digit(input[source + 3])?);
        output[target..target + 3].copy_from_slice(&word.to_be_bytes()[1..]);
        source += 4;
        target += 3;
    }
    match input.len() - source {
        0 => {}
        2 => {
            let a = digit(input[source])?;
            let b = digit(input[source + 1])?;
            if b & 0x0f != 0 {
                return Err(RendezvousManifestationRefusal::NonCanonical);
            }
            output[target] = (a << 2) | (b >> 4);
        }
        3 => {
            let a = digit(input[source])?;
            let b = digit(input[source + 1])?;
            let c = digit(input[source + 2])?;
            if c & 0x03 != 0 {
                return Err(RendezvousManifestationRefusal::NonCanonical);
            }
            output[target] = (a << 2) | (b >> 4);
            output[target + 1] = (b << 4) | (c >> 2);
        }
        _ => return Err(RendezvousManifestationRefusal::NonCanonical),
    }
    Ok(())
}

fn digit(byte: u8) -> Result<u8, RendezvousManifestationRefusal> {
    decode_digit(byte).ok_or(RendezvousManifestationRefusal::InvalidAlphabet)
}

const fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily};

    fn descriptor() -> RunningHostRendezvousDescriptor {
        RunningHostRendezvousDescriptor::new(
            alloc::vec![RendezvousCandidate {
                candidate_id: "candidate-a".into(),
                line_family: RendezvousLineFamily::AuthenticatedTlsStream,
                reachability: "192.0.2.1:443".into(),
                authentication: RendezvousAuthentication {
                    server_identity: "relay.example".into(),
                    transport_binding_sha256: [7; 32],
                },
                expires_at_millis: 2_000,
                maximum_attempts: 2,
                attempt_timeout_millis: 500,
            }],
            [9; 32],
            1_000,
        )
        .unwrap()
    }

    #[test]
    fn one_envelope_round_trips_for_text_file_qr_and_link_carriers() {
        let mut text = [0_u8; MAX_RENDEZVOUS_ENVELOPE_BYTES];
        let length = encode_running_host_rendezvous_text(&descriptor(), &mut text).unwrap();
        let envelope = core::str::from_utf8(&text[..length]).unwrap();
        assert!(envelope.starts_with(RENDEZVOUS_TEXT_PREFIX));
        assert!(!envelope.contains('='));

        let mut binary = [0_u8; MAX_RENDEZVOUS_CBOR_BYTES];
        let decoded = decode_running_host_rendezvous_text(envelope, 1_000, &mut binary).unwrap();
        assert_eq!(
            decoded.candidates().next().unwrap().candidate_id,
            "candidate-a"
        );
        assert_eq!(decoded.copy_session_secret_for_attempt(), [9; 32]);
    }

    #[test]
    fn envelope_refuses_parallel_grammars_padding_and_noncanonical_tail_bits() {
        let mut binary = [0_u8; MAX_RENDEZVOUS_CBOR_BYTES];
        assert_eq!(
            decode_running_host_rendezvous_text("candidate=a", 0, &mut binary).unwrap_err(),
            RendezvousManifestationRefusal::UnsupportedEnvelope
        );
        assert_eq!(
            decode_running_host_rendezvous_text("conduit-rendezvous-v1:AA=", 0, &mut binary)
                .unwrap_err(),
            RendezvousManifestationRefusal::InvalidAlphabet
        );
        assert_eq!(
            decode_running_host_rendezvous_text("conduit-rendezvous-v1:AB", 0, &mut binary)
                .unwrap_err(),
            RendezvousManifestationRefusal::NonCanonical
        );
    }
}
