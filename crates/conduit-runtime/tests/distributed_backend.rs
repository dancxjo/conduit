use conduit_core::{
    AcknowledgementMode, ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime,
    CarrierSecurityMode, CredentialVerification, CredentialVerificationOutcome, DelegationPolicy,
    DisconnectPolicy, DistributedAuthorityContext, DistributedCordBudget, DistributedCordHandshake,
    DistributedDelivery, DistributedEvidenceKind, DistributedHandshakeContext, DistributedOrdering,
    DistributedPeerProof, DistributedPeerRequirement, DistributedReason, EffectRequirement,
    FlowCapacity, FlowPolicy, FlowWatermarks, GrantStatus, HostCapability, Id, ObservedGrant,
    PassportStatus, PassportStatusObservation, PinnedDescriptor, PlanArtifact, PlanAuthority,
    PlanDistributedCord, PlanResourceBudget, Pressure, ReconnectMode, ResourceRef,
    ResourceSelector, ResumeProof, SemanticHash, StopPolicy, TerminalClass, WorkloadDelegation,
    resolve_authority,
};
use conduit_runtime::{
    DistributedBackendReadiness, DistributedCordBackend, DistributedFrameKind,
    InMemoryDistributedCordBackend, InMemoryTransportFault, OutboundDistributedFrame,
    RuntimeTimestamp, RuntimeValueEnvelope, decode_distributed_envelope,
    encode_distributed_envelope,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const PLAN: SemanticHash = SemanticHash::from_bytes([90; 32]);

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn peer(
    host: &'static str,
    realm: &'static str,
    entity: &'static str,
    passport_byte: u8,
    grant_byte: u8,
) -> DistributedPeerRequirement<'static> {
    DistributedPeerRequirement {
        node: conduit_core::InstancePath::new(if entity == "fixture/writer" {
            "root/source"
        } else {
            "root/sink"
        })
        .unwrap(),
        host_observation: Id(host),
        realm: Id(realm),
        realm_identity: hash(passport_byte + 20),
        entity: Id(entity),
        passport: hash(passport_byte),
        passport_schema_version: 0,
        credential: if entity == "fixture/writer" {
            Id("fixture/writer-credential")
        } else {
            Id("fixture/reader-credential")
        },
        credential_epoch: 2,
        key: if entity == "fixture/writer" {
            Id("fixture/writer-key")
        } else {
            Id("fixture/reader-key")
        },
        key_epoch: 3,
        status_reporter: pin("fixture/status-provider", 30),
        credential_verifier: pin("fixture/possession-provider", 31),
        audience: Id("fixture/distributed-run"),
        grant_hash: hash(grant_byte),
    }
}

fn binding() -> PlanDistributedCord<'static> {
    let capacity = FlowCapacity::new(4, 64, 256).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Reject,
        FlowWatermarks::new(1, 3, capacity).unwrap(),
    )
    .unwrap();
    let mut value = PlanDistributedCord {
        schema_version: 0,
        identity: ZERO,
        cord: Id("fixture/remote-cord"),
        writer_port_contract_hash: hash(10),
        reader_port_contract_hash: hash(11),
        flow,
        session: Id("fixture/session"),
        initial_session_epoch: 1,
        backend: pin("fixture/distributed-backend", 40),
        backend_artifact: Some(PlanArtifact {
            id: Id("artifact/zenoh-rust"),
            digest: ArtifactDigest::from_bytes([43; 32]),
        }),
        backend_profile: Some(pin("conduit/zenoh-hosted-accounted", 44)),
        carrier_security: pin("fixture/mtls-profile", 41),
        carrier_security_mode: Some(CarrierSecurityMode::MutualTls),
        carrier_endpoint: Some("tls/zenoh.example:7447"),
        carrier_binding: Id("fixture/resolved-carrier-binding"),
        delivery: DistributedDelivery::AtLeastOnce,
        acknowledgement: AcknowledgementMode::Cumulative,
        ordering: DistributedOrdering::InOrder,
        reconnect: ReconnectMode::ResumeSameEpoch,
        disconnect: DisconnectPolicy::AwaitReconnect,
        writer: peer(
            "fixture/writer-report",
            "fixture/realm-a",
            "fixture/writer",
            50,
            60,
        ),
        reader: peer(
            "fixture/reader-report",
            "fixture/realm-b",
            "fixture/reader",
            51,
            61,
        ),
        federation_policy: Some(pin("fixture/federation-a-to-b", 42)),
        budget: DistributedCordBudget {
            send_items: 4,
            send_bytes: 320,
            receive_items: 4,
            receive_bytes: 320,
            retry_items: 4,
            retry_bytes: 320,
            reorder_items: 2,
            reorder_bytes: 160,
            dedup_items: 2,
            maximum_payload_bytes: 64,
            maximum_frame_bytes: 640,
            maximum_unacknowledged: 4,
            maximum_retries: 2,
            maximum_reconnect_attempts: 2,
            heartbeat_interval_ticks: 5,
            liveness_timeout_ticks: 10,
            reconnect_deadline_ticks: 20,
            maximum_evidence_events: 32,
            allocated_memory_bytes: 1_120,
        },
        allocation: PlanResourceBudget {
            memory_bytes: 1_120,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 2,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 32,
        },
    };
    value.identity = value.semantic_hash().unwrap();
    value
}

fn status(requirement: DistributedPeerRequirement<'static>) -> PassportStatusObservation<'static> {
    PassportStatusObservation {
        passport: requirement.passport,
        realm: requirement.realm,
        entity: requirement.entity,
        reporter: requirement.status_reporter,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
        status: PassportStatus::Active,
    }
}

fn possession(requirement: DistributedPeerRequirement<'static>) -> CredentialVerification<'static> {
    let mut value = CredentialVerification {
        identity: ZERO,
        credential: requirement.credential,
        passport: requirement.passport,
        verifier: requirement.credential_verifier,
        challenge: Id("fixture/session"),
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
        outcome: CredentialVerificationOutcome::Verified,
        receipt: hash(70),
    };
    value.identity = value.computed_semantic_hash().unwrap();
    value
}

fn proof(requirement: DistributedPeerRequirement<'static>) -> DistributedPeerProof<'static> {
    DistributedPeerProof {
        credential_epoch: requirement.credential_epoch,
        key: requirement.key,
        key_epoch: requirement.key_epoch,
        status: status(requirement),
        possession: possession(requirement),
        delegation: WorkloadDelegation {
            id: if requirement.entity == Id("fixture/writer") {
                Id("fixture/writer-delegation")
            } else {
                Id("fixture/reader-delegation")
            },
            realm: requirement.realm,
            entity: requirement.entity,
            passport: requirement.passport,
            plan: PLAN,
            run: Id("fixture/run"),
            epoch: 7,
            audience: requirement.audience,
            expires_at_tick: 30,
            depth: 0,
            receipt: hash(71),
        },
    }
}

fn handshake(binding: PlanDistributedCord<'static>) -> DistributedCordHandshake<'static> {
    DistributedCordHandshake {
        protocol_version: 0,
        plan_identity: PLAN,
        binding_identity: binding.identity,
        cord: binding.cord,
        session: binding.session,
        session_epoch: 1,
        run: Id("fixture/run"),
        run_epoch: 7,
        writer: proof(binding.writer),
        reader: proof(binding.reader),
    }
}

fn context() -> DistributedHandshakeContext<'static> {
    DistributedHandshakeContext {
        expected_plan_identity: PLAN,
        now: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
    }
}

fn endpoint_authority(
    requirement: DistributedPeerRequirement<'static>,
    action: &'static str,
    resource_id: &'static str,
    capability_id: &'static str,
    grant_id: &'static str,
    audit_id: &'static str,
) -> PlanAuthority<'static> {
    let resource = ResourceRef {
        kind: Id("fixture/network"),
        id: Id(resource_id),
    };
    let effect = EffectRequirement {
        id: Id(action),
        administrative_class: None,
        policy_budget_class: None,
        action: Id(action),
        resource: ResourceSelector::Exact(resource),
        requester: requirement.node,
        audience: requirement.audience,
        constraints: &[],
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id(capability_id),
        action: effect.action,
        resource,
        host: requirement.host_observation,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
    };
    let grant = AuthorityGrant {
        id: Id(grant_id),
        action: effect.action,
        resource,
        scope: AuthorityScope {
            root: requirement.node,
            descendants: false,
        },
        audience: requirement.audience,
        constraints: &[],
        time_basis: Id("fixture/clock"),
        not_before_tick: 10,
        expires_at_tick: 30,
        issued_for_host: requirement.host_observation,
        delegation: DelegationPolicy::CrossHostDescendants,
        audit_id: Id(audit_id),
        terminal_policy: StopPolicy::Abort,
    };
    let observed = ObservedGrant {
        grant,
        status: GrantStatus::Active,
    };
    PlanAuthority {
        node: requirement.node,
        effect_hash: effect.semantic_hash().unwrap(),
        grant_hash: grant.semantic_hash().unwrap(),
        effect,
        capability,
        grant,
        binding: resolve_authority(
            effect,
            requirement.host_observation,
            AuthorityTime {
                basis: Id("fixture/clock"),
                tick: 20,
            },
            &[capability],
            &[observed],
        )
        .unwrap(),
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
        commit_profile: None,
    }
}

fn plan_authorities(
    binding: PlanDistributedCord<'static>,
) -> (PlanAuthority<'static>, PlanAuthority<'static>) {
    (
        endpoint_authority(
            binding.writer,
            "fixture/connect",
            "fixture/network-a",
            "fixture/network-capability-a",
            "fixture/network-grant-a",
            "fixture/network-audit-a",
        ),
        endpoint_authority(
            binding.reader,
            "fixture/accept",
            "fixture/network-b",
            "fixture/network-capability-b",
            "fixture/network-grant-b",
            "fixture/network-audit-b",
        ),
    )
}

fn authorities(binding: PlanDistributedCord<'static>) -> DistributedAuthorityContext<'static> {
    let (writer, reader) = plan_authorities(binding);
    DistributedAuthorityContext {
        writer,
        writer_grant: ObservedGrant {
            grant: writer.grant,
            status: GrantStatus::Active,
        },
        reader,
        reader_grant: ObservedGrant {
            grant: reader.grant,
            status: GrantStatus::Active,
        },
        now: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
    }
}

fn sealed_binding() -> PlanDistributedCord<'static> {
    let mut binding = binding();
    let (writer, reader) = plan_authorities(binding);
    binding.writer.grant_hash = writer.grant_hash;
    binding.reader.grant_hash = reader.grant_hash;
    binding.identity = binding.semantic_hash().unwrap();
    binding
}

fn open_backend() -> (PlanDistributedCord<'static>, InMemoryDistributedCordBackend) {
    let binding = sealed_binding();
    let mut backend = InMemoryDistributedCordBackend::new();
    backend
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .unwrap();
    (binding, backend)
}

fn value(sequence: u64, payload: &[u8]) -> OutboundDistributedFrame<'_> {
    OutboundDistributedFrame {
        kind: DistributedFrameKind::Value,
        session_epoch: 1,
        sequence: Some(sequence),
        attempt: None,
        correlation: Some(hash(100)),
        value_envelope: None,
        payload,
    }
}

#[test]
fn distributed_envelope_preserves_authorized_value_facts_and_rejects_impersonation() {
    let mut binding = binding();
    binding.budget.maximum_frame_bytes = 1_024;
    binding.identity = binding.semantic_hash().unwrap();
    let mut timestamps = [RuntimeTimestamp::default(); conduit_core::MAX_VALUE_CLOCK_DOMAINS];
    timestamps[0] = RuntimeTimestamp {
        domain_index: 0,
        tick: 44,
        uncertainty_ticks: 2,
    };
    let envelope = RuntimeValueEnvelope {
        representation: hash(101),
        envelope_bytes: 48,
        fragment_count: 1,
        fragment_bytes: 4,
        identity: Some(hash(102)),
        correlation: Some(hash(100)),
        causation: Some(hash(103)),
        provenance: Some(hash(104)),
        timestamp_count: 1,
        timestamps,
        sensitivity: conduit_core::Sensitivity::Restricted,
    };
    let frame = OutboundDistributedFrame {
        value_envelope: Some(envelope),
        ..value(1, b"data")
    };
    let mut bytes = [0_u8; 1_024];
    let used = encode_distributed_envelope(PLAN, &binding, frame, &mut bytes).unwrap();
    let decoded = decode_distributed_envelope(&bytes[..used], PLAN, &binding).unwrap();
    assert_eq!(decoded.frame.value_envelope, Some(envelope));
    assert_eq!(decoded.frame.payload, b"data");

    bytes[210] ^= 1;
    assert_eq!(
        decode_distributed_envelope(&bytes[..used], PLAN, &binding),
        Err(conduit_runtime::TransportReason::EnvelopeIdentityMismatch)
    );

    let (fault_binding, mut backend) = open_backend();
    backend
        .send(&fault_binding, frame, authorities(fault_binding))
        .unwrap();
    let mut payload = [0_u8; 64];
    let received = backend
        .receive(&fault_binding, &mut payload, authorities(fault_binding))
        .unwrap()
        .unwrap();
    assert_eq!(received.value_envelope, Some(envelope));
    assert_eq!(&payload[..received.payload_bytes], b"data");
}

#[test]
fn backend_opens_only_after_exact_handshake_and_emits_correlatable_evidence() {
    let binding = sealed_binding();
    let mut backend = InMemoryDistributedCordBackend::new();
    let mut wrong = handshake(binding);
    wrong.plan_identity = hash(99);
    assert_eq!(
        backend.open(&binding, wrong, context(), authorities(binding)),
        Err(DistributedReason::HandshakeMismatch)
    );
    assert_eq!(
        backend.send_readiness(),
        DistributedBackendReadiness::Closed
    );

    backend
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .unwrap();
    let evidence = backend.take_evidence().unwrap();
    assert_eq!(evidence.kind, DistributedEvidenceKind::HandshakeAccepted);
    assert_eq!(evidence.plan_identity, PLAN);
    assert_eq!(evidence.binding_identity, binding.identity);
    assert_eq!(evidence.cord, binding.cord.as_str());
}

#[test]
fn revoked_authority_fails_closed_at_the_send_boundary() {
    let (binding, mut backend) = open_backend();
    backend.take_evidence();
    let mut authority = authorities(binding);
    authority.writer_grant.status = GrantStatus::Revoked {
        at_tick: 20,
        reason: Id("fixture/revoked"),
    };

    assert_eq!(
        backend.send(&binding, value(0, b"forbidden"), authority),
        Err(DistributedReason::AuthorityDenied)
    );
    assert_eq!(backend.queued_items(), 0);
    let evidence = backend.take_evidence().unwrap();
    assert_eq!(evidence.kind, DistributedEvidenceKind::FrameRejected);
    assert_eq!(evidence.reason, Some(DistributedReason::AuthorityDenied));
}

#[test]
fn mutated_grant_cannot_reuse_the_plan_pinned_grant_hash() {
    let (binding, mut backend) = open_backend();
    backend.take_evidence();
    let mut authority = authorities(binding);
    authority.writer.grant.expires_at_tick = 40;
    authority.writer_grant.grant = authority.writer.grant;

    assert_eq!(
        backend.send(&binding, value(0, b"forbidden"), authority),
        Err(DistributedReason::AuthorityDenied)
    );
    assert_eq!(backend.queued_items(), 0);
    let evidence = backend.take_evidence().unwrap();
    assert_eq!(evidence.kind, DistributedEvidenceKind::FrameRejected);
    assert_eq!(evidence.reason, Some(DistributedReason::AuthorityDenied));
}

#[test]
fn caller_owned_receive_buffer_and_transport_capacity_are_bounded() {
    let (binding, mut backend) = open_backend();
    backend.take_evidence();
    backend
        .send(&binding, value(0, b"first"), authorities(binding))
        .unwrap();
    backend
        .send(&binding, value(1, b"second"), authorities(binding))
        .unwrap();
    assert_eq!(backend.queued_items(), 2);
    assert_eq!(backend.queued_bytes(), 11);

    let mut too_small = [0_u8; 2];
    assert_eq!(
        backend.receive(&binding, &mut too_small, authorities(binding)),
        Err(DistributedReason::BufferFull)
    );
    assert_eq!(backend.queued_items(), 2);

    let mut bytes = [0_u8; 64];
    let frame = backend
        .receive(&binding, &mut bytes, authorities(binding))
        .unwrap()
        .unwrap();
    assert_eq!(&bytes[..frame.payload_bytes], b"first");
    assert_eq!(frame.correlation, Some(hash(100)));
    assert_eq!(backend.queued_items(), 1);
}

#[test]
fn fault_backend_reproduces_duplicate_reorder_lost_ack_and_partition() {
    let (duplicate_binding, mut duplicate) = open_backend();
    duplicate.take_evidence();
    duplicate
        .inject_fault(InMemoryTransportFault::DuplicateNextValue)
        .unwrap();
    duplicate
        .send(
            &duplicate_binding,
            value(0, b"value"),
            authorities(duplicate_binding),
        )
        .unwrap();
    assert_eq!(duplicate.queued_items(), 2);

    let (reordered_binding, mut reordered) = open_backend();
    reordered.take_evidence();
    reordered
        .inject_fault(InMemoryTransportFault::ReorderNextValuePair)
        .unwrap();
    reordered
        .send(
            &reordered_binding,
            value(0, b"zero"),
            authorities(reordered_binding),
        )
        .unwrap();
    assert_eq!(
        reordered.receive_readiness(),
        DistributedBackendReadiness::Pending
    );
    reordered
        .send(
            &reordered_binding,
            value(1, b"one"),
            authorities(reordered_binding),
        )
        .unwrap();
    let mut bytes = [0_u8; 64];
    assert_eq!(
        reordered
            .receive(
                &reordered_binding,
                &mut bytes,
                authorities(reordered_binding),
            )
            .unwrap()
            .unwrap()
            .sequence,
        Some(1)
    );
    assert_eq!(
        reordered
            .receive(
                &reordered_binding,
                &mut bytes,
                authorities(reordered_binding),
            )
            .unwrap()
            .unwrap()
            .sequence,
        Some(0)
    );

    let (lost_ack_binding, mut lost_ack) = open_backend();
    lost_ack.take_evidence();
    lost_ack
        .inject_fault(InMemoryTransportFault::DropNextAcknowledgement)
        .unwrap();
    lost_ack
        .send(
            &lost_ack_binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::Acknowledgement,
                session_epoch: 1,
                sequence: Some(0),
                attempt: None,
                correlation: None,
                value_envelope: None,
                payload: &[],
            },
            authorities(lost_ack_binding),
        )
        .unwrap();
    assert_eq!(
        lost_ack.receive_readiness(),
        DistributedBackendReadiness::Pending
    );

    let (lost_terminal_ack_binding, mut lost_terminal_ack) = open_backend();
    lost_terminal_ack.take_evidence();
    lost_terminal_ack
        .inject_fault(InMemoryTransportFault::DropNextTerminalAcknowledgement)
        .unwrap();
    lost_terminal_ack
        .send(
            &lost_terminal_ack_binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::TerminalAcknowledgement,
                session_epoch: 1,
                sequence: Some(1),
                attempt: None,
                correlation: None,
                value_envelope: None,
                payload: &[],
            },
            authorities(lost_terminal_ack_binding),
        )
        .unwrap();
    assert_eq!(
        lost_terminal_ack.receive_readiness(),
        DistributedBackendReadiness::Pending
    );
    assert_eq!(
        lost_terminal_ack.take_evidence().unwrap().kind,
        DistributedEvidenceKind::FrameDropped
    );

    let (partitioned_binding, mut partitioned) = open_backend();
    partitioned.take_evidence();
    partitioned.set_partitioned(true);
    assert_eq!(
        partitioned.send(
            &partitioned_binding,
            value(0, b"value"),
            authorities(partitioned_binding),
        ),
        Err(DistributedReason::Partitioned)
    );
    let evidence = partitioned.take_evidence().unwrap();
    assert_eq!(evidence.kind, DistributedEvidenceKind::Disconnected);
    assert_eq!(evidence.reason, Some(DistributedReason::Partitioned));
}

#[test]
fn oversized_control_and_terminal_frames_follow_the_same_boundary() {
    let (binding, mut backend) = open_backend();
    backend.take_evidence();
    let oversized = vec![0_u8; binding.budget.maximum_payload_bytes as usize + 1];
    assert_eq!(
        backend.send(&binding, value(0, &oversized), authorities(binding)),
        Err(DistributedReason::OversizedFrame)
    );
    assert_eq!(backend.queued_items(), 0);
    let evidence = backend.take_evidence().unwrap();
    assert_eq!(evidence.kind, DistributedEvidenceKind::FrameRejected);
    assert_eq!(evidence.reason, Some(DistributedReason::OversizedFrame));

    backend
        .cancel(&binding, 1, 0, Some(hash(101)), authorities(binding))
        .unwrap();
    backend
        .close(
            &binding,
            1,
            1,
            TerminalClass::Succeeded,
            Some(hash(102)),
            authorities(binding),
        )
        .unwrap();
    let mut bytes = [];
    assert_eq!(
        backend
            .receive(&binding, &mut bytes, authorities(binding))
            .unwrap()
            .unwrap()
            .kind,
        DistributedFrameKind::Cancellation
    );
    assert_eq!(
        backend
            .receive(&binding, &mut bytes, authorities(binding))
            .unwrap()
            .unwrap()
            .kind,
        DistributedFrameKind::Terminal(TerminalClass::Succeeded)
    );
}

#[test]
fn evidence_ceiling_fails_before_mutating_frame_queues() {
    let (binding, mut backend) = open_backend();
    let mut destination = [0_u8; 64];
    for sequence in 0..15 {
        backend
            .send(&binding, value(sequence, b"value"), authorities(binding))
            .unwrap();
        backend
            .receive(&binding, &mut destination, authorities(binding))
            .unwrap()
            .unwrap();
    }
    backend
        .send(&binding, value(15, b"retained"), authorities(binding))
        .unwrap();
    assert_eq!(backend.queued_items(), 1);
    assert_eq!(
        backend.send(&binding, value(16, b"rejected"), authorities(binding)),
        Err(DistributedReason::EvidenceFull)
    );
    assert_eq!(backend.queued_items(), 1);
    assert_eq!(
        backend.receive(&binding, &mut destination, authorities(binding)),
        Err(DistributedReason::EvidenceFull)
    );
    assert_eq!(backend.queued_items(), 1);
}

#[test]
fn reconnect_reauthenticates_live_proofs_and_requires_resume_receipt() {
    let (binding, mut backend) = open_backend();
    backend.take_evidence();
    backend.set_partitioned(true);
    let mut stale = handshake(binding);
    stale.writer.status.valid_until_tick = 20;
    assert_eq!(
        backend.reauthenticate(&binding, stale, context(), None, authorities(binding)),
        Err(DistributedReason::StalePeerStatus)
    );
    assert_eq!(
        backend.send_readiness(),
        DistributedBackendReadiness::Pending
    );

    let proof = ResumeProof {
        plan_identity: PLAN,
        binding_identity: binding.identity,
        session_epoch: 1,
        writer_next_sequence: 0,
        reader_next_sequence: 0,
        acknowledged_through: None,
        receipt: hash(72),
    };
    backend
        .reauthenticate(
            &binding,
            handshake(binding),
            context(),
            Some(proof),
            authorities(binding),
        )
        .unwrap();
    assert_eq!(backend.send_readiness(), DistributedBackendReadiness::Ready);
    assert_eq!(
        backend.take_evidence().unwrap().kind,
        DistributedEvidenceKind::Reconnected
    );
}
