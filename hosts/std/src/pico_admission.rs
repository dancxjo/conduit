//! Native USB transport adapter for inert provisioned-Pico admission.

use std::time::Duration;

use conduit_body::{
    AdmissionChallenge, AdmissionId, AmbientAdmissionProof, BodyId, CandidateObservation,
    DiscoveryProofId, PicoAdmissionAdvertisement, PicoAdmissionChallenge,
    MAX_PICO_ADMISSION_FRAME_BYTES, PICO_ADMISSION_PROTOCOL, PICO_ADMISSION_REQUEST,
};
use conduit_core::{BootId, HostId, LinkBindingId, SignId};
use serde::Deserialize;

use crate::usb_cdc::{NativePathCdcLine, NativeUsbCdcError};

#[derive(Debug)]
pub enum PicoAdmissionTransportError {
    Usb(NativeUsbCdcError),
    Malformed,
    WrongProtocol,
    InvalidVerifyingKey,
    InvalidProof,
    Oversized,
}

impl From<NativeUsbCdcError> for PicoAdmissionTransportError {
    fn from(error: NativeUsbCdcError) -> Self {
        Self::Usb(error)
    }
}

pub struct PicoAdmissionArrival {
    pub observation: CandidateObservation,
    pub verifying_key: [u8; 32],
    pub socket: PicoAdmissionSocket,
}

impl PicoAdmissionArrival {
    pub fn into_socket(self) -> PicoAdmissionSocket {
        self.socket
    }
}

pub struct PicoAdmissionSocket {
    line: NativePathCdcLine,
}

impl PicoAdmissionSocket {
    pub fn open(path: &str) -> Result<Self, PicoAdmissionTransportError> {
        Ok(Self {
            line: NativePathCdcLine::open(path, MAX_PICO_ADMISSION_FRAME_BYTES)?,
        })
    }

    pub fn observe(
        mut self,
        binding_id: LinkBindingId,
        sign_id: SignId,
        proof_id: DiscoveryProofId,
        timeout: Duration,
    ) -> Result<PicoAdmissionArrival, PicoAdmissionTransportError> {
        self.line
            .send_raw_stream_frame(PICO_ADMISSION_REQUEST, timeout)?;
        let mut bytes = [0u8; MAX_PICO_ADMISSION_FRAME_BYTES];
        let encoded = self.line.receive_raw_stream_frame(&mut bytes, timeout)?;
        let frame = decode_advertisement(encoded)?;
        let verifying_key: [u8; 32] = frame
            .verifying_key
            .as_slice()
            .try_into()
            .map_err(|_| PicoAdmissionTransportError::InvalidVerifyingKey)?;
        let observation = CandidateObservation {
            advertisement: frame.advertisement,
            friendly_label: frame.friendly_label,
            observed_binding_id: binding_id,
            observation_sign_id: sign_id,
            proof_id,
            freshness_sequence: frame.freshness_sequence,
            encoded_bytes: u32::try_from(encoded.len())
                .map_err(|_| PicoAdmissionTransportError::Oversized)?,
        };
        Ok(PicoAdmissionArrival {
            observation,
            verifying_key,
            socket: self,
        })
    }

    pub fn prove(
        mut self,
        challenge: &AdmissionChallenge,
        timeout: Duration,
    ) -> Result<(AmbientAdmissionProof, Self), PicoAdmissionTransportError> {
        let mut output = [0u8; 1024];
        let length = encode_challenge(challenge, &mut output)?;
        self.line
            .send_raw_stream_frame(&output[..length], timeout)?;
        let mut input = [0u8; 1024];
        let proof = self.line.receive_raw_stream_frame(&mut input, timeout)?;
        Ok((decode_proof(proof)?, self))
    }

    pub fn is_connected(&self) -> Result<bool, PicoAdmissionTransportError> {
        Ok(self.line.is_connected()?)
    }
}

pub fn decode_advertisement(
    encoded: &[u8],
) -> Result<PicoAdmissionAdvertisement, PicoAdmissionTransportError> {
    if encoded.is_empty() || encoded.len() > MAX_PICO_ADMISSION_FRAME_BYTES {
        return Err(PicoAdmissionTransportError::Oversized);
    }
    let frame: PicoAdmissionAdvertisement =
        serde_json::from_slice(encoded).map_err(|_| PicoAdmissionTransportError::Malformed)?;
    if frame.protocol != PICO_ADMISSION_PROTOCOL {
        return Err(PicoAdmissionTransportError::WrongProtocol);
    }
    if frame.verifying_key.len() != 32 {
        return Err(PicoAdmissionTransportError::InvalidVerifyingKey);
    }
    Ok(frame)
}

pub fn encode_challenge(
    challenge: &AdmissionChallenge,
    output: &mut [u8],
) -> Result<usize, PicoAdmissionTransportError> {
    let frame = PicoAdmissionChallenge {
        protocol: PICO_ADMISSION_PROTOCOL,
        admission_id: challenge.admission_id.as_str(),
        body_id: challenge.body_id.as_str(),
        host_id: challenge.host_id.as_str(),
        boot_id: challenge.boot_id.as_str(),
        offer_generation: challenge.offer_generation.0,
        nonce: challenge.nonce,
        issued_at_millis: challenge.issued_at_millis,
        expires_at_millis: challenge.expires_at_millis,
    };
    let encoded = serde_json::to_vec(&frame).map_err(|_| PicoAdmissionTransportError::Malformed)?;
    if encoded.len() > output.len() || encoded.len() > 1024 {
        return Err(PicoAdmissionTransportError::Oversized);
    }
    output[..encoded.len()].copy_from_slice(&encoded);
    Ok(encoded.len())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedProof {
    protocol: u16,
    admission_id: AdmissionId,
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    nonce: Vec<u8>,
    signature: Vec<u8>,
}

pub fn decode_proof(encoded: &[u8]) -> Result<AmbientAdmissionProof, PicoAdmissionTransportError> {
    if encoded.is_empty() || encoded.len() > 1024 {
        return Err(PicoAdmissionTransportError::Oversized);
    }
    let proof: OwnedProof =
        serde_json::from_slice(encoded).map_err(|_| PicoAdmissionTransportError::Malformed)?;
    if proof.protocol != PICO_ADMISSION_PROTOCOL {
        return Err(PicoAdmissionTransportError::WrongProtocol);
    }
    Ok(AmbientAdmissionProof {
        admission_id: proof.admission_id,
        body_id: proof.body_id,
        host_id: proof.host_id,
        boot_id: proof.boot_id,
        nonce: proof
            .nonce
            .try_into()
            .map_err(|_| PicoAdmissionTransportError::InvalidProof)?,
        signature: proof
            .signature
            .try_into()
            .map_err(|_| PicoAdmissionTransportError::InvalidProof)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        AdmissionManager, AdmissionSigns, Body, BodyMembership, CandidateInventory, CandidateState,
        PicoAdmissionProof,
    };
    use conduit_core::{CheckedFormId, SourceDocumentId};
    use conduit_signal_conformance::pico_local_advertisement;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn codec_carries_exact_advertisement_and_rejects_protocol_and_size() {
        let mut advertisement = pico_local_advertisement();
        advertisement.host_id = HostId::from("pico/provisioned");
        advertisement.boot_id = BootId::from("pico/boot-1");
        advertisement.offer_generation = conduit_core::OfferGeneration(3);
        let frame = PicoAdmissionAdvertisement {
            protocol: PICO_ADMISSION_PROTOCOL,
            advertisement: advertisement.clone(),
            friendly_label: "Pico W".into(),
            verifying_key: vec![7; 32],
            freshness_sequence: 1,
        };
        let encoded = serde_json::to_vec(&frame).unwrap();
        assert!(encoded.len() < MAX_PICO_ADMISSION_FRAME_BYTES);
        assert_eq!(
            decode_advertisement(&encoded).unwrap().advertisement,
            advertisement
        );

        let mut wrong = frame.clone();
        wrong.protocol += 1;
        assert!(matches!(
            decode_advertisement(&serde_json::to_vec(&wrong).unwrap()),
            Err(PicoAdmissionTransportError::WrongProtocol)
        ));
        assert!(matches!(
            decode_advertisement(&vec![0; MAX_PICO_ADMISSION_FRAME_BYTES + 1]),
            Err(PicoAdmissionTransportError::Oversized)
        ));
        assert!(matches!(
            decode_advertisement(b"{not-json"),
            Err(PicoAdmissionTransportError::Malformed)
        ));

        let mut invalid_key = frame;
        invalid_key.verifying_key.pop();
        assert!(matches!(
            decode_advertisement(&serde_json::to_vec(&invalid_key).unwrap()),
            Err(PicoAdmissionTransportError::InvalidVerifyingKey)
        ));
    }

    #[test]
    fn decoded_pico_proof_enters_only_the_canonical_admission_manager() {
        let key = SigningKey::from_bytes(&[31; 32]);
        let body = Body::born(
            SourceDocumentId::from("source/pico-wire"),
            CheckedFormId::from("checked/pico-wire"),
            1,
            SignId::from("sign/body-born"),
        )
        .unwrap();
        let mut advertisement = pico_local_advertisement();
        advertisement.host_id = HostId::from("pico/provisioned");
        advertisement.boot_id = BootId::from("pico/boot-1");
        advertisement.offer_generation = conduit_core::OfferGeneration(3);
        let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
        let candidate_id = candidates
            .observe(CandidateObservation {
                advertisement,
                friendly_label: "Pico W".into(),
                observed_binding_id: LinkBindingId::from("line/usb-pico"),
                observation_sign_id: SignId::from("sign/pico-observed"),
                proof_id: DiscoveryProofId::bind("proof/pico-usb").unwrap(),
                freshness_sequence: 1,
                encoded_bytes: 2_300,
            })
            .unwrap();
        let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
        let challenge = manager
            .begin_ambient(
                &mut candidates,
                &candidate_id,
                key.verifying_key().to_bytes(),
                [44; 32],
                1_000,
                2_000,
                SignId::from("sign/admission-requested"),
            )
            .unwrap();
        let signature = key.sign(&challenge.signing_transcript()).to_bytes();
        let wire = PicoAdmissionProof {
            protocol: PICO_ADMISSION_PROTOCOL,
            admission_id: challenge.admission_id.as_str(),
            body_id: challenge.body_id.as_str(),
            host_id: challenge.host_id.as_str(),
            boot_id: challenge.boot_id.as_str(),
            nonce: challenge.nonce,
            signature: &signature,
        };
        let encoded = serde_json::to_vec(&wire).unwrap();
        let proof = decode_proof(&encoded).unwrap();
        assert!(matches!(
            decode_proof(b"{not-json"),
            Err(PicoAdmissionTransportError::Malformed)
        ));
        assert!(matches!(
            decode_proof(&vec![0; 1025]),
            Err(PicoAdmissionTransportError::Oversized)
        ));
        let mut wrong_protocol = serde_json::to_value(&wire).unwrap();
        wrong_protocol["protocol"] = serde_json::json!(PICO_ADMISSION_PROTOCOL + 1);
        assert!(matches!(
            decode_proof(&serde_json::to_vec(&wrong_protocol).unwrap()),
            Err(PicoAdmissionTransportError::WrongProtocol)
        ));
        let mut wrong_nonce = serde_json::to_value(&wire).unwrap();
        wrong_nonce["nonce"] = serde_json::json!([1, 2, 3]);
        assert!(matches!(
            decode_proof(&serde_json::to_vec(&wrong_nonce).unwrap()),
            Err(PicoAdmissionTransportError::InvalidProof)
        ));
        assert!(matches!(
            encode_challenge(&challenge, &mut [0; 1]),
            Err(PicoAdmissionTransportError::Oversized)
        ));
        let mut membership = BodyMembership::new(body.body_id).unwrap();
        assert!(membership.parts.is_empty());
        let mut stale = proof.clone();
        stale.boot_id = BootId::from("pico/boot-stale");
        assert_eq!(
            manager.complete_ambient(
                &mut candidates,
                &mut membership,
                &stale,
                1_001,
                AdmissionSigns {
                    part_admitted: SignId::from("sign/stale-part"),
                    host_attached: SignId::from("sign/stale-host"),
                    candidate_admitted: SignId::from("sign/stale-candidate"),
                },
            ),
            Err(conduit_body::AdmissionRefusal::StaleBoot)
        );
        assert!(membership.parts.is_empty());
        manager
            .complete_ambient(
                &mut candidates,
                &mut membership,
                &proof,
                1_001,
                AdmissionSigns {
                    part_admitted: SignId::from("sign/part-admitted"),
                    host_attached: SignId::from("sign/host-attached"),
                    candidate_admitted: SignId::from("sign/candidate-admitted"),
                },
            )
            .unwrap();
        assert_eq!(membership.parts.len(), 1);
        assert_eq!(candidates.candidates[0].state, CandidateState::Admitted);
        let retained = membership.clone();
        assert_eq!(
            manager.complete_ambient(
                &mut candidates,
                &mut membership,
                &proof,
                1_002,
                AdmissionSigns {
                    part_admitted: SignId::from("sign/replay-part"),
                    host_attached: SignId::from("sign/replay-host"),
                    candidate_admitted: SignId::from("sign/replay-candidate"),
                },
            ),
            Err(conduit_body::AdmissionRefusal::Replay)
        );
        assert_eq!(membership, retained);
    }
}
