//! Finite public frame and refusal types for browser WebRTC rendezvous.

use conduit_body::MAX_BODY_PARTS;
use conduit_core::{BootId, ConnectionBase, HostId, LinkBindingId};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionMessage};
use serde::{Deserialize, Serialize};

use crate::browser_admission::BrowserAdmissionFrameError;

pub const MAX_WEBRTC_SESSION_HELLO_BYTES: usize = 1_024;
pub const MAX_WEBRTC_DESCRIPTION_BYTES: usize = 4_096;
pub const MAX_WEBRTC_NEGOTIATIONS: usize = MAX_BODY_PARTS;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserWebRtcDescription {
    Offer,
    Answer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserWebRtcSignal {
    pub negotiation_id: LinkBindingId,
    pub description: BrowserWebRtcDescription,
    pub session_hello: Vec<u8>,
    pub sdp: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserWebRtcRole {
    Source,
    Sink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserWebRtcGrant {
    pub negotiation_id: LinkBindingId,
    pub role: BrowserWebRtcRole,
    pub peer_host_id: HostId,
    pub peer_boot_id: BootId,
    pub session_hello: Vec<u8>,
}

impl BrowserWebRtcGrant {
    pub(crate) fn validate(&self) -> Result<(), BrowserAdmissionFrameError> {
        if self.negotiation_id.as_str().is_empty()
            || self.peer_host_id.as_str().is_empty()
            || self.peer_boot_id.as_str().is_empty()
            || self.session_hello.is_empty()
            || self.session_hello.len() > MAX_WEBRTC_SESSION_HELLO_BYTES
        {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        }
        let frame = decode_session_frame(
            &self.session_hello,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
        )
        .map_err(|_| BrowserAdmissionFrameError::InvalidGrant)?;
        let SessionMessage::Hello(hello) = frame.message else {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        };
        if hello.base != ConnectionBase::WebRtcDataChannel
            || hello.link_binding_id != self.negotiation_id.as_str()
        {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        }
        let peer_matches = match self.role {
            BrowserWebRtcRole::Source => {
                frame.identity.sink_host_id == self.peer_host_id.as_str()
                    && frame.identity.sink_boot_id == self.peer_boot_id.as_str()
            }
            BrowserWebRtcRole::Sink => {
                frame.identity.source_host_id == self.peer_host_id.as_str()
                    && frame.identity.source_boot_id == self.peer_boot_id.as_str()
            }
        };
        if !peer_matches {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        }
        let mut canonical = [0; MAX_WEBRTC_SESSION_HELLO_BYTES];
        let length = encode_session_frame_into(
            frame,
            &mut canonical,
            frame.identity.limits.maximum_payload_bytes,
            MAX_WEBRTC_SESSION_HELLO_BYTES as u32,
        )
        .map_err(|_| BrowserAdmissionFrameError::InvalidGrant)?;
        if canonical[..length] != self.session_hello {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        }
        Ok(())
    }
}

impl BrowserWebRtcSignal {
    pub(crate) fn validate(&self) -> Result<(), BrowserAdmissionFrameError> {
        if self.negotiation_id.as_str().is_empty()
            || self.session_hello.is_empty()
            || self.session_hello.len() > MAX_WEBRTC_SESSION_HELLO_BYTES
            || self.sdp.is_empty()
            || self.sdp.len() > MAX_WEBRTC_DESCRIPTION_BYTES
        {
            return Err(BrowserAdmissionFrameError::InvalidSignal);
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BrowserWebRtcRendezvousRefusal {
    InvalidSignal,
    SourceUnavailable,
    SourceCredentialMismatch,
    TargetUnavailable,
    SessionMismatch,
    WrongDirection,
    DuplicateNegotiation,
    UnknownNegotiation,
    InvalidStage,
    CapacityExhausted,
    DuplicateGrant,
    UngrantedSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedBrowserWebRtcSignal {
    pub source_host_id: HostId,
    pub source_boot_id: BootId,
    pub target_host_id: HostId,
    pub target_boot_id: BootId,
    pub signal: BrowserWebRtcSignal,
}
