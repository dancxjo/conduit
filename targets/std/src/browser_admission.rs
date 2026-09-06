//! Bounded transport frames for browser Body admission.
//!
//! This adapter carries untrusted observations and proof material. It owns no
//! candidate, membership, authority, Plan, or runtime truth; callers pass
//! decoded values through `conduit_body`'s canonical state machines.

use conduit_body::{
    AdmissionChallenge, AdmissionId, BodyBiographyEvidence, BodyId, HostOfferProjection,
    MembershipCredential, MembershipCredentialId, OfferDisclosureRequest, OfferDisclosureStage,
    PartId, PartReturnChallenge, SpawnInvitationId, ADMISSION_SIGNATURE_BYTES,
    MAX_CANDIDATE_ADVERTISEMENT_BYTES, MAX_CANDIDATE_LABEL_BYTES, MAX_DISCLOSED_CAPABILITIES,
    MAX_DISCLOSED_RESOURCES,
};
use conduit_core::{BootId, HostAdvertisement, HostId, PlanId, PortId, ResourceHandleId};
use conduit_human::{AcquiredMediaResource, MediaResourceAvailability};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener};

mod egress;
mod offer_evidence;
mod webrtc_signaling;

pub use webrtc_signaling::{
    browser_webrtc_line_contract, BrowserWebRtcDescription, BrowserWebRtcGrant,
    BrowserWebRtcRendezvous, BrowserWebRtcRendezvousRefusal, BrowserWebRtcRole,
    BrowserWebRtcSignal, RoutedBrowserWebRtcSignal, MAX_WEBRTC_DESCRIPTION_BYTES,
    MAX_WEBRTC_NEGOTIATIONS, MAX_WEBRTC_SESSION_HELLO_BYTES,
};

pub const BROWSER_ADMISSION_PROTOCOL: u16 = 1;
pub const MAX_WEBRTC_GRANT_GENERATIONS: u16 = 2;
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
    PresenceRenewal {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        sequence: u64,
    },
    PresenceLeave {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        sequence: u64,
    },
    OfferDisclosureRequest {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        request: OfferDisclosureRequest,
    },
    MediaResourceTruth {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        resource: AcquiredMediaResource,
    },
    WebRtcSignal {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        target_host_id: HostId,
        target_boot_id: BootId,
        signal: BrowserWebRtcSignal,
    },
    WebRtcGrantRequest {
        protocol: u16,
        credential_id: MembershipCredentialId,
        body_id: BodyId,
        part_id: PartId,
        host_id: HostId,
        boot_id: BootId,
        generation: u16,
        index: u16,
    },
    ReturnAdvertise {
        protocol: u16,
        credential: MembershipCredential,
        advertisement: HostAdvertisement,
    },
    ReturnProof {
        protocol: u16,
        admission_id: AdmissionId,
        body_id: BodyId,
        part_id: PartId,
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
    BiographyEvidence {
        protocol: u16,
        evidence: Box<BodyBiographyEvidence>,
    },
    OfferEvidence {
        protocol: u16,
        evidence: Box<HostOfferProjection>,
    },
    PresenceAccepted {
        protocol: u16,
        sequence: u64,
        renew_after_millis: u64,
        expires_at_millis: u64,
    },
    MediaUsePlan {
        protocol: u16,
        plan_id: PlanId,
        resource_handle: ResourceHandleId,
        output_port: PortId,
    },
    WebRtcPlanReady {
        protocol: u16,
        generation: u16,
        plan_id: PlanId,
    },
    WebRtcSignal {
        protocol: u16,
        source_host_id: HostId,
        source_boot_id: BootId,
        signal: BrowserWebRtcSignal,
    },
    WebRtcGrant {
        protocol: u16,
        generation: u16,
        index: u16,
        total: u16,
        grant: Option<BrowserWebRtcGrant>,
    },
    ReturnChallenge {
        protocol: u16,
        challenge: PartReturnChallenge,
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
    InvalidSequence,
    InvalidMediaResource,
    InvalidBiographyEvidence,
    InvalidOfferEvidence,
    InvalidOfferDisclosureRequest,
    InvalidSignal,
    InvalidGrant,
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

    pub fn accept(&self) -> Result<BrowserAdmissionSocket, BrowserAdmissionSocketError> {
        Ok(BrowserAdmissionSocket {
            line: self.inner.accept()?,
            input: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
            output: vec![0; MAX_BROWSER_ADMISSION_FRAME_BYTES].into_boxed_slice(),
        })
    }
}

pub struct BrowserAdmissionSocket {
    line: NativeWebSocketLine,
    // Fixed at admission, never resized during receive/send. Moving a socket
    // through coordinator state must not copy its frame arenas on the stack.
    input: Box<[u8]>,
    output: Box<[u8]>,
}

impl BrowserAdmissionSocket {
    pub fn set_read_timeout(
        &self,
        timeout: Option<Duration>,
    ) -> Result<(), BrowserAdmissionSocketError> {
        Ok(self.line.set_read_timeout(timeout)?)
    }

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

    pub fn close(&mut self) -> Result<(), BrowserAdmissionSocketError> {
        Ok(self.line.close()?)
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
    egress::validate(frame)?;
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
        BrowserAdmissionIngress::PresenceRenewal {
            protocol, sequence, ..
        }
        | BrowserAdmissionIngress::PresenceLeave {
            protocol, sequence, ..
        } => {
            if *sequence == 0 {
                return Err(BrowserAdmissionFrameError::InvalidSequence);
            }
            protocol
        }
        BrowserAdmissionIngress::MediaResourceTruth {
            protocol,
            host_id,
            boot_id,
            resource,
            ..
        } => {
            if resource.host_id != *host_id
                || resource.boot_id != *boot_id
                || resource.availability != MediaResourceAvailability::Available
                || resource.handle_id.as_str().is_empty()
                || resource.class_id.as_str().is_empty()
                || resource.value_kind.as_str().is_empty()
                || resource.use_authority_contract.as_str().is_empty()
                || resource.use_authority_grant.as_str().is_empty()
                || !resource.settings.is_valid()
                || !resource.flow_bounds.is_finite_and_valid()
            {
                return Err(BrowserAdmissionFrameError::InvalidMediaResource);
            }
            protocol
        }
        BrowserAdmissionIngress::OfferDisclosureRequest {
            protocol, request, ..
        } => {
            if request.stage != OfferDisclosureStage::Planning
                || request.capability_ids.len() > MAX_DISCLOSED_CAPABILITIES
                || request.resource_pool_ids.len() > MAX_DISCLOSED_RESOURCES
                || (request.capability_ids.is_empty() && request.resource_pool_ids.is_empty())
                || request
                    .capability_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || request
                    .resource_pool_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(BrowserAdmissionFrameError::InvalidOfferDisclosureRequest);
            }
            protocol
        }
        BrowserAdmissionIngress::WebRtcSignal {
            protocol, signal, ..
        } => {
            signal.validate()?;
            protocol
        }
        BrowserAdmissionIngress::WebRtcGrantRequest {
            protocol,
            generation,
            index,
            ..
        } => {
            if *generation >= MAX_WEBRTC_GRANT_GENERATIONS
                || usize::from(*index) >= MAX_WEBRTC_NEGOTIATIONS
            {
                return Err(BrowserAdmissionFrameError::InvalidGrant);
            }
            protocol
        }
        BrowserAdmissionIngress::ReturnAdvertise { protocol, .. } => protocol,
        BrowserAdmissionIngress::ReturnProof {
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

#[cfg(test)]
#[path = "browser_admission/tests.rs"]
mod tests;
