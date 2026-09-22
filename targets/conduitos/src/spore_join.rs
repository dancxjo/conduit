//! Bounded boot-time join proof emitted on the admitted early serial Line.

use alloc::vec::Vec;
use conduit_body::{SpawnInvitationClaim, SpawnInvitationSecret};
use serde::Serialize;

use crate::spore_provision::NativeMediaProvision;

pub const JOIN_SCHEMA: &str = "conduit.conduitos/serial-spawn-observation@1";
pub const MAXIMUM_JOIN_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    InvalidSecret,
    Serialization,
    Bound,
}

impl JoinError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSecret => "spore-join-secret-invalid",
            Self::Serialization => "spore-join-serialization-failed",
            Self::Bound => "spore-join-bound-exceeded",
        }
    }
}

#[derive(Serialize)]
struct JoinEnvelope<'a> {
    schema: &'static str,
    protocol: u16,
    spore_id: &'a str,
    image_id: &'a str,
    advertisement: &'a conduit_core::HostAdvertisement,
    invitation_id: &'a str,
    body_id: &'a str,
    host_id: &'a str,
    boot_id: &'a str,
    nonce: [u8; 32],
    signature: &'a [u8],
    expiry_checked_by_body: bool,
    membership_claimed: bool,
}

pub fn encode(
    provision: NativeMediaProvision,
    advertisement: &conduit_core::HostAdvertisement,
) -> Result<Vec<u8>, JoinError> {
    let mut provision = provision;
    let claim = SpawnInvitationClaim {
        invitation_id: serde_json::from_value(serde_json::Value::String(
            provision.invitation_provision.invitation_id.clone(),
        ))
        .map_err(|_| JoinError::Serialization)?,
        body_id: serde_json::from_value(serde_json::Value::String(provision.spore.body_id.clone()))
            .map_err(|_| JoinError::Serialization)?,
        nonce: provision
            .invitation_provision
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| JoinError::InvalidSecret)?,
        expires_at_millis: provision.invitation_provision.expires_at_millis,
    };
    let mut secret_bytes: [u8; 32] = provision
        .invitation_provision
        .secret
        .as_slice()
        .try_into()
        .map_err(|_| JoinError::InvalidSecret)?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret_bytes)
        .map_err(|_| JoinError::InvalidSecret)?;
    let transcript = claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    );
    let signature = secret.sign(&transcript);
    secret_bytes.fill(0);
    provision.invitation_provision.secret.fill(0);
    let envelope = JoinEnvelope {
        schema: JOIN_SCHEMA,
        protocol: 1,
        spore_id: &provision.spore.spore_id,
        image_id: &provision.spore.image_id,
        advertisement,
        invitation_id: claim.invitation_id.as_str(),
        body_id: claim.body_id.as_str(),
        host_id: advertisement.host_id.as_str(),
        boot_id: advertisement.boot_id.as_str(),
        nonce: claim.nonce,
        signature: &signature,
        // ConduitOS has no admitted civil-time source at this early stage.
        // The body owns the authoritative expiry check during admission.
        expiry_checked_by_body: true,
        membership_claimed: false,
    };
    let encoded = serde_json::to_vec(&envelope).map_err(|_| JoinError::Serialization)?;
    if encoded.len() > MAXIMUM_JOIN_BYTES {
        return Err(JoinError::Bound);
    }
    Ok(encoded)
}

pub fn encode_region(
    region: &[u8],
    target: &str,
    profile_id: &str,
    build_id: &str,
    advertisement: &conduit_core::HostAdvertisement,
) -> Result<Option<Vec<u8>>, &'static str> {
    let provision = crate::spore_provision::decode(region).map_err(|error| error.as_str())?;
    let Some(provision) = provision else {
        return Ok(None);
    };
    crate::spore_provision::validate_image_binding(&provision, target, profile_id, build_id)
        .map_err(|error| error.as_str())?;
    encode(provision, advertisement)
        .map(Some)
        .map_err(|error| error.as_str())
}

#[cfg(target_arch = "x86_64")]
pub fn encode_native(
    provision: NativeMediaProvision,
    identities: crate::identity::BootIdentities,
) -> Result<Vec<u8>, JoinError> {
    let host_id = conduit_core::HostId::from(crate::identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(crate::identity::hex(&identities.boot));
    let advertisement = crate::presenter_control::native_host_advertisement(&host_id, &boot_id, 1);
    encode(provision, &advertisement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec;
    use conduit_core::{BootId, HostId};

    fn provision() -> NativeMediaProvision {
        serde_json::from_value(serde_json::json!({
            "schema":"conduit.spore/native-media-provision@1", "image_bytes":8192,
            "spore": {"schema":"conduit.body/spore-manifest@2", "spore_id":"spore/one",
                "body_id":"body/one", "binding":{"mode":"self-joining","invitation_id":"invitation/one"},
                "body_description_id":"description/one", "host_entry_name":"host", "host_configuration_id":"config/one",
                "profile_id":"profile/one", "build_id":"build/one", "image_id":"image/one",
                "image_content_digest":format!("sha256:{}", "1".repeat(64)), "target":"conduitos/x86_64/pc",
                "output":"disk-image", "fabrication":{}, "source_identity":"source/one"},
            "invitation_provision":{"invitation_id":"invitation/one", "nonce":vec![17;32],
                "expires_at_millis":1_800_000_000_000_u64, "secret":vec![13;32]}
        }))
        .unwrap()
    }

    #[test]
    fn emits_one_bounded_non_member_serial_join_without_logging_the_secret() {
        let advertisement = crate::presenter_control::native_host_advertisement(
            &HostId::from("host/one"),
            &BootId::from("boot/one"),
            1,
        );
        let encoded = encode(provision(), &advertisement).unwrap();
        assert!(encoded.len() < MAXIMUM_JOIN_BYTES);
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["schema"], JOIN_SCHEMA);
        assert_eq!(value["spore_id"], "spore/one");
        assert_eq!(value["image_id"], "image/one");
        assert_eq!(value["membership_claimed"], false);
        assert_eq!(value["expiry_checked_by_body"], true);
        assert_eq!(value["signature"].as_array().unwrap().len(), 64);
        assert!(!encoded.windows(32).any(|window| window == [13; 32]));
    }
}
