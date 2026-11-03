//! Public construction pins for the existing invited USB admission exchange.
//! This local handoff contains no invitation secret and creates no authority.
use super::{PicoSpawnObservation, PicoSpawnTransportError};
use conduit_body::{FederationPeer, PortableSpawnAdmissionRequest, SpawnInvitationClaim};
use conduit_core::{HostId, HostProfileId};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PicoSpawnAdmissionBasis {
    pub spore_id: String,
    pub image_id: String,
    pub invitation: SpawnInvitationClaim,
    pub expected_host: HostId,
    pub expected_profile: HostProfileId,
    pub member_verifying_key: [u8; 32],
    pub owner: FederationPeer,
}

impl PicoSpawnAdmissionBasis {
    pub fn validate(&self, now_millis: u64) -> Result<(), PicoSpawnTransportError> {
        self.invitation
            .inspect(now_millis)
            .map_err(|_| PicoSpawnTransportError::WrongInvitation)?;
        for value in [
            self.spore_id.as_str(),
            self.image_id.as_str(),
            self.expected_host.as_str(),
            self.expected_profile.as_str(),
            self.owner.host_id.as_str(),
            self.owner.boot_id.as_str(),
        ] {
            if value.is_empty()
                || value.len() > conduit_body::MAX_PICO_ADMISSION_ID_BYTES
                || value.chars().any(char::is_control)
            {
                return Err(PicoSpawnTransportError::Malformed);
            }
        }
        VerifyingKey::from_bytes(&self.member_verifying_key)
            .map_err(|_| PicoSpawnTransportError::Malformed)?;
        Ok(())
    }

    /// Check exact physical/configuration pins and the device's actual proof
    /// before requesting the ordinary owner Core commit. Still not admission.
    pub fn request(
        &self,
        observation: &PicoSpawnObservation,
        now_millis: u64,
    ) -> Result<PortableSpawnAdmissionRequest, PicoSpawnTransportError> {
        self.validate(now_millis)?;
        let ad = &observation.advertisement;
        let proof = &observation.proof;
        if observation.spore_id != self.spore_id
            || observation.image_id != self.image_id
            || ad.host_id != self.expected_host
            || ad.profile != self.expected_profile
            || observation.verifying_key != self.member_verifying_key
            || proof.invitation_id != self.invitation.invitation_id
            || proof.body_id != self.invitation.body_id
            || proof.nonce != self.invitation.nonce
            || proof.host_id != ad.host_id
            || proof.boot_id != ad.boot_id
        {
            return Err(PicoSpawnTransportError::WrongAdvertisement);
        }
        let transcript =
            self.invitation
                .signing_transcript(&ad.host_id, &ad.boot_id, ad.offer_generation);
        VerifyingKey::from_bytes(&self.member_verifying_key)
            .map_err(|_| PicoSpawnTransportError::Malformed)?
            .verify_strict(&transcript, &Signature::from_bytes(&proof.signature))
            .map_err(|_| PicoSpawnTransportError::WrongInvitation)?;
        let request = PortableSpawnAdmissionRequest {
            schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: proof.invitation_id.clone(),
            body_id: proof.body_id.clone(),
            host_advertisement: ad.clone(),
            nonce: proof.nonce,
            signature: proof.signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        request
            .validate()
            .map_err(|_| PicoSpawnTransportError::Malformed)?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{SpawnAdmissionProof, SpawnInvitationSecret};
    use conduit_core::OfferGeneration;

    #[test]
    fn usb_admission_requires_construction_pins_and_actual_proof() {
        let key = SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap();
        let mut ad = crate::StdHost::new().advertisement().clone();
        ad.bases.clear();
        ad.resources.clear();
        ad.capabilities.clear();
        ad.planner_capabilities.clear();
        let basis = PicoSpawnAdmissionBasis {
            spore_id: "spore/test".into(),
            image_id: "image/test".into(),
            invitation: serde_json::from_value(serde_json::json!({
                "invitation_id": "invitation/test", "body_id": "body/test",
                "nonce": ([3_u8; 32].to_vec()), "expires_at_millis": 2000,
            }))
            .unwrap(),
            expected_host: ad.host_id.clone(),
            expected_profile: ad.profile.clone(),
            member_verifying_key: key.verifying_key(),
            owner: FederationPeer {
                host_id: "host/owner".into(),
                boot_id: "boot/owner".into(),
                offer_generation: OfferGeneration(1),
                verifying_key: [7; 32],
            },
        };
        let signature = key.sign(&basis.invitation.signing_transcript(
            &ad.host_id,
            &ad.boot_id,
            ad.offer_generation,
        ));
        let mut observed = PicoSpawnObservation {
            proof: SpawnAdmissionProof {
                invitation_id: basis.invitation.invitation_id.clone(),
                body_id: basis.invitation.body_id.clone(),
                host_id: ad.host_id.clone(),
                boot_id: ad.boot_id.clone(),
                nonce: basis.invitation.nonce,
                signature,
            },
            advertisement: ad,
            verifying_key: key.verifying_key(),
            spore_id: basis.spore_id.clone(),
            image_id: basis.image_id.clone(),
        };
        let request = basis.request(&observed, 1000).unwrap();
        assert!(!request.membership_admitted && !request.plan_created && !request.play_created);
        for change in 0..6 {
            let mut wrong = basis.clone();
            match change {
                0 => wrong.expected_host = "host/other".into(),
                1 => wrong.expected_profile = "profile/other".into(),
                2 => wrong.member_verifying_key = [8; 32],
                3 => {
                    wrong.invitation.body_id =
                        serde_json::from_value(serde_json::json!("body/other")).unwrap()
                }
                4 => wrong.image_id = "image/other".into(),
                _ => wrong.invitation.nonce = [4; 32],
            }
            assert!(wrong.request(&observed, 1000).is_err());
        }
        assert!(basis.request(&observed, 2000).is_err());
        observed.advertisement.offer_generation.0 += 1;
        assert!(
            basis.request(&observed, 1000).is_err(),
            "proof signs actual generation"
        );
        observed.advertisement.offer_generation.0 -= 1;
        observed.proof.signature[0] ^= 1;
        assert!(basis.request(&observed, 1000).is_err());
        let public = serde_json::to_value(&basis).unwrap();
        assert!(public.get("secret").is_none());
    }
}
