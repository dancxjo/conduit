use conduit_core::{
    AcknowledgementMode, ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime,
    BlockingFairness, CredentialVerification, CredentialVerificationOutcome, DelegationPolicy,
    DisconnectPolicy, DistributedAuthorityContext, DistributedCordBudget, DistributedCordHandshake,
    DistributedDelivery, DistributedHandshakeContext, DistributedOrdering, DistributedPeerProof,
    DistributedPeerRequirement, EffectRequirement, FlowCapacity, FlowPolicy, FlowWatermarks,
    GrantStatus, HostCapability, Id, ObservedGrant, PassportStatus, PassportStatusObservation,
    PinnedDescriptor, PlanArtifact, PlanAuthority, PlanDistributedCord, PlanResourceBudget,
    Pressure, ReconnectMode, ResourceRef, ResourceSelector, SemanticHash, StopPolicy,
    WorkloadDelegation, resolve_authority,
};
use conduit_runtime::{
    CarrierSecurityCapabilities, CarrierSecurityMode, DISTRIBUTED_ENVELOPE_VERSION,
    ResolvedPlacementBinding, ResolvedReplacementSupport, ResolvedTransportSelection,
    TransportCapabilities,
};

pub const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
pub const PLAN: SemanticHash = SemanticHash::from_bytes([90; 32]);

pub const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

pub const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
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

pub fn binding<'a>(endpoint: &'a str, security: CarrierSecurityMode) -> PlanDistributedCord<'a> {
    let capacity = FlowCapacity::new(4, 64, 256).unwrap();
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
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
        backend: pin("conduit/transport.zenoh-rust", 40),
        backend_artifact: Some(PlanArtifact {
            id: Id("artifact/zenoh-rust-1-9-0"),
            digest: ArtifactDigest::from_bytes([44; 32]),
        }),
        backend_profile: Some(pin("conduit/profile.zenoh-hosted-observed", 45)),
        carrier_security: security_pin(security),
        carrier_security_mode: Some(security),
        carrier_endpoint: Some(endpoint),
        carrier_binding: Id("conduit/zenoh-tests-session"),
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
            send_bytes: 256,
            receive_items: 4,
            receive_bytes: 2_048,
            retry_items: 4,
            retry_bytes: 256,
            reorder_items: 2,
            reorder_bytes: 128,
            dedup_items: 2,
            maximum_payload_bytes: 64,
            maximum_frame_bytes: 512,
            maximum_unacknowledged: 4,
            maximum_retries: 2,
            maximum_reconnect_attempts: 2,
            heartbeat_interval_ticks: 5,
            liveness_timeout_ticks: 10,
            reconnect_deadline_ticks: 20,
            maximum_evidence_events: 32,
            allocated_memory_bytes: 2_097_152,
        },
        allocation: PlanResourceBudget {
            memory_bytes: 2_097_152,
            storage_bytes: 0,
            cpu_units: 1,
            timers: 4,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 32 * 256,
        },
    };
    value.writer.grant_hash = endpoint_authority(
        value.writer,
        "fixture/connect",
        "fixture/network-a",
        "fixture/network-capability-a",
        "fixture/network-grant-a",
        "fixture/network-audit-a",
    )
    .grant_hash;
    value.reader.grant_hash = endpoint_authority(
        value.reader,
        "fixture/accept",
        "fixture/network-b",
        "fixture/network-capability-b",
        "fixture/network-grant-b",
        "fixture/network-audit-b",
    )
    .grant_hash;
    value.identity = value.semantic_hash().unwrap();
    value
}

pub const fn security_pin(mode: CarrierSecurityMode) -> PinnedDescriptor<'static> {
    match mode {
        CarrierSecurityMode::Plaintext => pin("conduit/carrier-security.plaintext", 41),
        CarrierSecurityMode::Tls => pin("conduit/carrier-security.tls", 42),
        CarrierSecurityMode::MutualTls => pin("conduit/carrier-security.mtls", 43),
    }
}

pub fn capabilities(binding: &PlanDistributedCord<'_>) -> TransportCapabilities {
    TransportCapabilities {
        protocol_version: DISTRIBUTED_ENVELOPE_VERSION,
        publish_subscribe: true,
        query_reply: false,
        reconnect: true,
        deterministic_faults: false,
        security: CarrierSecurityCapabilities {
            plaintext: true,
            tls: true,
            mutual_tls: true,
        },
        maximum_frame_bytes: binding.budget.maximum_frame_bytes,
        adapter_send_items: 1,
        adapter_receive_items: binding.budget.receive_items,
        adapter_evidence_items: binding.budget.maximum_evidence_events,
        carrier_queue_items: 1,
        carrier_queue_bytes: u64::from(binding.budget.maximum_frame_bytes),
        receive_buffer_bytes: u64::from(binding.budget.maximum_frame_bytes),
        defragmentation_bytes: u64::from(binding.budget.maximum_frame_bytes),
        socket_send_bytes: 4_096,
        socket_receive_bytes: 4_096,
        session_state_bytes: 1_048_576,
        discovery_state_bytes: 0,
        pending_operation_bytes: u64::from(binding.budget.maximum_frame_bytes),
        retained_payload_bytes: u64::from(binding.budget.maximum_frame_bytes),
        timer_state_bytes: 4_096,
        worker_stack_bytes: 0,
        pending_links: 1,
        maximum_links: 1,
        maximum_sessions: 1,
        retry_timers: 2,
        complete_stack_hard_bounded: false,
    }
}

pub fn placement(binding: &PlanDistributedCord<'_>) -> ResolvedPlacementBinding {
    let artifact = binding.backend_artifact.unwrap();
    ResolvedPlacementBinding {
        instance: "root/transport".to_owned(),
        semantic_contract: hash(39),
        implementation_id: binding.backend.id.as_str().to_owned(),
        implementation_identity: binding.backend.semantic_hash,
        replacement: ResolvedReplacementSupport::Cold,
        host: "fixture/linux-host".to_owned(),
        report_id: "fixture/linux-report".to_owned(),
        report_identity: hash(46),
        report_time_basis: "fixture/clock".to_owned(),
        report_observed_at_tick: 10,
        report_valid_until_tick: 30,
        allocation: binding.allocation,
        artifacts: vec![(artifact.id.as_str().to_owned(), artifact.digest)],
        capability_subjects: vec!["fixture/network-interface".to_owned()],
        capability_proofs: Vec::new(),
        resource_ids: vec!["fixture/network-interface".to_owned()],
        authority_grants: vec![
            "fixture/network-grant-a".to_owned(),
            "fixture/network-grant-b".to_owned(),
        ],
    }
}

pub fn selection<'a>(
    binding: &PlanDistributedCord<'a>,
    security_mode: CarrierSecurityMode,
) -> ResolvedTransportSelection<'a> {
    ResolvedTransportSelection {
        backend: binding.backend,
        artifact: binding.backend_artifact.unwrap(),
        execution_profile: binding.backend_profile.unwrap(),
        endpoint: binding.carrier_endpoint.unwrap(),
        carrier_binding: binding.carrier_binding,
        security_descriptor: binding.carrier_security,
        security_mode,
        capabilities: capabilities(binding),
    }
}

fn status<'a>(requirement: DistributedPeerRequirement<'a>) -> PassportStatusObservation<'a> {
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

fn possession<'a>(requirement: DistributedPeerRequirement<'a>) -> CredentialVerification<'a> {
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

fn proof<'a>(requirement: DistributedPeerRequirement<'a>) -> DistributedPeerProof<'a> {
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

pub fn handshake<'a>(binding: PlanDistributedCord<'a>) -> DistributedCordHandshake<'a> {
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

pub fn context() -> DistributedHandshakeContext<'static> {
    DistributedHandshakeContext {
        expected_plan_identity: PLAN,
        now: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
    }
}

fn endpoint_authority<'a>(
    requirement: DistributedPeerRequirement<'a>,
    action: &'a str,
    resource_id: &'a str,
    capability_id: &'a str,
    grant_id: &'a str,
    audit_id: &'a str,
) -> PlanAuthority<'a> {
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

fn plan_authorities<'a>(
    binding: PlanDistributedCord<'a>,
) -> (PlanAuthority<'a>, PlanAuthority<'a>) {
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

pub fn authorities<'a>(binding: PlanDistributedCord<'a>) -> DistributedAuthorityContext<'a> {
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
