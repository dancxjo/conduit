//! Shared rendezvous-candidate admission for the native WebRTC realization.

use async_trait::async_trait;
use conduit_body::{RendezvousCandidate, RendezvousLineFamily};
use std::time::Duration;
use tokio::time::timeout;

use crate::browser_admission::WebRtcBootstrapConfiguration;

use super::{NativeWebRtcEndpoint, NativeWebRtcInspection, NativeWebRtcRefusal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWebRtcCandidateRefusal {
    WrongFamily,
    Expired,
    SignalingTimeout,
    Signaling,
    PeerBinding,
    Transport(NativeWebRtcRefusal),
}

impl From<NativeWebRtcRefusal> for NativeWebRtcCandidateRefusal {
    fn from(value: NativeWebRtcRefusal) -> Self {
        Self::Transport(value)
    }
}

pub struct AuthenticatedWebRtcAnswer {
    pub sdp: String,
    pub server_identity: String,
    pub transport_binding_sha256: [u8; 32],
}

#[async_trait]
pub trait NativeWebRtcSignaling {
    async fn exchange_offer(
        &mut self,
        candidate: &RendezvousCandidate,
        offer_sdp: String,
    ) -> Result<AuthenticatedWebRtcAnswer, NativeWebRtcCandidateRefusal>;
}

pub struct NativeWebRtcCandidateConnection {
    pub endpoint: NativeWebRtcEndpoint,
    pub inspection: NativeWebRtcInspection,
}

pub async fn open_native_webrtc_candidate<S: NativeWebRtcSignaling>(
    candidate: &RendezvousCandidate,
    bootstrap: Option<&WebRtcBootstrapConfiguration>,
    now_millis: u64,
    signaling: &mut S,
) -> Result<NativeWebRtcCandidateConnection, NativeWebRtcCandidateRefusal> {
    if candidate.line_family != RendezvousLineFamily::WebRtcDataChannel {
        return Err(NativeWebRtcCandidateRefusal::WrongFamily);
    }
    if candidate.expires_at_millis <= now_millis {
        return Err(NativeWebRtcCandidateRefusal::Expired);
    }
    let attempt_timeout = Duration::from_millis(candidate.attempt_timeout_millis.into());
    let mut offer = NativeWebRtcEndpoint::offer(bootstrap, now_millis, attempt_timeout).await?;
    let answer = timeout(
        attempt_timeout,
        signaling.exchange_offer(candidate, offer.sdp),
    )
    .await
    .map_err(|_| NativeWebRtcCandidateRefusal::SignalingTimeout)??;
    if answer.server_identity != candidate.authentication.server_identity
        || answer.transport_binding_sha256 != candidate.authentication.transport_binding_sha256
    {
        let _ = offer.endpoint.close().await;
        return Err(NativeWebRtcCandidateRefusal::PeerBinding);
    }
    offer.endpoint.accept_answer(answer.sdp).await?;
    offer.endpoint.await_open().await?;
    let inspection = offer.endpoint.inspect().await?;
    Ok(NativeWebRtcCandidateConnection {
        endpoint: offer.endpoint,
        inspection,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::RendezvousAuthentication;

    fn candidate() -> RendezvousCandidate {
        RendezvousCandidate {
            candidate_id: "candidate/webrtc".into(),
            line_family: RendezvousLineFamily::WebRtcDataChannel,
            reachability: "webrtc-bootstrap:test/negotiation-7".into(),
            authentication: RendezvousAuthentication {
                server_identity: "signaling/test/key-7".into(),
                transport_binding_sha256: [0x77; 32],
            },
            expires_at_millis: 10_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 2_000,
        }
    }

    struct DirectSignaling {
        answer: Option<NativeWebRtcEndpoint>,
    }

    #[async_trait]
    impl NativeWebRtcSignaling for DirectSignaling {
        async fn exchange_offer(
            &mut self,
            candidate: &RendezvousCandidate,
            offer_sdp: String,
        ) -> Result<AuthenticatedWebRtcAnswer, NativeWebRtcCandidateRefusal> {
            assert_eq!(
                candidate.reachability,
                "webrtc-bootstrap:test/negotiation-7"
            );
            let answer = NativeWebRtcEndpoint::answer(
                None,
                1_000,
                Duration::from_millis(candidate.attempt_timeout_millis.into()),
                offer_sdp,
            )
            .await?;
            self.answer = Some(answer.endpoint);
            Ok(AuthenticatedWebRtcAnswer {
                sdp: answer.sdp,
                server_identity: candidate.authentication.server_identity.clone(),
                transport_binding_sha256: candidate.authentication.transport_binding_sha256,
            })
        }
    }

    #[tokio::test]
    async fn shared_candidate_opens_existing_native_datachannel_without_retaining_sdp() {
        let mut signaling = DirectSignaling { answer: None };
        let connection = open_native_webrtc_candidate(&candidate(), None, 1_000, &mut signaling)
            .await
            .unwrap();
        let mut answer = signaling.answer.take().unwrap();
        answer.await_open().await.unwrap();
        connection.endpoint.send(b"candidate-owned").await.unwrap();
        let mut received = [0; 32];
        let length = answer.receive(&mut received).await.unwrap();
        assert_eq!(&received[..length], b"candidate-owned");
        assert_eq!(
            connection.inspection.selected_ice_path,
            super::super::NativeWebRtcIcePath::Direct
        );
    }

    #[tokio::test]
    async fn stale_candidate_refuses_before_signaling() {
        let mut signaling = DirectSignaling { answer: None };
        assert!(matches!(
            open_native_webrtc_candidate(&candidate(), None, 10_000, &mut signaling).await,
            Err(NativeWebRtcCandidateRefusal::Expired)
        ));
        assert!(signaling.answer.is_none());
    }
}
