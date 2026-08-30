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
pub const PICO_SPAWN_PROTOCOL: u16 = 2;
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

/// A spore-bound invitation provisioned over the operator-admitted local Line.
/// The secret is transient proof material; constrained firmware does not retain
/// it after producing the Boot-specific join request.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicoSpawnProvision<'a> {
    pub protocol: u16,
    pub spore_id: &'a str,
    pub image_id: &'a str,
    pub invitation_id: &'a str,
    pub body_id: &'a str,
    pub nonce: [u8; 32],
    pub expires_at_millis: u64,
    pub secret: [u8; 32],
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PicoSpawnJoinRequest<'a> {
    pub protocol: u16,
    pub spore_id: &'a str,
    pub image_id: &'a str,
    pub invitation_id: &'a str,
    pub body_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: u64,
    pub nonce: [u8; 32],
    pub signature: &'a [u8],
}

pub fn validate_pico_spawn_provision(provision: &PicoSpawnProvision<'_>) -> bool {
    provision.protocol == PICO_SPAWN_PROTOCOL
        && !provision.spore_id.is_empty()
        && provision.spore_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && !provision.image_id.is_empty()
        && provision.image_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && !provision.invitation_id.is_empty()
        && provision.invitation_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && !provision.body_id.is_empty()
        && provision.body_id.len() <= MAX_PICO_ADMISSION_ID_BYTES
        && provision.nonce != [0; 32]
        && provision.expires_at_millis != 0
        && provision.secret.len() == 32
        && provision.secret.iter().any(|byte| *byte != 0)
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

    #[test]
    fn spawn_provision_requires_exact_finite_non_secret_identities_and_secret() {
        let mut provision = PicoSpawnProvision {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id: "spore/one",
            image_id: "image/one",
            invitation_id: "invitation/one",
            body_id: "body:one",
            nonce: [7; 32],
            expires_at_millis: 20,
            secret: [9; 32],
        };
        assert!(validate_pico_spawn_provision(&provision));
        provision.protocol += 1;
        assert!(!validate_pico_spawn_provision(&provision));
        provision.protocol = PICO_SPAWN_PROTOCOL;
        provision.secret = [0; 32];
        assert!(!validate_pico_spawn_provision(&provision));
    }

    #[test]
    fn spawn_provision_round_trips_through_the_firmware_json_decoder() {
        let provision = PicoSpawnProvision {
            protocol: PICO_SPAWN_PROTOCOL,
            spore_id: "spore/one",
            image_id: "image/one",
            invitation_id: "invitation/one",
            body_id: "body/one",
            nonce: [7; 32],
            expires_at_millis: 20,
            secret: [9; 32],
        };
        let encoded = serde_json::to_vec(&provision).unwrap();
        let decoded = serde_json::from_slice::<PicoSpawnProvision<'_>>(&encoded).unwrap();
        assert_eq!(decoded, provision);
    }
}
