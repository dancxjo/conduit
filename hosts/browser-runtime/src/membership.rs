//! Browser-owned admission proof material, kept separate from renderer handles.

use conduit_body::{
    AdmissionChallenge, AdmissionRefusal, AmbientAdmissionProof, ADMISSION_SIGNATURE_BYTES,
};
use conduit_core::{BootId, HostId};
use ed25519_dalek::{Signer, SigningKey};

/// Exact identity and private proof capability for one browser Host incarnation.
///
/// The seed must come from the browser's cryptographic RNG. This type does not
/// implement `Debug` or expose the seed; DOM and window identities never enter
/// the signed admission transcript.
pub struct BrowserAdmissionIdentity {
    host_id: HostId,
    boot_id: BootId,
    signing_key: SigningKey,
}

impl BrowserAdmissionIdentity {
    pub fn from_csprng_seed(
        host_id: HostId,
        boot_id: BootId,
        seed: [u8; 32],
    ) -> Result<Self, AdmissionRefusal> {
        if host_id.as_str().is_empty()
            || boot_id.as_str().is_empty()
            || host_id.as_str().len() > 128
            || boot_id.as_str().len() > 128
        {
            return Err(AdmissionRefusal::InvalidProof);
        }
        if seed == [0; 32] {
            return Err(AdmissionRefusal::WeakSecret);
        }
        Ok(Self {
            host_id,
            boot_id,
            signing_key: SigningKey::from_bytes(&seed),
        })
    }

    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }

    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub fn verifying_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn prove(
        &self,
        challenge: &AdmissionChallenge,
    ) -> Result<AmbientAdmissionProof, AdmissionRefusal> {
        if challenge.host_id != self.host_id {
            return Err(AdmissionRefusal::WrongHost);
        }
        if challenge.boot_id != self.boot_id {
            return Err(AdmissionRefusal::StaleBoot);
        }
        let signature: [u8; ADMISSION_SIGNATURE_BYTES] = self
            .signing_key
            .sign(&challenge.signing_transcript())
            .to_bytes();
        Ok(AmbientAdmissionProof {
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            nonce: challenge.nonce,
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        AdmissionManager, AdmissionSigns, Body, BodyMembership, CandidateInventory,
        CandidateObservation, DiscoveryProofId,
    };
    use conduit_core::{
        CheckedFormId, HostAdvertisement, HostProfileId, LinkBindingId, OfferGeneration, SignId,
        SourceDocumentId, PROTOCOL_VERSION,
    };

    fn advertisement(identity: &BrowserAdmissionIdentity) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: identity.host_id().clone(),
            boot_id: identity.boot_id().clone(),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("browser/host"),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        }
    }

    #[test]
    fn browser_key_proves_exact_host_and_boot_without_becoming_identity() {
        let body = Body::born(
            SourceDocumentId::from("source/browser-admission"),
            CheckedFormId::from("checked/browser-admission"),
            1,
            SignId::from("sign/body-born"),
        )
        .unwrap();
        let identity = BrowserAdmissionIdentity::from_csprng_seed(
            HostId::from("browser/tab-a"),
            BootId::from("browser-boot/a"),
            [7; 32],
        )
        .unwrap();
        let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
        let candidate = candidates
            .observe(CandidateObservation {
                advertisement: advertisement(&identity),
                friendly_label: "forged friendly label".into(),
                observed_binding_id: LinkBindingId::from("line/browser-websocket"),
                observation_sign_id: SignId::from("sign/browser-observed"),
                proof_id: DiscoveryProofId::bind("proof/discovery-only").unwrap(),
                freshness_sequence: 1,
                encoded_bytes: 256,
            })
            .unwrap();
        let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
        let challenge = manager
            .begin_ambient(
                &mut candidates,
                &candidate,
                identity.verifying_key(),
                [9; 32],
                1_000,
                2_000,
                SignId::from("sign/admission-requested"),
            )
            .unwrap();
        let mut membership = BodyMembership::new(body.body_id).unwrap();
        manager
            .complete_ambient(
                &mut candidates,
                &mut membership,
                &identity.prove(&challenge).unwrap(),
                1_001,
                AdmissionSigns {
                    part_admitted: SignId::from("sign/part-admitted"),
                    host_attached: SignId::from("sign/host-attached"),
                    candidate_admitted: SignId::from("sign/candidate-admitted"),
                },
            )
            .unwrap();
        assert_eq!(membership.parts.len(), 1);
        assert_eq!(
            membership.parts[0].current.as_ref().unwrap().host_id,
            *identity.host_id()
        );

        let mut mismatched = challenge;
        mismatched.host_id = HostId::from("browser/forged");
        assert!(matches!(
            identity.prove(&mismatched),
            Err(AdmissionRefusal::WrongHost)
        ));
        mismatched.host_id = identity.host_id().clone();
        mismatched.boot_id = BootId::from("browser-boot/stale");
        assert!(matches!(
            identity.prove(&mismatched),
            Err(AdmissionRefusal::StaleBoot)
        ));
    }

    #[test]
    fn browser_admission_identity_refuses_weak_seed() {
        assert!(matches!(
            BrowserAdmissionIdentity::from_csprng_seed(
                HostId::from("browser/tab"),
                BootId::from("browser-boot/tab"),
                [0; 32]
            ),
            Err(AdmissionRefusal::WeakSecret)
        ));
    }
}
