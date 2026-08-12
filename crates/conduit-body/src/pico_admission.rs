//! Bounded USB CDC frames for provisioned Pico Body admission.
//!
//! These frames carry observations and proof material only. Decoding one does
//! not create a candidate, membership, authority, Plan, or Play.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};

pub const PICO_ADMISSION_PROTOCOL: u16 = 1;
pub const PICO_ADMISSION_REQUEST: &[u8] = b"CONDUIT_BODY_ADVERTISE_V1";
pub const MAX_PICO_ADMISSION_FRAME_BYTES: usize = 4_096;
pub const MAX_PICO_ADMISSION_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicoAdmissionAdvertisement {
    pub protocol: u16,
    pub advertisement: HostAdvertisement,
    pub friendly_label: String,
    pub verifying_key: Vec<u8>,
    pub freshness_sequence: u64,
}

/// Borrowed challenge shape so constrained firmware can decode it without
/// allocating Body identity strings.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicoAdmissionChallenge<'a> {
    pub protocol: u16,
    pub admission_id: &'a str,
    pub body_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: u64,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

/// Borrowed proof shape emitted from a fixed firmware buffer.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PicoAdmissionProof<'a> {
    pub protocol: u16,
    pub admission_id: &'a str,
    pub body_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub nonce: [u8; 32],
    pub signature: &'a [u8],
}

pub fn validate_pico_challenge(
    challenge: &PicoAdmissionChallenge<'_>,
    expected_host: &str,
    expected_boot: &str,
    expected_generation: u64,
) -> bool {
    challenge.protocol == PICO_ADMISSION_PROTOCOL
        && !challenge.admission_id.is_empty()
        && challenge.admission_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && !challenge.body_id.is_empty()
        && challenge.body_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && challenge.host_id == expected_host
        && challenge.boot_id == expected_boot
        && challenge.offer_generation == expected_generation
        && challenge.nonce != [0; 32]
        && challenge.expires_at_millis > challenge.issued_at_millis
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_validation_is_boot_and_generation_exact() {
        let mut challenge = PicoAdmissionChallenge {
            protocol: PICO_ADMISSION_PROTOCOL,
            admission_id: "admission/1",
            body_id: "body/1",
            host_id: "pico/1",
            boot_id: "boot/1",
            offer_generation: 7,
            nonce: [9; 32],
            issued_at_millis: 10,
            expires_at_millis: 20,
        };
        assert!(validate_pico_challenge(&challenge, "pico/1", "boot/1", 7));
        challenge.boot_id = "boot/stale";
        assert!(!validate_pico_challenge(&challenge, "pico/1", "boot/1", 7));
        challenge.boot_id = "boot/1";
        challenge.offer_generation = 8;
        assert!(!validate_pico_challenge(&challenge, "pico/1", "boot/1", 7));
    }
}
