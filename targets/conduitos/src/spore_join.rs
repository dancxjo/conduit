//! Bounded boot-time join proof emitted on the admitted early serial Line.

use alloc::{string::String, vec::Vec};
use conduit_body::{
    PortableSpawnAdmissionRequest, ROUTED_ADMISSION_REQUEST_SCHEMA, RoutedAdmissionRequest,
    SPAWN_ADMISSION_REQUEST_SCHEMA, SpawnInvitationClaim, SpawnInvitationSecret,
    SpawnRendezvousDescriptor,
};
use serde::Serialize;

use crate::spore_provision::{NativeMediaProvision, RouteCertificate};

pub const JOIN_SCHEMA: &str = "conduit.conduitos/serial-spawn-observation@1";
pub const MAXIMUM_JOIN_BYTES: usize = 32 * 1024;
pub const MAXIMUM_ROUTE_REQUEST_BYTES: usize =
    crate::native_network_bounds::MAXIMUM_BINARY_MESSAGE_BYTES;

/// Public provision facts carried into the native Face. A signed observation
/// is still only a request: no receipt, membership, or Body exists here yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingNativeJoin {
    pub spore_id: String,
    pub body_id: String,
    pub invitation_id: String,
    /// Public, bounded owner reachability. This is not admission authority.
    pub rendezvous: Option<SpawnRendezvousDescriptor>,
    /// Exact public TLS leaves checked against the candidate bindings.
    pub route_certificates: Vec<RouteCertificate>,
}

impl PendingNativeJoin {
    pub fn from_provision(provision: &NativeMediaProvision) -> Self {
        Self {
            spore_id: provision.spore.spore_id.clone(),
            body_id: provision.spore.body_id.clone(),
            invitation_id: provision.invitation_provision.invitation_id.clone(),
            rendezvous: provision.invitation_provision.rendezvous.clone(),
            route_certificates: provision.invitation_provision.route_certificates.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    InvalidSecret,
    Serialization,
    Bound,
}

/// One exact signed boot observation and, for a routed invitation, its
/// transport-owned admission request. Neither value is a membership receipt.
/// The request must never be projected into the pending Face or serial log.
pub struct PreparedNativeJoin {
    pub serial_observation: Vec<u8>,
    pub routed_request: Option<RoutedAdmissionRequest>,
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
    prepare(provision, advertisement).map(|join| join.serial_observation)
}

pub fn prepare(
    provision: NativeMediaProvision,
    advertisement: &conduit_core::HostAdvertisement,
) -> Result<PreparedNativeJoin, JoinError> {
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
    let routed_request =
        provision
            .invitation_provision
            .rendezvous
            .as_ref()
            .map(|_| RoutedAdmissionRequest {
                schema: ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: claim.invitation_id.as_str().into(),
                request: PortableSpawnAdmissionRequest {
                    schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
                    invitation_id: claim.invitation_id.clone(),
                    body_id: claim.body_id.clone(),
                    host_advertisement: advertisement.clone(),
                    nonce: claim.nonce,
                    signature: signature.to_vec(),
                    membership_admitted: false,
                    plan_created: false,
                    play_created: false,
                },
            });
    if let Some(request) = &routed_request {
        let encoded = serde_json::to_vec(request).map_err(|_| JoinError::Serialization)?;
        if encoded.len() > MAXIMUM_ROUTE_REQUEST_BYTES {
            return Err(JoinError::Bound);
        }
    }
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
    Ok(PreparedNativeJoin {
        serial_observation: encoded,
        routed_request,
    })
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
    let advertisement = crate::mask_control::native_host_advertisement(&host_id, &boot_id, 1);
    encode(provision, &advertisement)
}

#[cfg(target_arch = "x86_64")]
pub fn prepare_native(
    provision: NativeMediaProvision,
    identities: crate::identity::BootIdentities,
) -> Result<PreparedNativeJoin, JoinError> {
    let host_id = conduit_core::HostId::from(crate::identity::hex(&identities.host));
    let boot_id = conduit_core::BootId::from(crate::identity::hex(&identities.boot));
    let advertisement = crate::mask_control::native_host_advertisement(&host_id, &boot_id, 1);
    prepare(provision, &advertisement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec;
    use conduit_core::{BootId, HostId};
    use sha2::{Digest, Sha256};

    fn provision() -> NativeMediaProvision {
        serde_json::from_value(provision_value()).unwrap()
    }

    fn provision_value() -> serde_json::Value {
        serde_json::json!({
            "schema":"conduit.spore/native-media-provision@1", "image_bytes":8192,
            "spore": {"schema":"conduit.body/spore-manifest@2", "spore_id":"spore/one",
                "body_id":"body/one", "binding":{"mode":"self-joining","invitation_id":"invitation/one"},
                "body_description_id":"description/one", "host_entry_name":"host", "host_configuration_id":"config/one",
                "profile_id":"profile/one", "build_id":"build/one", "image_id":"image/one",
                "image_content_digest":format!("sha256:{}", "1".repeat(64)), "target":"conduitos/x86_64/pc",
                "output":"disk-image", "make":{}, "source_identity":"source/one"},
            "invitation_provision":{"invitation_id":"invitation/one", "nonce":vec![17;32],
                "expires_at_millis":1_800_000_000_000_u64, "secret":vec![13;32]}
        })
    }

    #[test]
    fn emits_one_bounded_non_member_serial_join_without_logging_the_secret() {
        let advertisement = crate::mask_control::native_host_advertisement(
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
        assert!(
            prepare(provision(), &advertisement)
                .unwrap()
                .routed_request
                .is_none()
        );
    }

    #[test]
    fn pending_join_retains_public_route_but_no_invitation_secret() {
        let certificate = vec![42; 128];
        let binding: [u8; 32] = Sha256::digest(&certificate).into();
        let mut value = provision_value();
        value["invitation_provision"]["rendezvous"] = serde_json::json!({
            "protocol": 1,
            "body_id": "body/one",
            "invitation_id": "invitation/one",
            "candidates": [{
                "candidate_id": "candidate/owner",
                "line_family": "authenticated-tls-stream",
                "reachability": "wss://owner.example:443/conduit",
                "authentication": {
                    "server_identity": "owner.example",
                    "transport_binding_sha256": binding
                },
                "expires_at_millis": 1_800_000_000_000_u64,
                "maximum_attempts": 1,
                "attempt_timeout_millis": 2_000
            }]
        });
        value["invitation_provision"]["route_certificates"] = serde_json::json!([{
            "candidate_id":"candidate/owner", "certificate_der":certificate,
        }]);
        let provision: NativeMediaProvision = serde_json::from_value(value).unwrap();
        crate::spore_provision::validate_route_certificates(
            provision.invitation_provision.rendezvous.as_ref(),
            &provision.invitation_provision.route_certificates,
        )
        .unwrap();
        let pending = PendingNativeJoin::from_provision(&provision);
        assert_eq!(pending.body_id, "body/one");
        assert_eq!(pending.rendezvous.as_ref().unwrap().candidates.len(), 1);
        assert_eq!(pending.route_certificates.len(), 1);
        assert_eq!(pending.route_certificates[0].certificate_der, vec![42; 128]);
        assert!(!format!("{pending:?}").contains(&format!("{:?}", [13_u8; 32])));

        let advertisement = crate::mask_control::native_host_advertisement(
            &HostId::from("host/one"),
            &BootId::from("boot/one"),
            1,
        );
        let prepared = prepare(provision, &advertisement).unwrap();
        let request = prepared.routed_request.unwrap();
        assert_eq!(request.schema, ROUTED_ADMISSION_REQUEST_SCHEMA);
        assert_eq!(request.invitation_id, "invitation/one");
        assert_eq!(request.request.host_advertisement, advertisement);
        assert_eq!(request.request.body_id.as_str(), "body/one");
        assert!(!request.request.membership_admitted);
        assert!(!request.request.plan_created);
        assert!(!request.request.play_created);
        assert!(serde_json::to_vec(&request).unwrap().len() <= MAXIMUM_ROUTE_REQUEST_BYTES);
        let observation: serde_json::Value =
            serde_json::from_slice(&prepared.serial_observation).unwrap();
        assert_eq!(
            observation["signature"],
            serde_json::json!(request.request.signature)
        );
    }
}
