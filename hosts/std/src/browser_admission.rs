//! Bounded transport frames for browser Body admission.
//!
//! This adapter carries untrusted observations and proof material. It owns no
//! candidate, membership, authority, Plan, or runtime truth; callers pass
//! decoded values through `conduit_body`'s canonical state machines.

use conduit_body::{
    AdmissionChallenge, AdmissionId, BodyId, MembershipCredential, SpawnInvitationId,
    ADMISSION_SIGNATURE_BYTES, MAX_CANDIDATE_ADVERTISEMENT_BYTES, MAX_CANDIDATE_LABEL_BYTES,
};
use conduit_core::{BootId, HostAdvertisement, HostId};
use serde::{Deserialize, Serialize};

use crate::websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener};

pub const BROWSER_ADMISSION_PROTOCOL: u16 = 1;
pub const MAX_BROWSER_ADMISSION_FRAME_BYTES: usize =
    MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize + 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrowserAdmissionIngress {
    Advertise {
        protocol: u16,
        advertisement: HostAdvertisement,
        friendly_label: String,
        verifying_key: Vec<u8>,
        freshness_sequence: u64,
    },
    AmbientProof {
        protocol: u16,
        admission_id: AdmissionId,
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    SpawnProof {
        protocol: u16,
        invitation_id: SpawnInvitationId,
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrowserAdmissionEgress {
    Challenge {
        protocol: u16,
        challenge: AdmissionChallenge,
    },
    Admitted {
        protocol: u16,
        credential: MembershipCredential,
    },
    Refused {
        protocol: u16,
        code: String,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserAdmissionFrameError {
    Empty,
    Oversized,
    Malformed,
    WrongProtocol,
    LabelTooLong,
    InvalidVerifyingKey,
    InvalidNonce,
    InvalidSignature,
    OutputTooSmall,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserAdmissionSocketError {
    Transport(NativeWebSocketError),
    Frame(BrowserAdmissionFrameError),
}

impl From<NativeWebSocketError> for BrowserAdmissionSocketError {
    fn from(error: NativeWebSocketError) -> Self {
        Self::Transport(error)
    }
}

impl From<BrowserAdmissionFrameError> for BrowserAdmissionSocketError {
    fn from(error: BrowserAdmissionFrameError) -> Self {
        Self::Frame(error)
    }
}

pub struct BrowserAdmissionListener {
    inner: NativeWebSocketListener,
}

impl BrowserAdmissionListener {
    pub fn bind_loopback() -> Result<Self, BrowserAdmissionSocketError> {
        Ok(Self {
            inner: NativeWebSocketListener::bind_loopback(
                MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            )?,
        })
    }

    pub fn url(&self) -> Result<String, BrowserAdmissionSocketError> {
        Ok(self.inner.url()?)
    }

    pub fn accept(self) -> Result<BrowserAdmissionSocket, BrowserAdmissionSocketError> {
        Ok(BrowserAdmissionSocket {
            line: self.inner.accept()?,
            input: [0; MAX_BROWSER_ADMISSION_FRAME_BYTES],
            output: [0; MAX_BROWSER_ADMISSION_FRAME_BYTES],
        })
    }
}

pub struct BrowserAdmissionSocket {
    line: NativeWebSocketLine,
    input: [u8; MAX_BROWSER_ADMISSION_FRAME_BYTES],
    output: [u8; MAX_BROWSER_ADMISSION_FRAME_BYTES],
}

impl BrowserAdmissionSocket {
    pub fn receive(&mut self) -> Result<BrowserAdmissionIngress, BrowserAdmissionSocketError> {
        self.receive_with_size().map(|(frame, _)| frame)
    }

    pub fn receive_with_size(
        &mut self,
    ) -> Result<(BrowserAdmissionIngress, u32), BrowserAdmissionSocketError> {
        let length = self.line.receive_binary(&mut self.input)?;
        let encoded_bytes =
            u32::try_from(length).map_err(|_| BrowserAdmissionFrameError::Oversized)?;
        Ok((
            decode_browser_admission_frame(&self.input[..length])?,
            encoded_bytes,
        ))
    }

    pub fn send(
        &mut self,
        frame: &BrowserAdmissionEgress,
    ) -> Result<(), BrowserAdmissionSocketError> {
        let length = encode_browser_admission_frame(frame, &mut self.output)?;
        self.line.send_binary(&self.output[..length])?;
        Ok(())
    }
}

pub fn decode_browser_admission_frame(
    encoded: &[u8],
) -> Result<BrowserAdmissionIngress, BrowserAdmissionFrameError> {
    if encoded.is_empty() {
        return Err(BrowserAdmissionFrameError::Empty);
    }
    if encoded.len() > MAX_BROWSER_ADMISSION_FRAME_BYTES {
        return Err(BrowserAdmissionFrameError::Oversized);
    }
    let frame = serde_json::from_slice::<BrowserAdmissionIngress>(encoded)
        .map_err(|_| BrowserAdmissionFrameError::Malformed)?;
    validate_ingress(&frame)?;
    Ok(frame)
}

pub fn encode_browser_admission_frame(
    frame: &BrowserAdmissionEgress,
    output: &mut [u8],
) -> Result<usize, BrowserAdmissionFrameError> {
    validate_egress(frame)?;
    let encoded = serde_json::to_vec(frame).map_err(|_| BrowserAdmissionFrameError::Malformed)?;
    if encoded.len() > MAX_BROWSER_ADMISSION_FRAME_BYTES || encoded.len() > output.len() {
        return Err(BrowserAdmissionFrameError::OutputTooSmall);
    }
    output[..encoded.len()].copy_from_slice(&encoded);
    Ok(encoded.len())
}

fn validate_ingress(frame: &BrowserAdmissionIngress) -> Result<(), BrowserAdmissionFrameError> {
    let protocol = match frame {
        BrowserAdmissionIngress::Advertise {
            protocol,
            friendly_label,
            verifying_key,
            ..
        } => {
            if friendly_label.len() > MAX_CANDIDATE_LABEL_BYTES {
                return Err(BrowserAdmissionFrameError::LabelTooLong);
            }
            if verifying_key.len() != 32 {
                return Err(BrowserAdmissionFrameError::InvalidVerifyingKey);
            }
            protocol
        }
        BrowserAdmissionIngress::AmbientProof {
            protocol,
            nonce,
            signature,
            ..
        }
        | BrowserAdmissionIngress::SpawnProof {
            protocol,
            nonce,
            signature,
            ..
        } => {
            if nonce.len() != 32 {
                return Err(BrowserAdmissionFrameError::InvalidNonce);
            }
            if signature.len() != ADMISSION_SIGNATURE_BYTES {
                return Err(BrowserAdmissionFrameError::InvalidSignature);
            }
            protocol
        }
    };
    if *protocol != BROWSER_ADMISSION_PROTOCOL {
        return Err(BrowserAdmissionFrameError::WrongProtocol);
    }
    Ok(())
}

fn validate_egress(frame: &BrowserAdmissionEgress) -> Result<(), BrowserAdmissionFrameError> {
    let protocol = match frame {
        BrowserAdmissionEgress::Challenge { protocol, .. }
        | BrowserAdmissionEgress::Admitted { protocol, .. }
        | BrowserAdmissionEgress::Refused { protocol, .. } => protocol,
    };
    (*protocol == BROWSER_ADMISSION_PROTOCOL)
        .then_some(())
        .ok_or(BrowserAdmissionFrameError::WrongProtocol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{HostProfileId, OfferGeneration, PROTOCOL_VERSION};
    use std::net::SocketAddr;

    fn advertisement() -> BrowserAdmissionIngress {
        BrowserAdmissionIngress::Advertise {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            advertisement: HostAdvertisement {
                protocol_version: PROTOCOL_VERSION,
                host_id: HostId::from("browser/frame-host"),
                boot_id: BootId::from("browser/frame-boot"),
                offer_generation: OfferGeneration(1),
                profile: HostProfileId::from("browser/frame-profile"),
                resources: Vec::new(),
                capabilities: Vec::new(),
                planner_capabilities: Vec::new(),
            },
            friendly_label: "Browser".into(),
            verifying_key: vec![7; 32],
            freshness_sequence: 1,
        }
    }

    #[test]
    fn bounded_advertisement_round_trips_without_creating_membership() {
        let encoded = serde_json::to_vec(&advertisement()).unwrap();
        assert_eq!(
            decode_browser_admission_frame(&encoded),
            Ok(advertisement())
        );
    }

    #[test]
    fn malformed_oversized_and_bad_key_frames_are_distinct() {
        assert_eq!(
            decode_browser_admission_frame(b"{"),
            Err(BrowserAdmissionFrameError::Malformed)
        );
        assert_eq!(
            decode_browser_admission_frame(&vec![b'x'; MAX_BROWSER_ADMISSION_FRAME_BYTES + 1]),
            Err(BrowserAdmissionFrameError::Oversized)
        );
        let BrowserAdmissionIngress::Advertise {
            protocol,
            advertisement,
            friendly_label,
            freshness_sequence,
            ..
        } = advertisement()
        else {
            unreachable!()
        };
        let bad_key = BrowserAdmissionIngress::Advertise {
            protocol,
            advertisement,
            friendly_label,
            verifying_key: vec![0; 31],
            freshness_sequence,
        };
        assert_eq!(
            decode_browser_admission_frame(&serde_json::to_vec(&bad_key).unwrap()),
            Err(BrowserAdmissionFrameError::InvalidVerifyingKey)
        );
    }

    #[test]
    fn egress_refuses_wrong_protocol_and_too_small_output() {
        let wrong = BrowserAdmissionEgress::Refused {
            protocol: 2,
            code: "wrong-body".into(),
        };
        assert_eq!(
            encode_browser_admission_frame(&wrong, &mut [0; 128]),
            Err(BrowserAdmissionFrameError::WrongProtocol)
        );
        let frame = BrowserAdmissionEgress::Refused {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            code: "invalid-proof".into(),
        };
        assert_eq!(
            encode_browser_admission_frame(&frame, &mut [0; 1]),
            Err(BrowserAdmissionFrameError::OutputTooSmall)
        );
    }

    #[test]
    fn loopback_socket_carries_one_exact_inert_advertisement_and_refusal() {
        let listener = BrowserAdmissionListener::bind_loopback().unwrap();
        let url = listener.url().unwrap();
        let address: SocketAddr = url
            .strip_prefix("ws://")
            .unwrap()
            .strip_suffix("/conduit")
            .unwrap()
            .parse()
            .unwrap();
        let expected = advertisement();
        let client_url = url.clone();
        let client = std::thread::spawn(move || {
            let mut line = NativeWebSocketLine::connect(
                address,
                &client_url,
                MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            )
            .unwrap();
            line.send_binary(&serde_json::to_vec(&expected).unwrap())
                .unwrap();
            let mut response = [0; MAX_BROWSER_ADMISSION_FRAME_BYTES];
            let length = line.receive_binary(&mut response).unwrap();
            serde_json::from_slice::<BrowserAdmissionEgress>(&response[..length]).unwrap()
        });
        let mut socket = listener.accept().unwrap();
        assert_eq!(socket.receive().unwrap(), advertisement());
        socket
            .send(&BrowserAdmissionEgress::Refused {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                code: "explicit-operator-refusal".into(),
            })
            .unwrap();
        assert_eq!(
            client.join().unwrap(),
            BrowserAdmissionEgress::Refused {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                code: "explicit-operator-refusal".into(),
            }
        );
    }
}
