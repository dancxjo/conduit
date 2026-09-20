//! Finite public frame and refusal types for browser WebRTC rendezvous.

use conduit_body::MAX_BODY_PARTS;
use conduit_core::{BootId, HostId, LinkBindingId};
use conduit_wire::{decode_session_frame, encode_session_frame_into, SessionMessage};
use serde::{Deserialize, Serialize};

use crate::browser_admission::BrowserAdmissionFrameError;

pub const MAX_WEBRTC_SESSION_HELLO_BYTES: usize = 1_024;
pub const MAX_WEBRTC_DESCRIPTION_BYTES: usize = 4_096;
pub const MAX_WEBRTC_NEGOTIATIONS: usize = MAX_BODY_PARTS;
pub const MAX_WEBRTC_ICE_SERVERS: usize = 4;
pub const MAX_WEBRTC_ICE_URLS_PER_SERVER: usize = 4;
pub const MAX_WEBRTC_ICE_TEXT_BYTES: usize = 512;
pub const MAX_WEBRTC_BOOTSTRAP_LIFETIME_MILLIS: u64 = 10 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebRtcIceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebRtcIceTransportPolicy {
    DirectAndRelay,
    RelayOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebRtcBootstrapConfiguration {
    pub provider_implementation_id: String,
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
    pub transport_policy: WebRtcIceTransportPolicy,
    pub ice_servers: Vec<WebRtcIceServer>,
}

impl WebRtcBootstrapConfiguration {
    pub fn validate(&self, now_millis: u64) -> Result<(), BrowserAdmissionFrameError> {
        if !bounded(&self.provider_implementation_id)
            || self.ice_servers.is_empty()
            || self.ice_servers.len() > MAX_WEBRTC_ICE_SERVERS
            || self.expires_at_millis <= now_millis
            || self.expires_at_millis <= self.issued_at_millis
            || self.expires_at_millis - self.issued_at_millis > MAX_WEBRTC_BOOTSTRAP_LIFETIME_MILLIS
        {
            return Err(BrowserAdmissionFrameError::InvalidGrant);
        }
        for server in &self.ice_servers {
            if server.urls.is_empty() || server.urls.len() > MAX_WEBRTC_ICE_URLS_PER_SERVER {
                return Err(BrowserAdmissionFrameError::InvalidGrant);
            }
            let mut turn = false;
            for url in &server.urls {
                if !bounded(url)
                    || !matches!(
                        url.split_once(':').map(|(scheme, _)| scheme),
                        Some("stun" | "stuns" | "turn" | "turns")
                    )
                {
                    return Err(BrowserAdmissionFrameError::InvalidGrant);
                }
                turn |= url.starts_with("turn:") || url.starts_with("turns:");
            }
            let credentials = match (&server.username, &server.credential) {
                (Some(username), Some(credential)) => bounded(username) && bounded(credential),
                (None, None) => true,
                _ => false,
            };
            if !credentials || (turn && server.username.is_none()) {
                return Err(BrowserAdmissionFrameError::InvalidGrant);
            }
        }
        Ok(())
    }
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_WEBRTC_ICE_TEXT_BYTES
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserWebRtcDescription {
    Offer,
    Answer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserWebRtcSignal {
    pub generation: u32,
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
    /// Absent means only local candidate gathering is admitted. Presence is a
    /// finite bootstrap capability, not membership or Line readiness.
    pub bootstrap: Option<WebRtcBootstrapConfiguration>,
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
        if let Some(bootstrap) = &self.bootstrap {
            // Freshness is checked by the consuming endpoint against its
            // current clock. Structural lifetime remains bounded here.
            bootstrap.validate(bootstrap.issued_at_millis)?;
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
        if hello.base != "conduit.base/webrtc-data-channel@1"
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
    InvalidBootstrap,
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
