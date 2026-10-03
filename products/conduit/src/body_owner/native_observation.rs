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
const MAXIMUM_ARTIFACT_ID_BYTES: usize = 192;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeSerialSpawnObservation {
    schema: String,
    protocol: u16,
    spore_id: String,
    image_id: String,
    advertisement: HostAdvertisement,
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
        if self.schema != SCHEMA || self.protocol != 1 || !self.expiry_checked_by_body {
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
            || self.host_id != self.advertisement.host_id
            || self.boot_id != self.advertisement.boot_id
            || self.host_id.as_str() != expected_host_id
        {
            return Err("native join observation differs from the authorized Host".into());
        }
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: self.invitation_id,
            body_id: self.body_id,
            host_advertisement: self.advertisement,
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
            advertisement,
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
