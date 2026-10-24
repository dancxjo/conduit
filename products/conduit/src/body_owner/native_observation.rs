//! Explicitly authorized owner ingestion of the ConduitOS serial join sign.
//!
//! Serial reachability is observation, not admission or owner authentication.
//! The foreground owner names the expected Host independently, then consumes
//! the existing invitation authority through the ordinary portable request.

use conduit_body::{
    BodyId, PortableSpawnAdmissionRequest, SpawnInvitationId, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{BootId, HostAdvertisement, HostId};
use serde::Deserialize;

const SCHEMA: &str = "conduit.conduitos/serial-spawn-observation@1";
const COMPACT_SCHEMA: &str = "conduit.conduitos/serial-spawn-observation@2";

#[derive(Deserialize)]
#[serde(untagged)]
enum SerialAdvertisement {
    Compact(conduit_body::admission_advertisement_wire::CompactAdvertisement),
    Legacy(HostAdvertisement),
}
const MAXIMUM_ARTIFACT_ID_BYTES: usize = 192;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeSerialSpawnObservation {
    schema: String,
    protocol: u16,
    spore_id: String,
    image_id: String,
    advertisement: SerialAdvertisement,
    invitation_id: SpawnInvitationId,
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    nonce: [u8; 32],
    signature: Vec<u8>,
    expiry_checked_by_body: bool,
    membership_claimed: bool,
}

impl NativeSerialSpawnObservation {
    pub(super) fn into_request(
        self: Box<Self>,
        expected_host_id: &str,
    ) -> Result<PortableSpawnAdmissionRequest, String> {
        let advertisement = match (self.schema.as_str(), self.protocol, self.advertisement) {
            (SCHEMA, 1, SerialAdvertisement::Legacy(value)) => value,
            (COMPACT_SCHEMA, 2, SerialAdvertisement::Compact(value)) => value.0,
            _ => return Err("native join observation has an unsupported contract".into()),
        };
        if !self.expiry_checked_by_body {
            return Err("native join observation has an unsupported contract".into());
        }
        if self.membership_claimed {
            return Err("native join observation claims membership before admission".into());
        }
        if self.spore_id.is_empty()
            || self.spore_id.len() > MAXIMUM_ARTIFACT_ID_BYTES
            || self.image_id.is_empty()
            || self.image_id.len() > MAXIMUM_ARTIFACT_ID_BYTES
        {
            return Err("native join observation has an invalid artifact identity".into());
        }
        if expected_host_id.is_empty()
            || self.host_id != advertisement.host_id
            || self.boot_id != advertisement.boot_id
            || self.host_id.as_str() != expected_host_id
        {
            return Err("native join observation differs from the authorized Host".into());
        }
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: self.invitation_id,
            body_id: self.body_id,
            host_advertisement: advertisement,
            nonce: self.nonce,
            signature: self.signature,
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        request.validate().map_err(|error| error.to_string())?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::OfferGeneration;
    use conduit_std_host::{StdHost, StdHostConfig};

    fn observation() -> Box<NativeSerialSpawnObservation> {
        let advertisement = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("host/native-guest"),
            boot_id: BootId::from("boot/native-guest"),
            offer_generation: OfferGeneration(1),
        })
        .advertisement()
        .clone();
        Box::new(NativeSerialSpawnObservation {
            schema: SCHEMA.into(),
            protocol: 1,
            spore_id: "spore/one".into(),
            image_id: "image/one".into(),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            advertisement: SerialAdvertisement::Legacy(advertisement),
            invitation_id: serde_json::from_value(serde_json::json!("invitation/one")).unwrap(),
            body_id: serde_json::from_value(serde_json::json!("body/one")).unwrap(),
            nonce: [1; 32],
            signature: vec![2; 64],
            expiry_checked_by_body: true,
            membership_claimed: false,
        })
    }

    #[test]
    fn exact_serial_observation_becomes_only_a_portable_request() {
        let request = observation().into_request("host/native-guest").unwrap();
        assert_eq!(
            request.host_advertisement.host_id.as_str(),
            "host/native-guest"
        );
        assert!(!request.membership_admitted);
        assert!(!request.plan_created);
        assert!(!request.play_created);
    }

    #[test]
    fn compact_serial_observation_reconstructs_exact_request_before_owner_admission() {
        let mut original = observation().into_request("host/native-guest").unwrap();
        // This carrier is the actual native product offer, not the much larger
        // general-purpose StdHost catalogue used by the legacy tests.
        original.host_advertisement = serde_json::from_str(include_str!(
            "../../../../proof/fixtures/native-admission/product-advertisement.json"
        ))
        .unwrap();
        let expected_host_id = original.host_advertisement.host_id.as_str();
        let value = serde_json::json!({
            "schema": COMPACT_SCHEMA, "protocol":2, "spore_id":"spore/one", "image_id":"image/one",
            "advertisement":conduit_body::admission_advertisement_wire::CompactAdvertisement(original.host_advertisement.clone()),
            "invitation_id":original.invitation_id,"body_id":original.body_id,
            "host_id":original.host_advertisement.host_id,"boot_id":original.host_advertisement.boot_id,
            "nonce":original.nonce,"signature":original.signature,
            "expiry_checked_by_body":true,"membership_claimed":false,
        });
        let decoded: Box<NativeSerialSpawnObservation> =
            serde_json::from_value(value.clone()).unwrap();
        let reconstructed = decoded.into_request(expected_host_id).unwrap();
        assert_eq!(
            serde_json::to_value(&reconstructed).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        let mut mixed = value;
        mixed["schema"] = serde_json::json!(SCHEMA);
        mixed["protocol"] = serde_json::json!(1);
        let decoded: Box<NativeSerialSpawnObservation> = serde_json::from_value(mixed).unwrap();
        assert!(decoded.into_request(expected_host_id).is_err());
    }

    #[test]
    fn large_stdhost_signed_legacy_route_keeps_complete_request_and_receipt() {
        use conduit_body::{
            AdmissionManager, AdmissionSigns, BodyMembership, PortableAdmissionReceipt,
            RoutedAdmissionRequest, RoutedAdmissionResponse, SpawnInvitationSecret,
            ROUTED_ADMISSION_REQUEST_SCHEMA, ROUTED_ADMISSION_RESPONSE_SCHEMA,
            SPAWN_ADMISSION_RECEIPT_SCHEMA,
        };
        let mut request = observation().into_request("host/native-guest").unwrap();
        let raw_ad = serde_json::to_vec(&request.host_advertisement).unwrap();
        assert!(raw_ad.len() > conduit_body::admission_advertisement_wire::MAXIMUM_EXPANDED_BYTES);
        assert_eq!(
            ROUTED_ADMISSION_REQUEST_SCHEMA,
            "conduit.body/routed-admission-request@1"
        );
        let mut manager = AdmissionManager::new(request.body_id.clone()).unwrap();
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
                [12; 32],
                10_000,
                20_000,
            )
            .unwrap();
        request.invitation_id = invitation.invitation_id.clone();
        request.nonce = invitation.nonce;
        request.signature = invitation
            .secret
            .sign(&invitation.signing_transcript(
                &request.host_advertisement.host_id,
                &request.host_advertisement.boot_id,
                request.host_advertisement.offer_generation,
            ))
            .to_vec();
        let routed = RoutedAdmissionRequest {
            schema: ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: request.invitation_id.as_str().into(),
            request: request.clone(),
        };
        let request_wire = serde_json::to_vec(&routed).unwrap();
        assert!(request_wire.len() as u64 <= crate::durable_host::MAXIMUM_BODY_ADMISSION_BYTES);
        let decoded: RoutedAdmissionRequest = serde_json::from_slice(&request_wire).unwrap();
        assert_eq!(
            serde_json::to_vec(&decoded.request).unwrap(),
            serde_json::to_vec(&request).unwrap()
        );
        let mut membership = BodyMembership::new(request.body_id.clone()).unwrap();
        let credential = manager
            .complete_spawn(
                &mut membership,
                &decoded.request.host_advertisement,
                &decoded.request.admission_proof().unwrap(),
                10_001,
                AdmissionSigns {
                    part_admitted: "sign/legacy-std-part".into(),
                    host_attached: "sign/legacy-std-host".into(),
                    candidate_admitted: "sign/legacy-std-candidate".into(),
                },
            )
            .unwrap();
        let receipt = PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential,
            host_advertisement: request.host_advertisement.clone(),
            membership_admitted: true,
            current_offers_available: !request.host_advertisement.capabilities.is_empty(),
            plan_created: false,
            play_created: false,
        };
        assert_eq!(routed.response_schema(), ROUTED_ADMISSION_RESPONSE_SCHEMA);
        let response_wire = serde_json::to_vec(&RoutedAdmissionResponse::Admitted {
            schema: routed.response_schema().into(),
            receipt: Box::new(receipt.clone()),
        })
        .unwrap();
        assert!(response_wire.len() as u64 <= crate::durable_host::MAXIMUM_BODY_ADMISSION_BYTES);
        let response: RoutedAdmissionResponse = serde_json::from_slice(&response_wire).unwrap();
        let RoutedAdmissionResponse::Admitted {
            receipt: decoded, ..
        } = response
        else {
            panic!("admitted response")
        };
        decoded.validate_against(&request).unwrap();
        assert_eq!(
            serde_json::to_vec(decoded.as_ref()).unwrap(),
            serde_json::to_vec(&receipt).unwrap()
        );
        std::eprintln!(
            "legacy_stdhost_signed_roundtrip expanded={} request={} response={} generic_carrier={}",
            raw_ad.len(),
            request_wire.len(),
            response_wire.len(),
            crate::durable_host::MAXIMUM_BODY_ADMISSION_BYTES
        );
    }

    #[test]
    fn altered_contract_authority_and_membership_claims_refuse() {
        let mut wrong = observation();
        wrong.protocol = 2;
        assert!(wrong.into_request("host/native-guest").is_err());
        let mut wrong = observation();
        wrong.membership_claimed = true;
        assert!(wrong.into_request("host/native-guest").is_err());
        let mut wrong = observation();
        wrong.boot_id = BootId::from("boot/other");
        assert!(wrong.into_request("host/native-guest").is_err());
        assert!(observation().into_request("host/other").is_err());
    }
}
