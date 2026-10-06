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
use conduit_presentation::{
    FaceInteraction, MaskShow, MaskWardrobeAction, OwnerFaceSnapshotRequest,
    OwnerFaceSnapshotResponse, MAX_FACE_INTERACTION_BYTES, MAX_OWNER_FACE_RESPONSE_BYTES,
    OWNER_FACE_RESPONSE_SCHEMA,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::websocket::{NativeWebSocketError, NativeWebSocketLine, NativeWebSocketListener};

mod egress;
mod offer_evidence;
mod webrtc_signaling;

pub use webrtc_signaling::{
    browser_webrtc_line_contract, BrowserWebRtcDescription, BrowserWebRtcGrant,
    BrowserWebRtcRendezvous, BrowserWebRtcRendezvousRefusal, BrowserWebRtcRole,
    BrowserWebRtcSignal, RoutedBrowserWebRtcSignal, WebRtcBootstrapConfiguration, WebRtcIceServer,
    WebRtcIceTransportPolicy, MAX_WEBRTC_BOOTSTRAP_LIFETIME_MILLIS, MAX_WEBRTC_DESCRIPTION_BYTES,
    MAX_WEBRTC_ICE_SERVERS, MAX_WEBRTC_ICE_TEXT_BYTES, MAX_WEBRTC_ICE_URLS_PER_SERVER,
    MAX_WEBRTC_NEGOTIATIONS, MAX_WEBRTC_SESSION_HELLO_BYTES,
};

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
    FaceSnapshotRequest {
        protocol: u16,
        request: OwnerFaceSnapshotRequest,
    },
    FaceShowAcknowledgement {
        protocol: u16,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
    },
    FaceWardrobeRequest {
        protocol: u16,
        request_id: String,
        request: OwnerFaceSnapshotRequest,
        owner_plan_id: Option<PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    },
    FaceInteractionRequest {
        protocol: u16,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
        interaction: FaceInteraction,
    },
    SelectedSpeechStart {
        protocol: u16,
        request_id: String,
        request: OwnerFaceSnapshotRequest,
        show: Box<MaskShow>,
    },
    SelectedSpeechStatus {
        protocol: u16,
        request_id: String,
        operation_id: String,
    },
    SelectedSpeechStop {
        protocol: u16,
        request_id: String,
        operation_id: String,
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
        generation: u32,
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
    FaceSnapshotResponse {
        protocol: u16,
        response: OwnerFaceSnapshotResponse,
    },
    FaceShowResponse {
        protocol: u16,
        accepted: bool,
        code: String,
    },
    FaceWardrobeResponse {
        protocol: u16,
        request_id: String,
        accepted: bool,
        code: String,
        report: Option<Box<Value>>,
    },
    FaceInteractionResponse {
        protocol: u16,
        accepted: bool,
        code: String,
    },
    SelectedSpeechResponse {
        protocol: u16,
        request_id: String,
        outcome: String,
        operation_id: Option<String>,
        status: Option<Box<Value>>,
        code: Option<String>,
    },
    MediaUsePlan {
        protocol: u16,
        plan_id: PlanId,
        resource_handle: ResourceHandleId,
        output_port: PortId,
    },
    WebRtcPlanReady {
        protocol: u16,
        generation: u32,
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
        generation: u32,
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
    InvalidFaceSnapshot,
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
        BrowserAdmissionIngress::FaceSnapshotRequest { protocol, request } => {
            if !request.has_exact_basis()
                || request
                    .last_seen_identity
                    .as_ref()
                    .is_some_and(|id| id.as_str().is_empty() || id.as_str().len() > 256)
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionIngress::FaceShowAcknowledgement {
            protocol,
            request,
            show,
        } => {
            if !request.has_exact_basis()
                || show.show.host_id != request.host_id
                || show.show.boot_id != request.boot_id
                || show.show.body_id.as_ref() != Some(&request.body_id)
                || serde_json::to_vec(show)
                    .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
                    .len()
                    > 64 * 1024
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionIngress::FaceWardrobeRequest {
            protocol,
            request_id,
            request,
            owner_plan_id,
            action,
            ..
        } => {
            if !request.has_exact_basis()
                || !valid_wardrobe_request_id(request_id)
                || owner_plan_id.is_some() != action.is_some()
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionIngress::FaceInteractionRequest {
            protocol,
            request,
            show,
            interaction,
        } => {
            if !request.has_exact_basis()
                || interaction.encode().len() > MAX_FACE_INTERACTION_BYTES
                || interaction.show_id != show.show_id.as_str()
                || interaction.face_revision != show.presentation_revision
                || interaction.face_id != show.presentation_id.as_str()
                || show.show.host_id != request.host_id
                || show.show.boot_id != request.boot_id
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionIngress::SelectedSpeechStart {
            protocol,
            request_id,
            request,
            show,
        } => {
            if !valid_speech_id(request_id)
                || !request.has_exact_basis()
                || show.show.host_id != request.host_id
                || show.show.boot_id != request.boot_id
                || show.show.body_id.as_ref() != Some(&request.body_id)
                || serde_json::to_vec(show)
                    .map_err(|_| BrowserAdmissionFrameError::InvalidFaceSnapshot)?
                    .len()
                    > 64 * 1024
            {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
            }
            protocol
        }
        BrowserAdmissionIngress::SelectedSpeechStatus {
            protocol,
            request_id,
            operation_id,
        }
        | BrowserAdmissionIngress::SelectedSpeechStop {
            protocol,
            request_id,
            operation_id,
        } => {
            if !valid_speech_id(request_id) || !valid_speech_id(operation_id) {
                return Err(BrowserAdmissionFrameError::InvalidFaceSnapshot);
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
            protocol, index, ..
        } => {
            if usize::from(*index) >= MAX_WEBRTC_NEGOTIATIONS {
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

fn valid_speech_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn valid_wardrobe_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

#[cfg(test)]
mod wardrobe_wire_tests {
    use super::*;

    #[test]
    fn wardrobe_request_uses_correlated_revision_bound_action_shape() {
        let body = conduit_body::Body::born(
            "source/wardrobe-test".into(),
            "checked/wardrobe-test".into(),
            1,
            "sign/birth".into(),
        )
        .unwrap();
        let part_id = PartId::bind(&body.body_id, "browser", 1).unwrap();
        let mask = conduit_core::PlotIdentity {
            source_document_id: conduit_core::SourceDocumentId::from("source/mask"),
            checked_plot_id: conduit_core::CheckedPlotId::from("checked/mask"),
            expanded_plot_id: conduit_core::ExpandedPlotId::from("expanded/mask"),
        };
        let request = OwnerFaceSnapshotRequest {
            schema: conduit_presentation::OWNER_FACE_REQUEST_SCHEMA.into(),
            credential_id: "credential/test".into(),
            body_id: body.body_id,
            part_id,
            host_id: HostId::from("host/test"),
            boot_id: BootId::from("boot/test"),
            last_seen_revision: None,
            last_seen_identity: None,
        };
        let frame = BrowserAdmissionIngress::FaceWardrobeRequest {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            request_id: "wardrobe:1".into(),
            request,
            owner_plan_id: Some(PlanId::from("plan/owner")),
            basis_revision: 7,
            action: Some(MaskWardrobeAction::Wear(mask)),
        };
        let value = serde_json::to_value(&frame).unwrap();
        assert_eq!(value["kind"], "face-wardrobe-request");
        assert_eq!(value["request_id"], "wardrobe:1");
        assert_eq!(value["owner_plan_id"], "plan/owner");
        assert_eq!(value["basis_revision"], 7);
        assert_eq!(value["action"]["Wear"]["source_document_id"], "source/mask");
        assert!(decode_browser_admission_frame(&serde_json::to_vec(&frame).unwrap()).is_ok());
        let response = BrowserAdmissionEgress::FaceWardrobeResponse {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            request_id: "wardrobe:1".into(),
            accepted: true,
            code: String::new(),
            report: Some(Box::new(
                serde_json::json!({"schema":"conduit.body/owner-mask-wardrobe@1"}),
            )),
        };
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["kind"], "face-wardrobe-response");
        assert_eq!(
            value["report"]["schema"],
            "conduit.body/owner-mask-wardrobe@1"
        );
    }
}

#[cfg(test)]
mod selected_speech_tests {
    use super::*;

    #[test]
    fn selected_speech_status_requires_a_bounded_correlated_operation() {
        let valid = br#"{"kind":"selected-speech-status","protocol":1,"request_id":"browser-speech/1","operation_id":"selected-speech/1"}"#;
        assert!(matches!(
            decode_browser_admission_frame(valid),
            Ok(BrowserAdmissionIngress::SelectedSpeechStatus { .. })
        ));
        for invalid in [
            br#"{"kind":"selected-speech-status","protocol":1,"request_id":"","operation_id":"selected-speech/1"}"#.as_slice(),
            br#"{"kind":"selected-speech-stop","protocol":1,"request_id":"browser-speech/2","operation_id":""}"#.as_slice(),
            br#"{"kind":"selected-speech-status","protocol":2,"request_id":"browser-speech/3","operation_id":"selected-speech/1"}"#.as_slice(),
        ] {
            assert!(decode_browser_admission_frame(invalid).is_err());
        }
    }
}

#[cfg(test)]
#[path = "browser_admission/tests.rs"]
mod tests;
