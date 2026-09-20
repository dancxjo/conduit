//! Native std realization of the existing bounded WebRTC DataChannel Base.
//!
//! Signaling and ICE configuration are supplied by the admitted rendezvous
//! layer. This adapter owns transport lifecycle only; it grants no membership,
//! Plan, Cord, or effect authority.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::BytesMut;
use tokio::{sync::mpsc, time::timeout};
use webrtc::{
    data_channel::{DataChannel, DataChannelEvent},
    peer_connection::{
        PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder,
        RTCIceGatheringState, RTCIceServer, RTCIceTransportPolicy, RTCSessionDescription,
        RTCStatsReportEntry, StatsSelector,
    },
};

use crate::browser_admission::{
    WebRtcBootstrapConfiguration, WebRtcIceTransportPolicy, MAX_WEBRTC_DESCRIPTION_BYTES,
};

#[path = "native_webrtc/session.rs"]
mod session;
pub use session::{NativeWebRtcSession, NativeWebRtcSessionRefusal};

pub const NATIVE_WEBRTC_IMPLEMENTATION_ID: &str = "std/webrtc-datachannel@1";
pub const MAXIMUM_NATIVE_WEBRTC_FRAME_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWebRtcIcePath {
    Direct,
    Relayed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeWebRtcInspection {
    pub implementation_id: &'static str,
    pub bootstrap_provider_implementation_id: Option<String>,
    pub transport_policy: WebRtcIceTransportPolicy,
    pub selected_ice_path: NativeWebRtcIcePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWebRtcRefusal {
    Bootstrap,
    Description,
    GatheringTimeout,
    DataChannelTimeout,
    DataChannelLost,
    Pressure,
    Transport,
}

pub struct NativeWebRtcOffer {
    pub endpoint: NativeWebRtcEndpoint,
    pub sdp: String,
}

pub struct NativeWebRtcAnswer {
    pub endpoint: NativeWebRtcEndpoint,
    pub sdp: String,
}

#[derive(Clone)]
struct Handler {
    gathered: mpsc::Sender<()>,
    channels: mpsc::Sender<Arc<dyn DataChannel>>,
}

#[async_trait]
impl PeerConnectionEventHandler for Handler {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            let _ = self.gathered.try_send(());
        }
    }

    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        let _ = self.channels.try_send(channel);
    }
}

pub struct NativeWebRtcEndpoint {
    peer: Arc<dyn PeerConnection>,
    channel: Option<Arc<dyn DataChannel>>,
    gathered: mpsc::Receiver<()>,
    channels: mpsc::Receiver<Arc<dyn DataChannel>>,
    maximum_frame_bytes: usize,
    operation_timeout: Duration,
    bootstrap_provider_implementation_id: Option<String>,
    transport_policy: WebRtcIceTransportPolicy,
}

impl NativeWebRtcEndpoint {
    pub const fn maximum_frame_bytes(&self) -> usize {
        self.maximum_frame_bytes
    }
    pub async fn offer(
        bootstrap: Option<&WebRtcBootstrapConfiguration>,
        now_millis: u64,
        operation_timeout: Duration,
    ) -> Result<NativeWebRtcOffer, NativeWebRtcRefusal> {
        let mut endpoint = Self::build(bootstrap, now_millis, operation_timeout).await?;
        let channel = endpoint
            .peer
            .create_data_channel("conduit-line", None)
            .await
            .map_err(|_| NativeWebRtcRefusal::Transport)?;
        endpoint.channel = Some(channel);
        let offer = endpoint
            .peer
            .create_offer(None)
            .await
            .map_err(|_| NativeWebRtcRefusal::Transport)?;
        endpoint
            .peer
            .set_local_description(offer)
            .await
            .map_err(|_| NativeWebRtcRefusal::Transport)?;
        let sdp = endpoint.finish_gathering().await?;
        Ok(NativeWebRtcOffer { endpoint, sdp })
    }

    pub async fn answer(
        bootstrap: Option<&WebRtcBootstrapConfiguration>,
        now_millis: u64,
        operation_timeout: Duration,
        remote_offer_sdp: String,
    ) -> Result<NativeWebRtcAnswer, NativeWebRtcRefusal> {
        let mut endpoint = Self::build(bootstrap, now_millis, operation_timeout).await?;
        let offer = RTCSessionDescription::offer(remote_offer_sdp)
            .map_err(|_| NativeWebRtcRefusal::Description)?;
        endpoint
            .peer
            .set_remote_description(offer)
            .await
            .map_err(|_| NativeWebRtcRefusal::Description)?;
        let answer = endpoint
            .peer
            .create_answer(None)
            .await
            .map_err(|_| NativeWebRtcRefusal::Transport)?;
        endpoint
            .peer
            .set_local_description(answer)
            .await
            .map_err(|_| NativeWebRtcRefusal::Transport)?;
        let sdp = endpoint.finish_gathering().await?;
        Ok(NativeWebRtcAnswer { endpoint, sdp })
    }

    pub async fn accept_answer(
        &self,
        remote_answer_sdp: String,
    ) -> Result<(), NativeWebRtcRefusal> {
        if remote_answer_sdp.len() > MAX_WEBRTC_DESCRIPTION_BYTES {
            return Err(NativeWebRtcRefusal::Description);
        }
        let answer = RTCSessionDescription::answer(remote_answer_sdp)
            .map_err(|_| NativeWebRtcRefusal::Description)?;
        self.peer
            .set_remote_description(answer)
            .await
            .map_err(|_| NativeWebRtcRefusal::Description)
    }

    pub async fn await_open(&mut self) -> Result<(), NativeWebRtcRefusal> {
        if self.channel.is_none() {
            self.channel = Some(
                timeout(self.operation_timeout, self.channels.recv())
                    .await
                    .map_err(|_| NativeWebRtcRefusal::DataChannelTimeout)?
                    .ok_or(NativeWebRtcRefusal::DataChannelLost)?,
            );
        }
        let channel = self.channel.as_ref().expect("installed above");
        loop {
            match timeout(self.operation_timeout, channel.poll()).await {
                Err(_) => return Err(NativeWebRtcRefusal::DataChannelTimeout),
                Ok(Some(DataChannelEvent::OnOpen)) => return Ok(()),
                Ok(Some(DataChannelEvent::OnError | DataChannelEvent::OnClose)) | Ok(None) => {
                    return Err(NativeWebRtcRefusal::DataChannelLost);
                }
                Ok(Some(_)) => {}
            }
        }
    }

    pub async fn send(&self, frame: &[u8]) -> Result<(), NativeWebRtcRefusal> {
        if frame.is_empty() || frame.len() > self.maximum_frame_bytes {
            return Err(NativeWebRtcRefusal::Pressure);
        }
        self.channel
            .as_ref()
            .ok_or(NativeWebRtcRefusal::DataChannelLost)?
            .send(BytesMut::from(frame))
            .await
            .map_err(|_| NativeWebRtcRefusal::DataChannelLost)
    }

    pub async fn receive(&self, output: &mut [u8]) -> Result<usize, NativeWebRtcRefusal> {
        if output.is_empty() || output.len() > self.maximum_frame_bytes {
            return Err(NativeWebRtcRefusal::Pressure);
        }
        let channel = self
            .channel
            .as_ref()
            .ok_or(NativeWebRtcRefusal::DataChannelLost)?;
        loop {
            match timeout(self.operation_timeout, channel.poll()).await {
                Err(_) => return Err(NativeWebRtcRefusal::DataChannelTimeout),
                Ok(Some(DataChannelEvent::OnMessage(message))) => {
                    if message.data.len() > output.len() {
                        return Err(NativeWebRtcRefusal::Pressure);
                    }
                    output[..message.data.len()].copy_from_slice(&message.data);
                    return Ok(message.data.len());
                }
                Ok(Some(DataChannelEvent::OnError | DataChannelEvent::OnClose)) | Ok(None) => {
                    return Err(NativeWebRtcRefusal::DataChannelLost);
                }
                Ok(Some(_)) => {}
            }
        }
    }

    pub async fn close(&self) -> Result<(), NativeWebRtcRefusal> {
        timeout(self.operation_timeout, self.peer.close())
            .await
            .map_err(|_| NativeWebRtcRefusal::DataChannelTimeout)?
            .map_err(|_| NativeWebRtcRefusal::Transport)
    }

    /// Returns bounded provenance for the selected path without retaining or
    /// exposing candidate addresses, ports, credentials, or SDP.
    pub async fn inspect(&self) -> Result<NativeWebRtcInspection, NativeWebRtcRefusal> {
        let report = self
            .peer
            .get_stats(Instant::now(), StatsSelector::None)
            .await;
        let selected_pair_id = &report
            .transport()
            .ok_or(NativeWebRtcRefusal::Transport)?
            .selected_candidate_pair_id;
        let pair = match report.get(selected_pair_id) {
            Some(RTCStatsReportEntry::IceCandidatePair(pair)) => pair,
            _ => return Err(NativeWebRtcRefusal::Transport),
        };
        // rtc's pair uses the raw candidate identity while its stats report
        // namespaces the same identity by local/remote kind. Inspect the
        // selected local carrier: a relay-only allocation is necessarily the
        // endpoint's TURN path, while host/srflx/prflx are direct ICE paths.
        let relayed = report
            .iter()
            .find_map(|entry| match entry {
                RTCStatsReportEntry::LocalCandidate(candidate)
                    if candidate.stats.id.ends_with(&pair.local_candidate_id) =>
                {
                    Some(candidate.candidate_type.to_string() == "relay")
                }
                _ => None,
            })
            .ok_or(NativeWebRtcRefusal::Transport)?;
        Ok(NativeWebRtcInspection {
            implementation_id: NATIVE_WEBRTC_IMPLEMENTATION_ID,
            bootstrap_provider_implementation_id: self.bootstrap_provider_implementation_id.clone(),
            transport_policy: self.transport_policy,
            selected_ice_path: if relayed {
                NativeWebRtcIcePath::Relayed
            } else {
                NativeWebRtcIcePath::Direct
            },
        })
    }

    async fn build(
        bootstrap: Option<&WebRtcBootstrapConfiguration>,
        now_millis: u64,
        operation_timeout: Duration,
    ) -> Result<Self, NativeWebRtcRefusal> {
        if let Some(bootstrap) = bootstrap {
            bootstrap
                .validate(now_millis)
                .map_err(|_| NativeWebRtcRefusal::Bootstrap)?;
        }
        if operation_timeout.is_zero() {
            return Err(NativeWebRtcRefusal::Bootstrap);
        }
        let transport_policy = bootstrap
            .map(|configuration| configuration.transport_policy)
            .unwrap_or(WebRtcIceTransportPolicy::DirectAndRelay);
        let bootstrap_provider_implementation_id =
            bootstrap.map(|configuration| configuration.provider_implementation_id.clone());
        let ice_servers = bootstrap
            .map(|configuration| {
                configuration
                    .ice_servers
                    .iter()
                    .map(|server| RTCIceServer {
                        urls: server.urls.clone(),
                        username: server.username.clone().unwrap_or_default(),
                        credential: server.credential.clone().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut configuration = RTCConfigurationBuilder::new().with_ice_servers(ice_servers);
        if bootstrap.is_some_and(|configuration| {
            configuration.transport_policy == WebRtcIceTransportPolicy::RelayOnly
        }) {
            configuration = configuration.with_ice_transport_policy(RTCIceTransportPolicy::Relay);
        }
        let (gathered_tx, gathered) = mpsc::channel(1);
        let (channels_tx, channels) = mpsc::channel(1);
        let peer: Arc<dyn PeerConnection> = Arc::new(
            PeerConnectionBuilder::new()
                .with_configuration(configuration.build())
                .with_handler(Arc::new(Handler {
                    gathered: gathered_tx,
                    channels: channels_tx,
                }))
                .with_udp_addrs(vec!["0.0.0.0:0"])
                .with_data_channel_send_buffer_limit(MAXIMUM_NATIVE_WEBRTC_FRAME_BYTES)
                .build()
                .await
                .map_err(|_| NativeWebRtcRefusal::Transport)?,
        );
        Ok(Self {
            peer,
            channel: None,
            gathered,
            channels,
            maximum_frame_bytes: MAXIMUM_NATIVE_WEBRTC_FRAME_BYTES,
            operation_timeout,
            bootstrap_provider_implementation_id,
            transport_policy,
        })
    }

    async fn finish_gathering(&mut self) -> Result<String, NativeWebRtcRefusal> {
        timeout(self.operation_timeout, self.gathered.recv())
            .await
            .map_err(|_| NativeWebRtcRefusal::GatheringTimeout)?
            .ok_or(NativeWebRtcRefusal::Transport)?;
        let description = self
            .peer
            .local_description()
            .await
            .ok_or(NativeWebRtcRefusal::Description)?;
        if description.sdp.is_empty() || description.sdp.len() > MAX_WEBRTC_DESCRIPTION_BYTES {
            return Err(NativeWebRtcRefusal::Description);
        }
        Ok(description.sdp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        bind_active_play, BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId,
        HostId, KindId, LineContract, LineDuplex, LineId, LineOrdering, LineReliability, LineScope,
        LineSecurity, LineTrafficShape, LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
        PROTOCOL_VERSION,
    };
    use conduit_wire::{
        LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits, SessionRole,
    };
    use std::{
        collections::HashMap,
        net::{IpAddr, SocketAddr},
    };
    use tokio::net::UdpSocket;
    use turn::{
        auth::{generate_auth_key, AuthHandler},
        relay::relay_static::RelayAddressGeneratorStatic,
        server::{
            config::{ConnConfig, ServerConfig},
            Server,
        },
    };
    use webrtc_util::vnet::net::Net;

    struct ProofTurnAuth {
        keys: HashMap<String, Vec<u8>>,
    }

    impl AuthHandler for ProofTurnAuth {
        fn auth_handle(
            &self,
            username: &str,
            _realm: &str,
            _source: SocketAddr,
        ) -> Result<Vec<u8>, turn::Error> {
            self.keys
                .get(username)
                .cloned()
                .ok_or(turn::Error::ErrFakeErr)
        }
    }

    async fn proof_turn_server() -> (Server, u16) {
        let listener = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let port = listener.local_addr().unwrap().port();
        let username = "conduit-proof";
        let realm = "conduit.invalid";
        let password = "short-lived-proof-secret";
        let keys = HashMap::from([(
            username.to_owned(),
            generate_auth_key(username, realm, password),
        )]);
        let server = Server::new(ServerConfig {
            conn_configs: vec![ConnConfig {
                conn: listener,
                relay_addr_generator: Box::new(RelayAddressGeneratorStatic {
                    relay_address: IpAddr::from([127, 0, 0, 1]),
                    address: "127.0.0.1".to_owned(),
                    net: Arc::new(Net::new(None)),
                }),
            }],
            realm: realm.to_owned(),
            auth_handler: Arc::new(ProofTurnAuth { keys }),
            channel_bind_timeout: Duration::ZERO,
            alloc_close_notify: None,
        })
        .await
        .unwrap();
        (server, port)
    }

    fn relay_bootstrap(port: u16) -> WebRtcBootstrapConfiguration {
        WebRtcBootstrapConfiguration {
            provider_implementation_id: "self-hosted/turn-proof@1".to_owned(),
            issued_at_millis: 1_000,
            expires_at_millis: 61_000,
            transport_policy: WebRtcIceTransportPolicy::RelayOnly,
            ice_servers: vec![crate::browser_admission::WebRtcIceServer {
                urls: vec![format!("turn:127.0.0.1:{port}?transport=udp")],
                username: Some("conduit-proof".to_owned()),
                credential: Some("short-lived-proof-secret".to_owned()),
            }],
        }
    }

    fn binding() -> SessionBinding {
        let plan_id = PlanId::from("plan/native-webrtc-proof");
        let source_host = HostId::from("host/native-webrtc-source");
        let source_boot = BootId::from("boot/native-webrtc-source/1");
        let sink_host = HostId::from("host/native-webrtc-sink");
        let sink_boot = BootId::from("boot/native-webrtc-sink/1");
        SessionBinding {
            protocol_version: PROTOCOL_VERSION,
            plan_id: plan_id.clone(),
            source_fragment_id: FragmentId::from("fragment/native-webrtc-source"),
            sink_fragment_id: FragmentId::from("fragment/native-webrtc-sink"),
            source_active_play_id: bind_active_play(&plan_id, &source_host, &source_boot, 0)
                .active_play_id,
            sink_active_play_id: bind_active_play(&plan_id, &sink_host, &sink_boot, 0)
                .active_play_id,
            connection_id: ConnectionId::from("connection/native-webrtc-proof"),
            source: SessionEndpointIdentity {
                host_id: source_host.clone(),
                boot_id: source_boot.clone(),
            },
            sink: SessionEndpointIdentity {
                host_id: sink_host.clone(),
                boot_id: sink_boot.clone(),
            },
            value_kind: KindId::from("value/native-webrtc-proof@1"),
            limits: SessionLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 256,
                maximum_buffered_bytes: 256,
            },
            attachment: LineAttachment {
                line_id: LineId::from("line/native-webrtc-proof"),
                link_binding_id: LinkBindingId::from("binding/native-webrtc-proof"),
                base: BaseImplementationId::from("conduit.base/webrtc-data-channel@1"),
                contract: LineContract {
                    scope: LineScope::PointToPoint,
                    traffic_shape: LineTrafficShape::Message,
                    duplex: LineDuplex::FullDuplex,
                    ordering: LineOrdering::Ordered,
                    reliability: LineReliability::Reliable,
                    continuation: conduit_core::LineContinuation::None,
                    security: LineSecurity::AuthenticatedEncrypted,
                },
                base_instance_id: BaseInstanceId::from("base/native-webrtc-proof"),
                source_host_id: source_host,
                source_boot_id: source_boot,
                source_endpoint_id: LinkEndpointId::from("endpoint/native-webrtc-source"),
                sink_host_id: sink_host,
                sink_boot_id: sink_boot,
                sink_endpoint_id: LinkEndpointId::from("endpoint/native-webrtc-sink"),
                limits: LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: 256,
                    maximum_buffered_bytes: 256,
                    maximum_frame_bytes: 1_024,
                },
            },
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_peers_open_one_bounded_direct_data_channel() {
        let timeout = Duration::from_secs(10);
        let mut offer = NativeWebRtcEndpoint::offer(None, 0, timeout).await.unwrap();
        let mut answer = NativeWebRtcEndpoint::answer(None, 0, timeout, offer.sdp)
            .await
            .unwrap();
        offer.endpoint.accept_answer(answer.sdp).await.unwrap();
        let (offer_open, answer_open) =
            tokio::join!(offer.endpoint.await_open(), answer.endpoint.await_open());
        offer_open.unwrap();
        answer_open.unwrap();

        let inspection = offer.endpoint.inspect().await.unwrap();
        assert_eq!(
            inspection.implementation_id,
            NATIVE_WEBRTC_IMPLEMENTATION_ID
        );
        assert_eq!(inspection.bootstrap_provider_implementation_id, None);
        assert_eq!(
            inspection.transport_policy,
            WebRtcIceTransportPolicy::DirectAndRelay
        );
        assert_eq!(inspection.selected_ice_path, NativeWebRtcIcePath::Direct);

        offer.endpoint.send(b"ordinary Cord value").await.unwrap();
        let mut output = [0_u8; 64];
        let length = answer.endpoint.receive(&mut output).await.unwrap();
        assert_eq!(&output[..length], b"ordinary Cord value");
        assert_eq!(
            offer
                .endpoint
                .send(&[0; MAXIMUM_NATIVE_WEBRTC_FRAME_BYTES + 1])
                .await,
            Err(NativeWebRtcRefusal::Pressure)
        );
        offer.endpoint.close().await.unwrap();
        answer.endpoint.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_peers_use_an_authenticated_self_hosted_relay_only_path() {
        let (server, port) = proof_turn_server().await;
        let bootstrap = relay_bootstrap(port);
        let timeout = Duration::from_secs(10);
        assert!(matches!(
            NativeWebRtcEndpoint::offer(Some(&bootstrap), bootstrap.expires_at_millis, timeout)
                .await,
            Err(NativeWebRtcRefusal::Bootstrap)
        ));
        let mut offer = NativeWebRtcEndpoint::offer(Some(&bootstrap), 2_000, timeout)
            .await
            .unwrap();
        let mut answer = NativeWebRtcEndpoint::answer(Some(&bootstrap), 2_000, timeout, offer.sdp)
            .await
            .unwrap();
        offer.endpoint.accept_answer(answer.sdp).await.unwrap();
        let (offer_open, answer_open) =
            tokio::join!(offer.endpoint.await_open(), answer.endpoint.await_open());
        offer_open.unwrap();
        answer_open.unwrap();

        for endpoint in [&offer.endpoint, &answer.endpoint] {
            let inspection = endpoint.inspect().await.unwrap();
            assert_eq!(inspection.selected_ice_path, NativeWebRtcIcePath::Relayed);
            assert_eq!(
                inspection.bootstrap_provider_implementation_id.as_deref(),
                Some("self-hosted/turn-proof@1")
            );
            assert_eq!(
                inspection.transport_policy,
                WebRtcIceTransportPolicy::RelayOnly
            );
        }
        offer.endpoint.send(b"relayed Cord value").await.unwrap();
        let mut output = [0_u8; 64];
        let length = answer.endpoint.receive(&mut output).await.unwrap();
        assert_eq!(&output[..length], b"relayed Cord value");
        offer.endpoint.close().await.unwrap();
        answer.endpoint.close().await.unwrap();
        server.close().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_datachannel_carries_the_exact_planned_session_contract() {
        let timeout = Duration::from_secs(10);
        let mut offer = NativeWebRtcEndpoint::offer(None, 0, timeout).await.unwrap();
        let mut answer = NativeWebRtcEndpoint::answer(None, 0, timeout, offer.sdp)
            .await
            .unwrap();
        offer.endpoint.accept_answer(answer.sdp).await.unwrap();
        let (offer_open, answer_open) =
            tokio::join!(offer.endpoint.await_open(), answer.endpoint.await_open());
        offer_open.unwrap();
        answer_open.unwrap();
        let exact_binding = binding();
        let mut source =
            NativeWebRtcSession::new(offer.endpoint, exact_binding.clone(), SessionRole::Source)
                .unwrap();
        let mut sink =
            NativeWebRtcSession::new(answer.endpoint, exact_binding, SessionRole::Sink).unwrap();
        let (source_ready, sink_ready) = tokio::join!(source.handshake(), sink.handshake());
        source_ready.unwrap();
        sink_ready.unwrap();
        let mut received = [0_u8; 64];
        let (offered, delivered) = tokio::join!(
            source.offer_and_wait_delivery(b"planned Cord value"),
            sink.receive_and_deliver(&mut received)
        );
        assert_eq!(offered.unwrap(), 0);
        let (sequence, length) = delivered.unwrap();
        assert_eq!(sequence, 0);
        assert_eq!(&received[..length], b"planned Cord value");
        let (source_finished, sink_finished) = tokio::join!(source.finish(), sink.finish());
        source_finished.unwrap();
        sink_finished.unwrap();
    }
}
