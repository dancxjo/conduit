use conduit_core::{
    AcknowledgementMode, ArtifactDigest, AuthorityGrant, AuthorityScope, AuthorityTime,
    BoundednessProfile, CancellationGuarantee, CarrierSecurityMode, CredentialVerification,
    CredentialVerificationOutcome, DelegationPolicy, Direction, DisconnectPolicy,
    DistributedCordBudget, DistributedCordHandshake, DistributedDelivery,
    DistributedHandshakeContext, DistributedOrdering, DistributedPeerProof,
    DistributedPeerRequirement, DistributedReason, DistributedSessionMachine,
    DistributedSessionState, EffectRequirement, ExecutionLimits, ExecutionPlan, ExecutionProfile,
    FlowCapacity, FlowPolicy, FlowWatermarks, GrantStatus, HostCapability, Id, InstancePath,
    MemoryClaim, ObservedGrant, PassportStatus, PassportStatusObservation, PendingControl,
    PinnedDescriptor, PlanArtifact, PlanAuthority, PlanDiagnosticCode, PlanDistributedCord,
    PlanHostObservation, PlanResourceBinding, PlanResourceBudget, PlanValidationContext, Pressure,
    ReceiveDisposition, ReconnectMode, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort,
    ResourceRef, ResourceSelector, ResumeProof, SemanticHash, StopPolicy, TerminalClass,
    TypeContractRef, WorkloadDelegation, resolve_authority, validate_distributed_binding,
    validate_distributed_handshake, validate_execution_plan,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../conformance/c5/distributed-cord-v1.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const PLAN: SemanticHash = SemanticHash::from_bytes([90; 32]);

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn flow() -> FlowPolicy<'static> {
    let capacity = FlowCapacity::new(4, 64, 256).unwrap();
    FlowPolicy::new(
        capacity,
        Pressure::Reject,
        FlowWatermarks::new(1, 3, capacity).unwrap(),
    )
    .unwrap()
}

fn peer(
    host: &'static str,
    realm: &'static str,
    entity: &'static str,
    passport_byte: u8,
    grant_byte: u8,
) -> DistributedPeerRequirement<'static> {
    DistributedPeerRequirement {
        node: InstancePath::new(if entity == "fixture/writer" {
            "root/source"
        } else {
            "root/sink"
        })
        .unwrap(),
        host_observation: Id(host),
        realm: Id(realm),
        realm_identity: hash(passport_byte.saturating_add(20)),
        entity: Id(entity),
        passport: hash(passport_byte),
        passport_schema_version: 1,
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
    let mut value = PlanDistributedCord {
        schema_version: 1,
        identity: ZERO,
        cord: Id("fixture/remote-cord"),
        writer_port_contract_hash: hash(10),
        reader_port_contract_hash: hash(11),
        flow: flow(),
        session: Id("fixture/session"),
        initial_session_epoch: 1,
        backend: pin("fixture/distributed-backend", 40),
        backend_artifact: None,
        backend_profile: None,
        carrier_security: pin("fixture/mtls-profile", 41),
        carrier_security_mode: None,
        carrier_endpoint: None,
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
            send_items: 2,
            send_bytes: 160,
            receive_items: 2,
            receive_bytes: 160,
            retry_items: 2,
            retry_bytes: 160,
            reorder_items: 2,
            reorder_bytes: 160,
            dedup_items: 2,
            maximum_payload_bytes: 64,
            maximum_frame_bytes: 80,
            maximum_unacknowledged: 2,
            maximum_retries: 2,
            maximum_reconnect_attempts: 2,
            heartbeat_interval_ticks: 5,
            liveness_timeout_ticks: 10,
            reconnect_deadline_ticks: 20,
            maximum_evidence_events: 32,
            allocated_memory_bytes: 640,
        },
        allocation: PlanResourceBudget {
            memory_bytes: 640,
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

fn binding_v2() -> PlanDistributedCord<'static> {
    let mut value = binding();
    value.schema_version = 2;
    value.backend_artifact = Some(PlanArtifact {
        id: Id("artifact/zenoh-rust-1-9-0"),
        digest: ArtifactDigest::from_bytes([43; 32]),
    });
    value.backend_profile = Some(pin("conduit/zenoh-hosted-accounted", 44));
    value.carrier_security_mode = Some(CarrierSecurityMode::MutualTls);
    value.carrier_endpoint = Some("tls/zenoh.example:7447");
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

fn possession(
    requirement: DistributedPeerRequirement<'static>,
    outcome: CredentialVerificationOutcome,
) -> CredentialVerification<'static> {
    let mut value = CredentialVerification {
        identity: ZERO,
        credential: requirement.credential,
        passport: requirement.passport,
        verifier: requirement.credential_verifier,
        challenge: Id("fixture/session"),
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
        outcome,
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
        possession: possession(requirement, CredentialVerificationOutcome::Verified),
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
        protocol_version: 1,
        plan_identity: PLAN,
        binding_identity: binding.identity,
        cord: binding.cord,
        session: binding.session,
        session_epoch: binding.initial_session_epoch,
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

fn open_machine(binding: &PlanDistributedCord<'_>) -> DistributedSessionMachine {
    let mut machine = DistributedSessionMachine::new(binding.initial_session_epoch);
    machine.establish(10).unwrap();
    machine
}

fn rejected(reason: DistributedReason) -> Value {
    json!({"accepted": false, "reason": reason.code()})
}

fn execute(case: &str) -> Value {
    let mut binding = binding();
    match case {
        "successful-realm-aware-handshake" => {
            validate_distributed_binding(&binding).unwrap();
            validate_distributed_handshake(&binding, handshake(binding), context()).unwrap();
            json!({"accepted": true})
        }
        "plan-type-policy-mismatch" => {
            let mut offered = handshake(binding);
            offered.binding_identity = hash(99);
            rejected(validate_distributed_handshake(&binding, offered, context()).unwrap_err())
        }
        "stale-peer-status" => {
            let mut offered = handshake(binding);
            offered.writer.status.valid_until_tick = 20;
            rejected(validate_distributed_handshake(&binding, offered, context()).unwrap_err())
        }
        "replayed-possession-proof" => {
            let mut offered = handshake(binding);
            offered.writer.possession.outcome = CredentialVerificationOutcome::Replayed;
            offered.writer.possession.identity =
                offered.writer.possession.computed_semantic_hash().unwrap();
            rejected(validate_distributed_handshake(&binding, offered, context()).unwrap_err())
        }
        "wrong-delegation-audience" => {
            let mut offered = handshake(binding);
            offered.reader.delegation.audience = Id("fixture/wrong-audience");
            rejected(validate_distributed_handshake(&binding, offered, context()).unwrap_err())
        }
        "lost-ack-bounded-retry" => {
            let mut sender = open_machine(&binding);
            let mut receiver = open_machine(&binding);
            let sequence = sender.begin_send(&binding, 20).unwrap();
            assert_eq!(
                receiver.receive(&binding, sequence, 20).unwrap(),
                ReceiveDisposition::Accepted
            );
            sender.retry(&binding, sequence, 1).unwrap();
            let duplicate = receiver.receive(&binding, sequence, 20).unwrap();
            json!({"retry": true, "duplicate": match duplicate {
                ReceiveDisposition::DuplicateSuppressed => "suppressed",
                _ => "unexpected",
            }})
        }
        "duplicate-outside-dedup-window" => {
            let mut receiver = open_machine(&binding);
            for sequence in 0..3 {
                assert_eq!(
                    receiver.receive(&binding, sequence, 20).unwrap(),
                    ReceiveDisposition::Accepted
                );
            }
            let duplicate = receiver.receive(&binding, 0, 20).unwrap();
            json!({"duplicate": match duplicate {
                ReceiveDisposition::DuplicateRedelivered => "redelivered",
                _ => "unexpected",
            }})
        }
        "reordering-within-window" => {
            let mut receiver = open_machine(&binding);
            assert_eq!(
                receiver.receive(&binding, 1, 20).unwrap(),
                ReceiveDisposition::HeldForMissingSequence
            );
            json!({"disposition": "held-for-missing-sequence"})
        }
        "retry-window-exhaustion" => {
            let mut sender = open_machine(&binding);
            let sequence = sender.begin_send(&binding, 20).unwrap();
            rejected(
                sender
                    .retry(&binding, sequence, binding.budget.maximum_retries + 1)
                    .unwrap_err(),
            )
        }
        "partition-is-explicit" => {
            let mut machine = open_machine(&binding);
            let reason = machine.observe_liveness(&binding, 20).unwrap_err();
            assert_eq!(machine.state, DistributedSessionState::Disconnected);
            json!({"state": "disconnected", "reason": reason.code()})
        }
        "reconnect-same-epoch-proof" => {
            let mut machine = open_machine(&binding);
            machine.disconnect(&binding).unwrap();
            machine
                .resume_same_epoch(
                    &binding,
                    PLAN,
                    ResumeProof {
                        plan_identity: PLAN,
                        binding_identity: binding.identity,
                        session_epoch: 1,
                        writer_next_sequence: 0,
                        reader_next_sequence: 0,
                        acknowledged_through: None,
                        receipt: hash(72),
                    },
                )
                .unwrap();
            json!({"accepted": true, "epoch": machine.session_epoch})
        }
        "reconnect-new-epoch" => {
            binding.reconnect = ReconnectMode::BeginNewEpoch;
            binding.identity = binding.semantic_hash().unwrap();
            let mut machine = open_machine(&binding);
            machine.begin_send(&binding, 20).unwrap();
            machine.disconnect(&binding).unwrap();
            machine.begin_new_epoch(&binding, 2).unwrap();
            json!({
                "accepted": true,
                "epoch": machine.session_epoch,
                "sequence_reset": machine.next_send_sequence == 0
            })
        }
        "cancellation-requires-ack" => {
            let mut machine = open_machine(&binding);
            machine.request_cancel().unwrap();
            assert_eq!(machine.state, DistributedSessionState::CancelPending);
            machine
                .acknowledge_control(PendingControl::Cancellation)
                .unwrap();
            json!({"before": "cancel-pending", "after": "closed"})
        }
        "terminal-ack-loss-remains-pending" => {
            let mut machine = open_machine(&binding);
            machine.request_terminal(TerminalClass::Succeeded).unwrap();
            assert_eq!(machine.state, DistributedSessionState::TerminalPending);
            json!({"state": "terminal-pending"})
        }
        "transport-buffer-full" => {
            let mut machine = open_machine(&binding);
            machine.begin_send(&binding, 20).unwrap();
            machine.begin_send(&binding, 20).unwrap();
            rejected(machine.begin_send(&binding, 20).unwrap_err())
        }
        "hostile-oversized-frame" => {
            let mut machine = open_machine(&binding);
            rejected(
                machine
                    .begin_send(&binding, binding.budget.maximum_payload_bytes + 1)
                    .unwrap_err(),
            )
        }
        other => panic!("fixture case `{other}` has no executable reference assertion"),
    }
}

#[test]
fn every_distributed_fixture_case_executes() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(execute(id), case["expected"], "case `{id}`");
    }
}

#[test]
fn binding_identity_covers_delivery_and_allocation() {
    let original = binding();
    let mut changed = original;
    changed.budget.maximum_retries += 1;
    assert_ne!(original.identity, changed.semantic_hash().unwrap());

    changed = original;
    changed.allocation.memory_bytes += 1;
    assert_ne!(original.identity, changed.semantic_hash().unwrap());
}

#[test]
fn binding_v2_pins_transport_artifact_profile_and_endpoint_without_reinterpreting_v1() {
    let legacy = binding();
    validate_distributed_binding(&legacy).expect("frozen schema-1 binding");
    let exact = binding_v2();
    validate_distributed_binding(&exact).expect("schema-2 exact transport binding");

    let mut changed = exact;
    changed.backend_artifact = Some(PlanArtifact {
        digest: ArtifactDigest::from_bytes([99; 32]),
        ..changed.backend_artifact.unwrap()
    });
    assert_ne!(exact.identity, changed.semantic_hash().unwrap());
    changed = exact;
    changed.backend_profile = Some(PinnedDescriptor {
        semantic_hash: hash(99),
        ..changed.backend_profile.unwrap()
    });
    assert_ne!(exact.identity, changed.semantic_hash().unwrap());
    changed = exact;
    changed.carrier_endpoint = Some("tls/other.example:7447");
    assert_ne!(exact.identity, changed.semantic_hash().unwrap());
    changed = exact;
    changed.carrier_security_mode = Some(CarrierSecurityMode::Tls);
    assert_ne!(exact.identity, changed.semantic_hash().unwrap());

    let mut incomplete = exact;
    incomplete.backend_profile = None;
    incomplete.identity = incomplete.semantic_hash().unwrap();
    assert_eq!(
        validate_distributed_binding(&incomplete),
        Err(DistributedReason::InvalidBinding)
    );
}

fn profile() -> ExecutionProfile<'static> {
    let mut profile = ExecutionProfile {
        id: Id("fixture/distributed-profile"),
        schema_version: 1,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: ExecutionLimits {
            max_step_work: 8,
            max_retained_values: 0,
            max_retained_bytes: 0,
            max_scratch_bytes: 0,
            max_input_leases: 0,
            max_input_bytes: 0,
            max_output_reservations: 0,
            max_output_bytes: 0,
            max_transactions: 1,
            max_fragments_per_step: 0,
            max_pending_operations: 0,
            max_timers: 0,
            max_child_tasks: 0,
            max_host_buffer_bytes: 0,
            max_foreign_queue_items: 0,
            max_foreign_queue_bytes: 0,
            max_checkpoint_bytes: 0,
            implementation_memory_bytes: 0,
            cancellation_ticks: 1,
        },
        representations: &[],
        memory_claims: &[] as &[MemoryClaim],
        checkpoint: None,
    };
    profile.semantic_hash = profile.computed_semantic_hash(&mut []).unwrap();
    profile
}

#[test]
fn schema_nine_requires_one_exact_binding_for_each_cross_host_cord() {
    let profile = profile();
    let observations = [
        PlanHostObservation {
            id: Id("fixture/writer-report"),
            host: Id("fixture/writer-host"),
            semantic_hash: hash(80),
            time_basis: Id("fixture/clock"),
            observed_at_tick: 10,
            valid_until_tick: 30,
        },
        PlanHostObservation {
            id: Id("fixture/reader-report"),
            host: Id("fixture/reader-host"),
            semantic_hash: hash(81),
            time_basis: Id("fixture/clock"),
            observed_at_tick: 10,
            valid_until_tick: 30,
        },
    ];
    let resource_a = ResourceRef {
        kind: Id("fixture/network"),
        id: Id("fixture/network-a"),
    };
    let resource_b = ResourceRef {
        kind: Id("fixture/network"),
        id: Id("fixture/network-b"),
    };
    let source = InstancePath::new("root/source").unwrap();
    let sink = InstancePath::new("root/sink").unwrap();
    let effect_a = EffectRequirement {
        id: Id("fixture/connect"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("fixture/connect"),
        resource: ResourceSelector::Exact(resource_a),
        requester: source,
        audience: Id("fixture/distributed-run"),
        constraints: &[],
        check_at_use: true,
    };
    let effect_b = EffectRequirement {
        id: Id("fixture/accept"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("fixture/accept"),
        resource: ResourceSelector::Exact(resource_b),
        requester: sink,
        audience: Id("fixture/distributed-run"),
        constraints: &[],
        check_at_use: true,
    };
    let capability_a = HostCapability {
        id: Id("fixture/network-capability-a"),
        action: effect_a.action,
        resource: resource_a,
        host: observations[0].host,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
    };
    let capability_b = HostCapability {
        id: Id("fixture/network-capability-b"),
        action: effect_b.action,
        resource: resource_b,
        host: observations[1].host,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
    };
    let grant_a = AuthorityGrant {
        id: Id("fixture/network-grant-a"),
        action: effect_a.action,
        resource: resource_a,
        scope: AuthorityScope {
            root: source,
            descendants: false,
        },
        audience: effect_a.audience,
        constraints: &[],
        time_basis: Id("fixture/clock"),
        not_before_tick: 10,
        expires_at_tick: 30,
        issued_for_host: observations[0].host,
        delegation: DelegationPolicy::CrossHostDescendants,
        audit_id: Id("fixture/network-audit-a"),
        terminal_policy: StopPolicy::Abort,
    };
    let grant_b = AuthorityGrant {
        id: Id("fixture/network-grant-b"),
        action: effect_b.action,
        resource: resource_b,
        scope: AuthorityScope {
            root: sink,
            descendants: false,
        },
        audience: effect_b.audience,
        constraints: &[],
        time_basis: Id("fixture/clock"),
        not_before_tick: 10,
        expires_at_tick: 30,
        issued_for_host: observations[1].host,
        delegation: DelegationPolicy::CrossHostDescendants,
        audit_id: Id("fixture/network-audit-b"),
        terminal_policy: StopPolicy::Abort,
    };
    let authority_a = PlanAuthority {
        node: source,
        effect_hash: effect_a.semantic_hash().unwrap(),
        grant_hash: grant_a.semantic_hash().unwrap(),
        effect: effect_a,
        capability: capability_a,
        grant: grant_a,
        binding: resolve_authority(
            effect_a,
            observations[0].host,
            AuthorityTime {
                basis: Id("fixture/clock"),
                tick: 20,
            },
            &[capability_a],
            &[ObservedGrant {
                grant: grant_a,
                status: GrantStatus::Active,
            }],
        )
        .unwrap(),
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
    };
    let authority_b = PlanAuthority {
        node: sink,
        effect_hash: effect_b.semantic_hash().unwrap(),
        grant_hash: grant_b.semantic_hash().unwrap(),
        effect: effect_b,
        capability: capability_b,
        grant: grant_b,
        binding: resolve_authority(
            effect_b,
            observations[1].host,
            AuthorityTime {
                basis: Id("fixture/clock"),
                tick: 20,
            },
            &[capability_b],
            &[ObservedGrant {
                grant: grant_b,
                status: GrantStatus::Active,
            }],
        )
        .unwrap(),
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
    };
    let authorities = [authority_a, authority_b];
    let source_effects = [authority_a.effect_hash];
    let sink_effects = [authority_b.effect_hash];
    let source_resources = [Id("fixture/network-binding-a")];
    let sink_resources = [Id("fixture/network-binding-b")];
    let resources = [
        PlanResourceBinding {
            id: source_resources[0],
            node: source,
            resource: resource_a,
            host_observation: observations[0].id,
        },
        PlanResourceBinding {
            id: sink_resources[0],
            node: sink,
            resource: resource_b,
            host_observation: observations[1].id,
        },
    ];
    let artifacts = [
        PlanArtifact {
            id: Id("fixture/source-artifact"),
            digest: ArtifactDigest::from_bytes([82; 32]),
        },
        PlanArtifact {
            id: Id("fixture/sink-artifact"),
            digest: ArtifactDigest::from_bytes([83; 32]),
        },
        PlanArtifact {
            id: Id("artifact/zenoh-rust-1-9-0"),
            digest: ArtifactDigest::from_bytes([43; 32]),
        },
    ];
    let node_allocation = PlanResourceBudget {
        memory_bytes: 100,
        cpu_units: 1,
        ..PlanResourceBudget::ZERO
    };
    let nodes = [
        ResolvedPlanNode {
            instance: source,
            contract: pin("fixture/source-contract", 84),
            implementation: pin("fixture/source-implementation", 85),
            lifecycle_policy: pin("fixture/lifecycle", 86),
            execution_profile: Some(&profile),
            artifact: artifacts[0].id,
            host_observation: observations[0].id,
            host: observations[0].host,
            allocation: node_allocation,
            required_resources: &source_resources,
            required_effects: &source_effects,
        },
        ResolvedPlanNode {
            instance: sink,
            contract: pin("fixture/sink-contract", 87),
            implementation: pin("fixture/sink-implementation", 88),
            lifecycle_policy: pin("fixture/lifecycle", 86),
            execution_profile: Some(&profile),
            artifact: artifacts[1].id,
            host_observation: observations[1].id,
            host: observations[1].host,
            allocation: node_allocation,
            required_resources: &sink_resources,
            required_effects: &sink_effects,
        },
    ];
    let value_type = TypeContractRef {
        contract_id: Id("fixture/value"),
        schema_version: 1,
        semantic_hash: hash(89),
    };
    let cords = [ResolvedPlanCord {
        id: Id("fixture/remote-cord"),
        from: ResolvedPlanPort {
            node: source,
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(10),
            value_type,
        },
        to: ResolvedPlanPort {
            node: sink,
            port: Id("in"),
            direction: Direction::Input,
            port_contract_hash: hash(11),
            value_type,
        },
        flow: flow(),
        queue_memory_bytes: 256,
    }];
    let mut distributed = binding();
    distributed.writer.grant_hash = authority_a.grant_hash;
    distributed.reader.grant_hash = authority_b.grant_hash;
    distributed.identity = distributed.semantic_hash().unwrap();
    let distributed_cords = [distributed];
    let mut plan = ExecutionPlan {
        schema_version: 9,
        identity: ZERO,
        source_semantic_hash: hash(91),
        resolver: pin("fixture/resolver", 92),
        resolver_policy_hash: hash(93),
        created_at: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
        budget: PlanResourceBudget {
            memory_bytes: 1_200,
            storage_bytes: 0,
            cpu_units: 4,
            timers: 4,
            transports: 1,
            checkpoints: 0,
            evidence_bytes: 64,
        },
        host_observations: &observations,
        resources: &resources,
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &[],
        distributed_cords: &distributed_cords,
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &authorities,
        hazard_closure: None,
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        supervisions: &[],
        unresolved: &[],
    };
    let mut scratch = [ZERO; 64];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    let context = PlanValidationContext {
        supported_schema_version: 9,
        now: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
    };
    assert_eq!(
        validate_execution_plan(&plan, context, &mut scratch),
        Ok(())
    );

    let mut exact_transport = distributed;
    exact_transport.schema_version = 2;
    exact_transport.backend_artifact = Some(artifacts[2]);
    exact_transport.backend_profile = Some(pin("conduit/zenoh-hosted-accounted", 44));
    exact_transport.carrier_security_mode = Some(CarrierSecurityMode::MutualTls);
    exact_transport.carrier_endpoint = Some("tls/zenoh.example:7447");
    exact_transport.identity = exact_transport.semantic_hash().unwrap();
    let exact_transports = [exact_transport];
    let mut current = ExecutionPlan {
        schema_version: 10,
        identity: ZERO,
        distributed_cords: &exact_transports,
        ..plan
    };
    current.identity = current.semantic_hash(&mut scratch).unwrap();
    let current_context = PlanValidationContext {
        supported_schema_version: 10,
        ..context
    };
    assert_eq!(
        validate_execution_plan(&current, current_context, &mut scratch),
        Ok(())
    );

    let mut wrong_binding_revision = ExecutionPlan {
        identity: ZERO,
        distributed_cords: &distributed_cords,
        ..current
    };
    wrong_binding_revision.identity = wrong_binding_revision.semantic_hash(&mut scratch).unwrap();
    assert_eq!(
        validate_execution_plan(&wrong_binding_revision, current_context, &mut scratch)
            .unwrap_err()
            .code,
        PlanDiagnosticCode::Distributed(DistributedReason::UnsupportedVersion)
    );

    let mut missing = ExecutionPlan {
        identity: ZERO,
        distributed_cords: &[],
        ..plan
    };
    missing.identity = missing.semantic_hash(&mut scratch).unwrap();
    assert_eq!(
        validate_execution_plan(&missing, context, &mut scratch)
            .unwrap_err()
            .code,
        PlanDiagnosticCode::Distributed(DistributedReason::PeerMismatch)
    );
}
