//! Hosted Zenoh implementation of the transport-neutral distributed-cord
//! boundary.
//!
//! Zenoh types stay inside this crate. Exact Conduit bindings, authority,
//! session epochs, envelopes, budgets, and evidence remain authoritative.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};

use conduit_core::{
    DistributedAuthorityContext, DistributedCordHandshake, DistributedEvidenceKind,
    DistributedHandshakeContext, DistributedReason, EventProviderCapabilities, PlanDistributedCord,
    ReconnectMode, ResumeProof, SemanticHash, TerminalClass, validate_distributed_authority_at_use,
    validate_distributed_handshake,
};
use conduit_runtime::{
    CarrierSecurityMode, DistributedBackendReadiness, DistributedCordBackend, DistributedFrameKind,
    HostedDistributedEvidence, OutboundDistributedFrame, ReceivedDistributedFrame,
    ResolvedPlacementBinding, ResolvedTransportSelection, TransportCapabilities, TransportReason,
    decode_distributed_envelope, encode_distributed_envelope, received_evidence_kind,
    validate_transport_selection,
};
use serde_json::json;
use zenoh::Wait;
use zenoh::config::Config;
use zenoh::pubsub::{Publisher, Subscriber};
use zenoh::qos::CongestionControl;
use zenoh::sample::{Locality, Sample};

pub const ZENOH_TRANSPORT_CONTRACT_ID: &str = "conduit/distributed-cord-transport";
pub const ZENOH_HOSTED_IMPLEMENTATION_ID: &str = "conduit/transport.zenoh-rust";
pub const ZENOH_PICO_IMPLEMENTATION_ID: &str = "conduit/transport.zenoh-pico";
pub const FIRMWARE_HOST_SERVICE_ADAPTER_ID: &str = "conduit/embedded-host-service-v1";
pub const FIRMWARE_MESSAGE_ABI_ID: &str = "conduit/ffi-message-v1";
pub const ZENOH_LIVE_EVENT_PROVIDER_CAPABILITIES: EventProviderCapabilities =
    EventProviderCapabilities {
        ephemeral: false,
        retained: false,
        durable: false,
        checkpoint_cursor: false,
        integrity: false,
        redaction: false,
        maximum_events: 0,
        maximum_bytes: 0,
        maximum_subscribers: 0,
        maximum_pending_operations: 0,
    };

/// How the exact endpoint is used by this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZenohEndpointRole {
    Listen,
    Connect,
}

/// Opaque host-side handle for a certificate or private-key file.
///
/// It is never included in plan identity, diagnostics, or evidence.
pub struct SecretFileHandle(PathBuf);

impl SecretFileHandle {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    fn as_utf8(&self) -> Result<&str, ZenohBackendError> {
        self.0.to_str().ok_or(ZenohBackendError::Transport(
            TransportReason::SecretHandleMissing,
        ))
    }
}

/// Host-resolved TLS material. `None` is valid only for plaintext.
#[derive(Default)]
pub struct ZenohTlsMaterial {
    pub root_ca: Option<SecretFileHandle>,
    pub listen_private_key: Option<SecretFileHandle>,
    pub listen_certificate: Option<SecretFileHandle>,
    pub connect_private_key: Option<SecretFileHandle>,
    pub connect_certificate: Option<SecretFileHandle>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZenohBackendError {
    Distributed(DistributedReason),
    Transport(TransportReason),
}

impl ZenohBackendError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Distributed(reason) => reason.code(),
            Self::Transport(reason) => reason.code(),
        }
    }
}

impl From<DistributedReason> for ZenohBackendError {
    fn from(reason: DistributedReason) -> Self {
        Self::Distributed(reason)
    }
}

impl From<TransportReason> for ZenohBackendError {
    fn from(reason: TransportReason) -> Self {
        Self::Transport(reason)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZenohTransportEvidence {
    pub common: HostedDistributedEvidence,
    pub transport_reason: Option<TransportReason>,
}

struct ExpectedSelection {
    binding_identity: SemanticHash,
    backend_id: String,
    backend_schema_version: u32,
    backend_identity: SemanticHash,
    artifact_id: String,
    artifact_digest: conduit_core::ArtifactDigest,
    profile_id: String,
    profile_schema_version: u32,
    profile_identity: SemanticHash,
    endpoint: String,
    carrier_binding: String,
    security_id: String,
    security_schema_version: u32,
    security_identity: SemanticHash,
    security_mode: CarrierSecurityMode,
}

/// One prepared, finite hosted Zenoh session.
///
/// Preparation validates resolver/plan agreement but performs no network I/O.
/// `open` performs the exact listen/connect effect after live authority and
/// realm/passport checks.
pub struct ZenohDistributedCordBackend {
    role: ZenohEndpointRole,
    expected: ExpectedSelection,
    capabilities: TransportCapabilities,
    tls: ZenohTlsMaterial,
    session: Option<zenoh::Session>,
    publisher: Option<Publisher<'static>>,
    subscriber: Option<Subscriber<()>>,
    incoming: Option<Receiver<Sample>>,
    incoming_items: Arc<AtomicUsize>,
    incoming_overflow: Arc<AtomicUsize>,
    evidence: VecDeque<ZenohTransportEvidence>,
    plan_identity: SemanticHash,
    cord: String,
    session_id: String,
    session_epoch: u64,
    maximum_evidence_events: u16,
    maximum_reconnect_attempts: u16,
    reconnect_attempts: u16,
}

impl ZenohDistributedCordBackend {
    pub fn prepare(
        binding: &PlanDistributedCord<'_>,
        placement: &ResolvedPlacementBinding,
        selected: ResolvedTransportSelection<'_>,
        role: ZenohEndpointRole,
        tls: ZenohTlsMaterial,
    ) -> Result<Self, ZenohBackendError> {
        validate_transport_selection(binding, placement, selected)?;
        validate_tls_shape(selected.security_mode, role, &tls)?;
        if !selected.capabilities.publish_subscribe || selected.capabilities.query_reply {
            return Err(TransportReason::ImplementationMismatch.into());
        }
        if selected.capabilities.maximum_frame_bytes > u32::from(u16::MAX)
            || selected.capabilities.adapter_receive_items == 0
            || selected.capabilities.adapter_evidence_items == 0
            || u32::try_from(selected.capabilities.socket_send_bytes).is_err()
            || u32::try_from(selected.capabilities.socket_receive_bytes).is_err()
        {
            return Err(TransportReason::ResourceUnderaccounted.into());
        }
        Ok(Self {
            role,
            expected: ExpectedSelection {
                binding_identity: binding.identity,
                backend_id: selected.backend.id.as_str().to_owned(),
                backend_schema_version: selected.backend.schema_version,
                backend_identity: selected.backend.semantic_hash,
                artifact_id: selected.artifact.id.as_str().to_owned(),
                artifact_digest: selected.artifact.digest,
                profile_id: selected.execution_profile.id.as_str().to_owned(),
                profile_schema_version: selected.execution_profile.schema_version,
                profile_identity: selected.execution_profile.semantic_hash,
                endpoint: selected.endpoint.to_owned(),
                carrier_binding: selected.carrier_binding.as_str().to_owned(),
                security_id: selected.security_descriptor.id.as_str().to_owned(),
                security_schema_version: selected.security_descriptor.schema_version,
                security_identity: selected.security_descriptor.semantic_hash,
                security_mode: selected.security_mode,
            },
            capabilities: selected.capabilities,
            tls,
            session: None,
            publisher: None,
            subscriber: None,
            incoming: None,
            incoming_items: Arc::new(AtomicUsize::new(0)),
            incoming_overflow: Arc::new(AtomicUsize::new(0)),
            evidence: VecDeque::new(),
            plan_identity: SemanticHash::from_bytes([0; 32]),
            cord: String::new(),
            session_id: String::new(),
            session_epoch: 0,
            maximum_evidence_events: selected.capabilities.adapter_evidence_items,
            maximum_reconnect_attempts: 0,
            reconnect_attempts: 0,
        })
    }

    #[must_use]
    pub fn has_remote_subscriber(&self) -> bool {
        self.publisher
            .as_ref()
            .and_then(|publisher| publisher.matching_status().wait().ok())
            .is_some_and(|status| status.matching())
    }

    pub fn shutdown(&mut self) -> Result<(), ZenohBackendError> {
        self.publisher.take();
        self.subscriber.take();
        self.incoming.take();
        if let Some(session) = self.session.take() {
            session
                .close()
                .wait()
                .map_err(|_| TransportReason::CarrierFailure)?;
        }
        Ok(())
    }

    fn validate_exact_binding(
        &self,
        binding: &PlanDistributedCord<'_>,
    ) -> Result<(), ZenohBackendError> {
        let artifact = binding
            .backend_artifact
            .ok_or(TransportReason::ArtifactMismatch)?;
        let profile = binding
            .backend_profile
            .ok_or(TransportReason::ProfileMismatch)?;
        if binding.identity != self.expected.binding_identity
            || binding.backend.id.as_str() != self.expected.backend_id
            || binding.backend.schema_version != self.expected.backend_schema_version
            || binding.backend.semantic_hash != self.expected.backend_identity
            || artifact.id.as_str() != self.expected.artifact_id
            || artifact.digest != self.expected.artifact_digest
            || profile.id.as_str() != self.expected.profile_id
            || profile.schema_version != self.expected.profile_schema_version
            || profile.semantic_hash != self.expected.profile_identity
            || binding.carrier_endpoint != Some(self.expected.endpoint.as_str())
            || binding.carrier_binding.as_str() != self.expected.carrier_binding
            || binding.carrier_security.id.as_str() != self.expected.security_id
            || binding.carrier_security.schema_version != self.expected.security_schema_version
            || binding.carrier_security.semantic_hash != self.expected.security_identity
        {
            return Err(TransportReason::BindingMismatch.into());
        }
        Ok(())
    }

    fn configure(&self) -> Result<Config, ZenohBackendError> {
        let mut config = Config::default();
        insert(&mut config, "mode", json!("peer"))?;
        insert(&mut config, "scouting/multicast/enabled", json!(false))?;
        insert(&mut config, "scouting/gossip/enabled", json!(false))?;
        insert(&mut config, "transport/unicast/open_timeout", json!(2_000))?;
        insert(
            &mut config,
            "transport/unicast/accept_timeout",
            json!(2_000),
        )?;
        insert(
            &mut config,
            "transport/unicast/accept_pending",
            json!(self.capabilities.pending_links),
        )?;
        insert(
            &mut config,
            "transport/unicast/max_sessions",
            json!(self.capabilities.maximum_sessions),
        )?;
        insert(
            &mut config,
            "transport/unicast/max_links",
            json!(self.capabilities.maximum_links),
        )?;
        insert(&mut config, "transport/unicast/lowlatency", json!(true))?;
        insert(&mut config, "transport/unicast/qos/enabled", json!(false))?;
        insert(
            &mut config,
            "transport/link/tx/batch_size",
            json!(self.capabilities.maximum_frame_bytes),
        )?;
        for priority in [
            "control",
            "real_time",
            "interactive_high",
            "interactive_low",
            "data_high",
            "data",
            "data_low",
            "background",
        ] {
            insert(
                &mut config,
                &format!("transport/link/tx/queue/size/{priority}"),
                json!(1),
            )?;
        }
        insert(
            &mut config,
            "transport/link/tx/queue/congestion_control/block/wait_before_close",
            json!(50_000),
        )?;
        insert(
            &mut config,
            "transport/link/tx/queue/batching/enabled",
            json!(false),
        )?;
        insert(
            &mut config,
            "transport/link/tx/queue/allocation/mode",
            json!("init"),
        )?;
        insert(
            &mut config,
            "transport/link/rx/buffer_size",
            json!(self.capabilities.receive_buffer_bytes),
        )?;
        insert(
            &mut config,
            "transport/link/rx/max_message_size",
            json!(self.capabilities.defragmentation_bytes),
        )?;
        let socket_prefix = if self.expected.security_mode == CarrierSecurityMode::Plaintext {
            "transport/link/tcp"
        } else {
            "transport/link/tls"
        };
        insert(
            &mut config,
            &format!("{socket_prefix}/so_sndbuf"),
            json!(self.capabilities.socket_send_bytes),
        )?;
        insert(
            &mut config,
            &format!("{socket_prefix}/so_rcvbuf"),
            json!(self.capabilities.socket_receive_bytes),
        )?;
        match self.role {
            ZenohEndpointRole::Listen => insert(
                &mut config,
                "listen/endpoints",
                json!([self.expected.endpoint]),
            )?,
            ZenohEndpointRole::Connect => insert(
                &mut config,
                "connect/endpoints",
                json!([self.expected.endpoint]),
            )?,
        }
        configure_tls(&mut config, self.expected.security_mode, &self.tls)?;
        Ok(config)
    }

    fn evidence_for(
        &self,
        frame: Option<OutboundDistributedFrame<'_>>,
        kind: DistributedEvidenceKind,
        distributed_reason: Option<DistributedReason>,
        transport_reason: Option<TransportReason>,
    ) -> ZenohTransportEvidence {
        let peer_authenticated = match self.expected.security_mode {
            CarrierSecurityMode::Plaintext => false,
            CarrierSecurityMode::Tls => self.role == ZenohEndpointRole::Connect,
            CarrierSecurityMode::MutualTls => true,
        };
        ZenohTransportEvidence {
            common: HostedDistributedEvidence {
                plan_identity: self.plan_identity,
                binding_identity: self.expected.binding_identity,
                cord: self.cord.clone(),
                session: self.session_id.clone(),
                session_epoch: frame.map_or(self.session_epoch, |frame| frame.session_epoch),
                sequence: frame.and_then(|frame| frame.sequence),
                attempt: frame.and_then(|frame| frame.attempt),
                correlation: frame.and_then(|frame| frame.correlation),
                kind,
                reason: distributed_reason,
                carrier_security: self.expected.security_mode,
                carrier_authenticated: peer_authenticated,
                carrier_mutually_authenticated: self
                    .expected
                    .security_mode
                    .mutually_authenticated(),
                carrier_encrypted: self.expected.security_mode.encrypted(),
                conduit_authority_checked: true,
            },
            transport_reason,
        }
    }

    fn push_evidence(&mut self, evidence: ZenohTransportEvidence) -> Result<(), ZenohBackendError> {
        if self.evidence.len() >= usize::from(self.maximum_evidence_events) {
            return Err(DistributedReason::EvidenceFull.into());
        }
        self.evidence.push_back(evidence);
        Ok(())
    }

    fn reject<T>(
        &mut self,
        frame: Option<OutboundDistributedFrame<'_>>,
        distributed_reason: Option<DistributedReason>,
        transport_reason: Option<TransportReason>,
    ) -> Result<T, ZenohBackendError> {
        self.push_evidence(self.evidence_for(
            frame,
            DistributedEvidenceKind::FrameRejected,
            distributed_reason,
            transport_reason,
        ))?;
        match (distributed_reason, transport_reason) {
            (Some(reason), _) => Err(reason.into()),
            (_, Some(reason)) => Err(reason.into()),
            (None, None) => Err(TransportReason::CarrierFailure.into()),
        }
    }
}

impl DistributedCordBackend for ZenohDistributedCordBackend {
    type Error = ZenohBackendError;
    type Evidence = ZenohTransportEvidence;

    fn capabilities(&self) -> TransportCapabilities {
        self.capabilities
    }

    fn open(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        self.validate_exact_binding(binding)?;
        validate_distributed_handshake(binding, handshake, context)?;
        validate_distributed_authority_at_use(binding, authority)?;
        if self.session.is_some() || handshake.session_epoch != binding.initial_session_epoch {
            return Err(DistributedReason::HandshakeMismatch.into());
        }
        self.plan_identity = handshake.plan_identity;
        self.cord = binding.cord.as_str().to_owned();
        self.session_id = binding.session.as_str().to_owned();
        self.session_epoch = handshake.session_epoch;
        self.maximum_evidence_events = binding.budget.maximum_evidence_events;
        self.maximum_reconnect_attempts = binding.budget.maximum_reconnect_attempts;
        let session = zenoh::open(self.configure()?)
            .wait()
            .map_err(|_| TransportReason::CarrierFailure)?;
        let key = zenoh::key_expr::KeyExpr::new(self.expected.carrier_binding.clone())
            .map_err(|_| TransportReason::BindingMismatch)?
            .into_owned();
        let (sender, receiver) = sync_channel(usize::from(binding.budget.receive_items));
        let incoming_items = Arc::clone(&self.incoming_items);
        let overflow = Arc::clone(&self.incoming_overflow);
        let subscriber = session
            .declare_subscriber(key.clone())
            .allowed_origin(Locality::Remote)
            .callback(move |sample| {
                nonblocking_receive(&sender, &incoming_items, &overflow, sample)
            })
            .wait()
            .map_err(|_| TransportReason::CarrierFailure)?;
        let publisher = session
            .declare_publisher(key)
            .allowed_destination(Locality::Remote)
            .congestion_control(CongestionControl::Block)
            .wait()
            .map_err(|_| TransportReason::CarrierFailure)?;
        self.incoming = Some(receiver);
        self.subscriber = Some(subscriber);
        self.publisher = Some(publisher);
        self.session = Some(session);
        self.push_evidence(self.evidence_for(
            None,
            DistributedEvidenceKind::HandshakeAccepted,
            None,
            None,
        ))
    }

    fn reauthenticate(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        handshake: DistributedCordHandshake<'_>,
        context: DistributedHandshakeContext<'_>,
        resume: Option<ResumeProof>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        self.validate_exact_binding(binding)?;
        validate_distributed_handshake(binding, handshake, context)?;
        validate_distributed_authority_at_use(binding, authority)?;
        self.reconnect_attempts = self
            .reconnect_attempts
            .checked_add(1)
            .ok_or(DistributedReason::ReconnectDenied)?;
        if self.reconnect_attempts > self.maximum_reconnect_attempts {
            return Err(DistributedReason::ReconnectDenied.into());
        }
        match binding.reconnect {
            ReconnectMode::Reject => return Err(DistributedReason::ReconnectDenied.into()),
            ReconnectMode::ResumeSameEpoch => {
                let proof = resume.ok_or(DistributedReason::ReconnectDenied)?;
                if handshake.session_epoch != self.session_epoch
                    || proof.plan_identity != self.plan_identity
                    || proof.binding_identity != self.expected.binding_identity
                    || proof.session_epoch != self.session_epoch
                    || proof.receipt == SemanticHash::from_bytes([0; 32])
                {
                    return Err(DistributedReason::EpochMismatch.into());
                }
            }
            ReconnectMode::BeginNewEpoch => {
                if resume.is_some()
                    || self.session_epoch.checked_add(1) != Some(handshake.session_epoch)
                {
                    return Err(DistributedReason::EpochMismatch.into());
                }
            }
        }
        self.session_epoch = handshake.session_epoch;
        self.push_evidence(self.evidence_for(
            None,
            DistributedEvidenceKind::Reconnected,
            None,
            None,
        ))
    }

    fn send_readiness(&self) -> DistributedBackendReadiness {
        if self.publisher.is_none() {
            DistributedBackendReadiness::Closed
        } else {
            DistributedBackendReadiness::Ready
        }
    }

    fn send(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        frame: OutboundDistributedFrame<'_>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        if let Err(reason) = self.validate_exact_binding(binding) {
            return self.reject(Some(frame), None, transport_from_error(reason));
        }
        if let Err(reason) = validate_distributed_authority_at_use(binding, authority) {
            return self.reject(Some(frame), Some(reason), None);
        }
        if frame.session_epoch != self.session_epoch {
            return self.reject(Some(frame), Some(DistributedReason::EpochMismatch), None);
        }
        let mut bytes = vec![0_u8; binding.budget.maximum_frame_bytes as usize];
        let used = match encode_distributed_envelope(self.plan_identity, binding, frame, &mut bytes)
        {
            Ok(used) => used,
            Err(reason) => return self.reject(Some(frame), None, Some(reason)),
        };
        bytes.truncate(used);
        let Some(publisher) = self.publisher.as_ref() else {
            return self.reject(Some(frame), None, Some(TransportReason::Disconnected));
        };
        if publisher.put(bytes).wait().is_err() {
            return self.reject(Some(frame), None, Some(TransportReason::CarrierFailure));
        }
        let kind = match frame.kind {
            DistributedFrameKind::Value if frame.attempt.is_some_and(|attempt| attempt > 0) => {
                DistributedEvidenceKind::Retried
            }
            DistributedFrameKind::Value => DistributedEvidenceKind::ValueSent,
            DistributedFrameKind::Acknowledgement => DistributedEvidenceKind::Acknowledged,
            DistributedFrameKind::Cancellation
            | DistributedFrameKind::CancellationAcknowledgement => {
                DistributedEvidenceKind::Cancelled
            }
            DistributedFrameKind::Terminal(_) | DistributedFrameKind::TerminalAcknowledgement => {
                DistributedEvidenceKind::Terminal
            }
            DistributedFrameKind::Heartbeat => DistributedEvidenceKind::Heartbeat,
        };
        self.push_evidence(self.evidence_for(Some(frame), kind, None, None))
    }

    fn receive_readiness(&self) -> DistributedBackendReadiness {
        if self.incoming.is_none() {
            DistributedBackendReadiness::Closed
        } else if self.incoming_overflow.load(Ordering::Acquire) > 0
            || self.incoming_items.load(Ordering::Acquire) > 0
        {
            DistributedBackendReadiness::Ready
        } else {
            DistributedBackendReadiness::Pending
        }
    }

    fn receive(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        destination: &mut [u8],
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<Option<ReceivedDistributedFrame>, Self::Error> {
        self.validate_exact_binding(binding)?;
        if let Err(reason) = validate_distributed_authority_at_use(binding, authority) {
            return self.reject(None, Some(reason), None);
        }
        if self.incoming_overflow.swap(0, Ordering::AcqRel) > 0 {
            return self.reject(
                None,
                Some(DistributedReason::BufferFull),
                Some(TransportReason::QueueFull),
            );
        }
        let sample = match self.incoming.as_ref().map(Receiver::try_recv) {
            Some(Ok(sample)) => {
                self.incoming_items.fetch_sub(1, Ordering::AcqRel);
                sample
            }
            Some(Err(TryRecvError::Empty)) => return Ok(None),
            Some(Err(TryRecvError::Disconnected)) | None => {
                return self.reject(None, None, Some(TransportReason::Disconnected));
            }
        };
        let bytes = sample.payload().to_bytes();
        let decoded = match decode_distributed_envelope(&bytes, self.plan_identity, binding) {
            Ok(decoded) => decoded,
            Err(reason) => return self.reject(None, None, Some(reason)),
        };
        if destination.len() < decoded.frame.payload.len() {
            return self.reject(
                Some(decoded.frame),
                Some(DistributedReason::BufferFull),
                None,
            );
        }
        destination[..decoded.frame.payload.len()].copy_from_slice(decoded.frame.payload);
        self.push_evidence(self.evidence_for(
            Some(decoded.frame),
            received_evidence_kind(decoded.frame.kind),
            None,
            None,
        ))?;
        Ok(Some(ReceivedDistributedFrame {
            kind: decoded.frame.kind,
            session_epoch: decoded.frame.session_epoch,
            sequence: decoded.frame.sequence,
            attempt: decoded.frame.attempt,
            correlation: decoded.frame.correlation,
            payload_bytes: decoded.frame.payload.len(),
        }))
    }

    fn close(
        &mut self,
        binding: &PlanDistributedCord<'_>,
        session_epoch: u64,
        sequence: u64,
        terminal: TerminalClass,
        correlation: Option<SemanticHash>,
        authority: DistributedAuthorityContext<'_>,
    ) -> Result<(), Self::Error> {
        self.send(
            binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::Terminal(terminal),
                session_epoch,
                sequence: Some(sequence),
                attempt: None,
                correlation,
                payload: &[],
            },
            authority,
        )
    }

    fn take_evidence(&mut self) -> Option<Self::Evidence> {
        self.evidence.pop_front()
    }
}

fn nonblocking_receive(
    sender: &SyncSender<Sample>,
    incoming_items: &AtomicUsize,
    overflow: &AtomicUsize,
    sample: Sample,
) {
    // Reserve the observation before publishing the sample into the channel.
    // A receiver may run as soon as `try_send` makes the sample visible.
    incoming_items.fetch_add(1, Ordering::AcqRel);
    match sender.try_send(sample) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            incoming_items.fetch_sub(1, Ordering::AcqRel);
            overflow.store(1, Ordering::Release);
        }
        Err(TrySendError::Disconnected(_)) => {
            incoming_items.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

fn validate_tls_shape(
    mode: CarrierSecurityMode,
    role: ZenohEndpointRole,
    tls: &ZenohTlsMaterial,
) -> Result<(), ZenohBackendError> {
    let valid = match (mode, role) {
        (CarrierSecurityMode::Plaintext, _) => {
            tls.root_ca.is_none()
                && tls.listen_private_key.is_none()
                && tls.listen_certificate.is_none()
                && tls.connect_private_key.is_none()
                && tls.connect_certificate.is_none()
        }
        (CarrierSecurityMode::Tls, ZenohEndpointRole::Listen) => {
            tls.listen_private_key.is_some() && tls.listen_certificate.is_some()
        }
        (CarrierSecurityMode::Tls, ZenohEndpointRole::Connect) => tls.root_ca.is_some(),
        (CarrierSecurityMode::MutualTls, ZenohEndpointRole::Listen) => {
            tls.root_ca.is_some()
                && tls.listen_private_key.is_some()
                && tls.listen_certificate.is_some()
        }
        (CarrierSecurityMode::MutualTls, ZenohEndpointRole::Connect) => {
            tls.root_ca.is_some()
                && tls.connect_private_key.is_some()
                && tls.connect_certificate.is_some()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(TransportReason::SecretHandleMissing.into())
    }
}

fn configure_tls(
    config: &mut Config,
    mode: CarrierSecurityMode,
    tls: &ZenohTlsMaterial,
) -> Result<(), ZenohBackendError> {
    if mode == CarrierSecurityMode::Plaintext {
        return Ok(());
    }
    insert(
        config,
        "transport/link/tls/enable_mtls",
        json!(mode == CarrierSecurityMode::MutualTls),
    )?;
    if let Some(value) = &tls.root_ca {
        insert(
            config,
            "transport/link/tls/root_ca_certificate",
            json!(value.as_utf8()?),
        )?;
    }
    if let Some(value) = &tls.listen_private_key {
        insert(
            config,
            "transport/link/tls/listen_private_key",
            json!(value.as_utf8()?),
        )?;
    }
    if let Some(value) = &tls.listen_certificate {
        insert(
            config,
            "transport/link/tls/listen_certificate",
            json!(value.as_utf8()?),
        )?;
    }
    if let Some(value) = &tls.connect_private_key {
        insert(
            config,
            "transport/link/tls/connect_private_key",
            json!(value.as_utf8()?),
        )?;
    }
    if let Some(value) = &tls.connect_certificate {
        insert(
            config,
            "transport/link/tls/connect_certificate",
            json!(value.as_utf8()?),
        )?;
    }
    Ok(())
}

fn insert(
    config: &mut Config,
    key: &str,
    value: serde_json::Value,
) -> Result<(), ZenohBackendError> {
    config
        .insert_json5(key, &value.to_string())
        .map_err(|_| TransportReason::CarrierFailure.into())
}

fn transport_from_error(error: ZenohBackendError) -> Option<TransportReason> {
    match error {
        ZenohBackendError::Transport(reason) => Some(reason),
        ZenohBackendError::Distributed(_) => None,
    }
}
