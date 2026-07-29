mod support;

use std::net::TcpListener;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, CAPABILITY_REPORT_SCHEMA_VERSION,
    CapabilityReport, DistributedEvidenceKind, EventClass, EventStreamContract, ExecutorKind,
    FederationPolicy, GrantStatus, Id, ImplementationManifest, InstancePath, ManifestArtifactRef,
    ManifestEntrypoint, PassportStatus, PinnedDescriptor, PlanResourceBudget, RealmReason,
    ReplayDelivery, ReportCapability, ReproducibilityClaim, ResonanceError, ResumeProof,
    RetentionPolicy, SemanticHash, Sensitivity, SubscriberCoupling, TypeContractRef,
    validate_stream_contract, validate_transport_federation,
};
use conduit_runtime::{
    CandidateRejectionReason, CapabilityPredicate, CarrierSecurityMode, DistributedCordBackend,
    DistributedFrameKind, HostResolverPolicy, InMemoryDistributedCordBackend,
    InMemoryTransportFault, OutboundDistributedFrame, PlacementCandidate, PlacementRequest,
    ResolverTiePolicy, TransportReason, TransportTransition, decode_distributed_envelope,
    encode_distributed_envelope, resolve_host_placement, validate_transport_selection,
    validate_transport_transition,
};
use conduit_zenoh::{
    FIRMWARE_HOST_SERVICE_ADAPTER_ID, FIRMWARE_MESSAGE_ABI_ID, SecretFileHandle,
    ZENOH_LIVE_EVENT_PROVIDER_CAPABILITIES, ZENOH_PICO_IMPLEMENTATION_ID,
    ZENOH_TRANSPORT_CONTRACT_ID, ZenohBackendError, ZenohDistributedCordBackend, ZenohEndpointRole,
    ZenohTlsMaterial,
};
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use serde_json::{Value, json};
use support::{PLAN, authorities, binding, context, handshake, hash, placement, selection};

const FIXTURE: &str = include_str!("../../../conformance/c5/zenoh-transport-v1.json");

fn unused_local_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local port");
    listener.local_addr().expect("local address").port()
}

fn wait_for_remote_subscribers(
    first: &ZenohDistributedCordBackend,
    second: &ZenohDistributedCordBackend,
) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if first.has_remote_subscriber() && second.has_remote_subscriber() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Zenoh peers did not discover their remote subscribers");
}

fn receive_with_deadline(
    backend: &mut ZenohDistributedCordBackend,
    binding: &conduit_core::PlanDistributedCord<'_>,
    destination: &mut [u8],
) -> conduit_runtime::ReceivedDistributedFrame {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Some(frame) = backend
            .receive(binding, destination, authorities(*binding))
            .expect("receive from Zenoh")
        {
            return frame;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Zenoh frame was not received before the deadline");
}

struct TestCertificates {
    _directory: tempfile::TempDir,
    ca: std::path::PathBuf,
    server_certificate: std::path::PathBuf,
    server_key: std::path::PathBuf,
    client_certificate: std::path::PathBuf,
    client_key: std::path::PathBuf,
}

fn write_secret(path: &Path, value: &str) {
    std::fs::write(path, value).expect("write temporary certificate material");
}

fn test_certificates() -> TestCertificates {
    let directory = tempfile::tempdir().expect("temporary certificate directory");
    let mut ca_params = CertificateParams::new(Vec::new()).expect("CA parameters");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let ca_key = KeyPair::generate().expect("CA key");
    let ca_certificate = ca_params.self_signed(&ca_key).expect("CA certificate");
    let issuer = Issuer::new(ca_params, ca_key);

    let server_key_pair = KeyPair::generate().expect("server key");
    let mut server_params =
        CertificateParams::new(vec!["localhost".to_owned()]).expect("server parameters");
    server_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_certificate = server_params
        .signed_by(&server_key_pair, &issuer)
        .expect("server certificate");

    let client_key_pair = KeyPair::generate().expect("client key");
    let mut client_params =
        CertificateParams::new(vec!["conduit-client".to_owned()]).expect("client parameters");
    client_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_certificate = client_params
        .signed_by(&client_key_pair, &issuer)
        .expect("client certificate");

    let ca = directory.path().join("ca.pem");
    let server_certificate_path = directory.path().join("server.pem");
    let server_key = directory.path().join("server.key");
    let client_certificate_path = directory.path().join("client.pem");
    let client_key = directory.path().join("client.key");
    write_secret(&ca, &ca_certificate.pem());
    write_secret(&server_certificate_path, &server_certificate.pem());
    write_secret(&server_key, &server_key_pair.serialize_pem());
    write_secret(&client_certificate_path, &client_certificate.pem());
    write_secret(&client_key, &client_key_pair.serialize_pem());
    TestCertificates {
        _directory: directory,
        ca,
        server_certificate: server_certificate_path,
        server_key,
        client_certificate: client_certificate_path,
        client_key,
    }
}

fn tls_materials(
    certificates: &TestCertificates,
    mode: CarrierSecurityMode,
) -> (ZenohTlsMaterial, ZenohTlsMaterial) {
    let listener = ZenohTlsMaterial {
        root_ca: (mode == CarrierSecurityMode::MutualTls)
            .then(|| SecretFileHandle::new(certificates.ca.clone())),
        listen_private_key: Some(SecretFileHandle::new(certificates.server_key.clone())),
        listen_certificate: Some(SecretFileHandle::new(
            certificates.server_certificate.clone(),
        )),
        connect_private_key: None,
        connect_certificate: None,
    };
    let connector = ZenohTlsMaterial {
        root_ca: Some(SecretFileHandle::new(certificates.ca.clone())),
        listen_private_key: None,
        listen_certificate: None,
        connect_private_key: (mode == CarrierSecurityMode::MutualTls)
            .then(|| SecretFileHandle::new(certificates.client_key.clone())),
        connect_certificate: (mode == CarrierSecurityMode::MutualTls)
            .then(|| SecretFileHandle::new(certificates.client_certificate.clone())),
    };
    (listener, connector)
}

fn real_exchange(mode: CarrierSecurityMode) -> Value {
    let certificates = (mode != CarrierSecurityMode::Plaintext).then(test_certificates);
    let scheme = if mode == CarrierSecurityMode::Plaintext {
        "tcp"
    } else {
        "tls"
    };
    let endpoint = format!("{scheme}/localhost:{}", unused_local_port());
    let binding = binding(&endpoint, mode);
    let placement = placement(&binding);
    let selected = selection(&binding, mode);
    let (listener_tls, connector_tls) = certificates.as_ref().map_or_else(
        || (ZenohTlsMaterial::default(), ZenohTlsMaterial::default()),
        |certificates| tls_materials(certificates, mode),
    );
    let mut listener = ZenohDistributedCordBackend::prepare(
        &binding,
        &placement,
        selected,
        ZenohEndpointRole::Listen,
        listener_tls,
    )
    .expect("prepare protected listener");
    let mut connector = ZenohDistributedCordBackend::prepare(
        &binding,
        &placement,
        selected,
        ZenohEndpointRole::Connect,
        connector_tls,
    )
    .expect("prepare protected connector");
    listener
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open protected listener");
    connector
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open protected connector");
    wait_for_remote_subscribers(&listener, &connector);
    let _ = listener.take_evidence();
    let _ = connector.take_evidence();
    connector
        .send(
            &binding,
            OutboundDistributedFrame {
                kind: DistributedFrameKind::Value,
                session_epoch: 1,
                sequence: Some(1),
                attempt: None,
                correlation: Some(hash(92)),
                payload: mode.as_str().as_bytes(),
            },
            authorities(binding),
        )
        .expect("send protected frame");
    let mut destination = [0_u8; 64];
    let received = receive_with_deadline(&mut listener, &binding, &mut destination);
    assert_eq!(
        &destination[..received.payload_bytes],
        mode.as_str().as_bytes()
    );
    let send_evidence = connector.take_evidence().expect("send evidence");
    let receive_evidence = listener.take_evidence().expect("receive evidence");
    for evidence in [&send_evidence.common, &receive_evidence.common] {
        assert_eq!(evidence.carrier_security, mode);
        assert!(evidence.conduit_authority_checked);
    }
    assert_eq!(
        receive_evidence.common.carrier_authenticated,
        mode == CarrierSecurityMode::MutualTls
    );
    connector.shutdown().expect("close protected connector");
    listener.shutdown().expect("close protected listener");
    json!({
        "accepted": true,
        "security": mode.as_str(),
        "authenticated": send_evidence.common.carrier_authenticated,
        "mutual": send_evidence.common.carrier_mutually_authenticated,
        "encrypted": send_evidence.common.carrier_encrypted
    })
}

fn artifact_manifest(id: &'static str, digest: ArtifactDigest) -> ArtifactManifest<'static> {
    let mut manifest = ArtifactManifest {
        schema_version: 1,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id(id),
        digest,
        media_type: "application/x-executable",
        byte_size: 1_024,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("fixture/reproducible-builder"),
            source_digest: ArtifactDigest::from_bytes([71; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([72; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &["Apache-2.0"],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    };
    let mut scratch = [SemanticHash::from_bytes([0; 32]); 1];
    manifest.identity = manifest
        .computed_semantic_hash(&mut scratch)
        .expect("artifact identity");
    manifest
}

fn implementation_manifest<'a>(
    binding: &conduit_core::PlanDistributedCord<'a>,
    artifact: &'a ManifestArtifactRef<'a>,
) -> ImplementationManifest<'a> {
    let mut manifest = ImplementationManifest {
        schema_version: 1,
        identity: SemanticHash::from_bytes([0; 32]),
        id: binding.backend.id,
        implementation_version: "1.9.0",
        semantic_contract: PinnedDescriptor {
            id: Id(ZENOH_TRANSPORT_CONTRACT_ID),
            schema_version: 1,
            semantic_hash: hash(39),
        },
        executor: ExecutorKind::NativeInProcess,
        entrypoint: ManifestEntrypoint {
            name: Id("run"),
            adapter: Id("conduit/message-step-v1"),
            abi: Id("conduit/hosted-rust-v1"),
            protocol_version: 1,
        },
        execution_profile: binding.backend_profile.expect("backend profile"),
        artifacts: core::slice::from_ref(artifact),
        required_interfaces: &[],
        provided_interfaces: &[],
        required_authorities: &[],
        required_effects: &[],
        minimum_plan_version: 10,
        maximum_plan_version: 10,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        replacement: conduit_core::ReplacementSupport::Cold,
        coexistence_memory_bytes: 0,
        reproducibility: Some(ReproducibilityClaim {
            source_digest: ArtifactDigest::from_bytes([71; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([72; 32]),
            expected_artifact_digest: artifact.digest,
        }),
    };
    let mut scratch = [SemanticHash::from_bytes([0; 32]); 4];
    manifest.identity = manifest
        .computed_semantic_hash(&mut scratch)
        .expect("implementation identity");
    manifest
}

fn capability_report<'a>(
    id: &'a str,
    host: &'a str,
    available: PlanResourceBudget,
    capabilities: &'a [ReportCapability<'a>],
    executors: &'a [ExecutorKind],
    targets: &'a [Id<'a>],
    abis: &'a [Id<'a>],
) -> CapabilityReport<'a> {
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id(id),
        host: Id(host),
        reporter: support::pin("fixture/host-reporter", 73),
        trust: support::pin("fixture/report-trust", 74),
        membership: None,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 30,
        available,
        capabilities,
        resources: &[],
        topology: &[],
        supported_executors: executors,
        supported_targets: targets,
        supported_abis: abis,
        minimum_plan_version: 10,
        maximum_plan_version: 10,
        current_constraints: &[],
    };
    let mut scratch = [SemanticHash::from_bytes([0; 32]); 8];
    report.identity = report
        .computed_semantic_hash(&mut scratch)
        .expect("capability report identity");
    report
}

#[test]
fn plaintext_zenoh_exchanges_a_real_frame_and_matches_the_oracle_evidence() {
    let endpoint = format!("tcp/127.0.0.1:{}", unused_local_port());
    let binding = binding(&endpoint, CarrierSecurityMode::Plaintext);
    let placement = placement(&binding);
    let selected = selection(&binding, CarrierSecurityMode::Plaintext);
    let mut listener = ZenohDistributedCordBackend::prepare(
        &binding,
        &placement,
        selected,
        ZenohEndpointRole::Listen,
        ZenohTlsMaterial::default(),
    )
    .expect("prepare listener");
    let mut connector = ZenohDistributedCordBackend::prepare(
        &binding,
        &placement,
        selected,
        ZenohEndpointRole::Connect,
        ZenohTlsMaterial::default(),
    )
    .expect("prepare connector");

    listener
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open listener");
    connector
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open connector");
    wait_for_remote_subscribers(&listener, &connector);
    assert_eq!(
        listener.take_evidence().unwrap().common.kind,
        DistributedEvidenceKind::HandshakeAccepted
    );
    assert_eq!(
        connector.take_evidence().unwrap().common.kind,
        DistributedEvidenceKind::HandshakeAccepted
    );

    let correlation = hash(91);
    let outbound = OutboundDistributedFrame {
        kind: DistributedFrameKind::Value,
        session_epoch: 1,
        sequence: Some(1),
        attempt: Some(0),
        correlation: Some(correlation),
        payload: b"live-zenoh",
    };
    connector
        .send(&binding, outbound, authorities(binding))
        .expect("publish actual Zenoh frame");
    let mut destination = [0_u8; 64];
    let received = receive_with_deadline(&mut listener, &binding, &mut destination);
    assert_eq!(received.kind, DistributedFrameKind::Value);
    assert_eq!(received.sequence, Some(1));
    assert_eq!(received.attempt, Some(0));
    assert_eq!(received.correlation, Some(correlation));
    assert_eq!(&destination[..received.payload_bytes], b"live-zenoh");

    let sent = connector.take_evidence().expect("Zenoh send evidence");
    let received_evidence = listener.take_evidence().expect("Zenoh receive evidence");
    assert_eq!(sent.common.kind, DistributedEvidenceKind::ValueSent);
    assert_eq!(
        received_evidence.common.kind,
        DistributedEvidenceKind::ValueReceived
    );
    for evidence in [&sent.common, &received_evidence.common] {
        assert_eq!(evidence.carrier_security, CarrierSecurityMode::Plaintext);
        assert!(!evidence.carrier_authenticated);
        assert!(!evidence.carrier_mutually_authenticated);
        assert!(!evidence.carrier_encrypted);
        assert!(evidence.conduit_authority_checked);
    }

    let mut oracle = InMemoryDistributedCordBackend::new();
    oracle
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open deterministic oracle");
    let _ = oracle.take_evidence();
    oracle
        .send(&binding, outbound, authorities(binding))
        .expect("send through deterministic oracle");
    let mut oracle_destination = [0_u8; 64];
    let oracle_received = oracle
        .receive(&binding, &mut oracle_destination, authorities(binding))
        .expect("oracle receive")
        .expect("oracle frame");
    let oracle_sent = oracle.take_evidence().expect("oracle send evidence");
    let oracle_received_evidence = oracle.take_evidence().expect("oracle receive evidence");
    assert_eq!(oracle_received, received);
    assert_eq!(
        (
            oracle_sent.kind,
            oracle_sent.sequence,
            oracle_sent.attempt,
            oracle_sent.correlation,
        ),
        (
            sent.common.kind,
            sent.common.sequence,
            sent.common.attempt,
            sent.common.correlation,
        )
    );
    assert_eq!(
        (
            oracle_received_evidence.kind,
            oracle_received_evidence.sequence,
            oracle_received_evidence.attempt,
            oracle_received_evidence.correlation,
        ),
        (
            received_evidence.common.kind,
            received_evidence.common.sequence,
            received_evidence.common.attempt,
            received_evidence.common.correlation,
        )
    );

    connector.shutdown().expect("close connector");
    listener.shutdown().expect("close listener");
}

#[test]
fn envelope_codec_rejects_wrong_identity_malformed_and_oversized_frames() {
    let binding = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext);
    let outbound = OutboundDistributedFrame {
        kind: DistributedFrameKind::Value,
        session_epoch: 1,
        sequence: Some(7),
        attempt: None,
        correlation: None,
        payload: b"bounded",
    };
    let mut bytes = [0_u8; 512];
    let used =
        encode_distributed_envelope(PLAN, &binding, outbound, &mut bytes).expect("encode frame");
    let decoded =
        decode_distributed_envelope(&bytes[..used], PLAN, &binding).expect("decode frame");
    assert_eq!(decoded.frame, outbound);
    assert_eq!(
        decode_distributed_envelope(&bytes[..used], hash(99), &binding),
        Err(TransportReason::EnvelopeIdentityMismatch)
    );
    bytes[0] = b'X';
    assert_eq!(
        decode_distributed_envelope(&bytes[..used], PLAN, &binding),
        Err(TransportReason::EnvelopeMalformed)
    );
    let oversized = OutboundDistributedFrame {
        payload: &[0_u8; 65],
        ..outbound
    };
    assert_eq!(
        encode_distributed_envelope(PLAN, &binding, oversized, &mut bytes),
        Err(TransportReason::EnvelopeTooLarge)
    );
}

#[test]
fn exact_transport_selection_rejects_artifact_profile_security_and_budget_drift() {
    let endpoint = "tcp/127.0.0.1:7447";
    let binding = binding(endpoint, CarrierSecurityMode::Plaintext);
    let placement = placement(&binding);
    let selected = selection(&binding, CarrierSecurityMode::Plaintext);
    validate_transport_selection(&binding, &placement, selected).expect("exact selection");
    assert!(selected.capabilities.publish_subscribe);
    assert!(!selected.capabilities.query_reply);
    assert!(!selected.capabilities.complete_stack_hard_bounded);

    let mut artifact = selected;
    artifact.artifact.digest = conduit_core::ArtifactDigest::from_bytes([99; 32]);
    assert_eq!(
        validate_transport_selection(&binding, &placement, artifact),
        Err(TransportReason::ArtifactMismatch)
    );
    let mut profile = selected;
    profile.execution_profile.semantic_hash = SemanticHash::from_bytes([99; 32]);
    assert_eq!(
        validate_transport_selection(&binding, &placement, profile),
        Err(TransportReason::ProfileMismatch)
    );
    let mut security = selected;
    security.security_mode = CarrierSecurityMode::MutualTls;
    security.capabilities.security.mutual_tls = false;
    assert_eq!(
        validate_transport_selection(&binding, &placement, security),
        Err(TransportReason::UnsupportedSecurity)
    );
    let mut budget = selected;
    budget.capabilities.session_state_bytes = binding.allocation.memory_bytes + 1;
    assert_eq!(
        validate_transport_selection(&binding, &placement, budget),
        Err(TransportReason::ResourceUnderaccounted)
    );
}

#[test]
fn transport_replacement_requires_a_new_epoch_and_never_weakens_security() {
    let current = binding("tls/host-a.example:7447", CarrierSecurityMode::MutualTls);
    assert_eq!(
        validate_transport_transition(
            &current,
            CarrierSecurityMode::MutualTls,
            &current,
            CarrierSecurityMode::MutualTls,
        ),
        Ok(TransportTransition::Unchanged)
    );

    let mut replacement = current;
    replacement.carrier_endpoint = Some("tls/host-b.example:7447");
    replacement.initial_session_epoch = 2;
    replacement.identity = replacement.semantic_hash().expect("replacement identity");
    assert_eq!(
        validate_transport_transition(
            &current,
            CarrierSecurityMode::MutualTls,
            &replacement,
            CarrierSecurityMode::MutualTls,
        ),
        Ok(TransportTransition::NewSessionEpoch)
    );

    let mut same_epoch = replacement;
    same_epoch.initial_session_epoch = current.initial_session_epoch;
    same_epoch.identity = same_epoch.semantic_hash().expect("same-epoch identity");
    assert_eq!(
        validate_transport_transition(
            &current,
            CarrierSecurityMode::MutualTls,
            &same_epoch,
            CarrierSecurityMode::MutualTls,
        ),
        Err(TransportReason::BindingMismatch)
    );
    assert_eq!(
        validate_transport_transition(
            &current,
            CarrierSecurityMode::MutualTls,
            &replacement,
            CarrierSecurityMode::Tls,
        ),
        Err(TransportReason::UnsupportedSecurity)
    );
}

#[test]
fn host_resolver_selects_the_exact_zenoh_implementation_artifact_and_profile() {
    let endpoint = "tcp/127.0.0.1:7447";
    let mut binding = binding(endpoint, CarrierSecurityMode::Plaintext);
    let planned_artifact = binding.backend_artifact.expect("planned artifact");
    let artifact = artifact_manifest(planned_artifact.id.as_str(), planned_artifact.digest);
    let artifact_reference = ManifestArtifactRef {
        id: planned_artifact.id,
        digest: planned_artifact.digest,
        role: Id("executable"),
        required: true,
    };
    let implementation = implementation_manifest(&binding, &artifact_reference);
    binding.backend.semantic_hash = implementation.identity;
    binding.identity = binding.semantic_hash().expect("updated binding identity");

    let interface = support::pin("conduit/host.zenoh-transport", 75);
    let observed_capability = ReportCapability {
        interface,
        mode: Id("hosted"),
        subject: Id("fixture/loopback"),
        details: hash(76),
        capacity: binding.allocation,
    };
    let report = capability_report(
        "fixture/linux-zenoh-report",
        "fixture/linux-host",
        PlanResourceBudget {
            memory_bytes: binding.allocation.memory_bytes * 2,
            storage_bytes: 0,
            cpu_units: 2,
            timers: 8,
            transports: 2,
            checkpoints: 0,
            evidence_bytes: binding.allocation.evidence_bytes * 2,
        },
        core::slice::from_ref(&observed_capability),
        &[ExecutorKind::NativeInProcess],
        &[],
        &[],
    );
    let requirement = CapabilityPredicate {
        interface,
        mode: Id("hosted"),
        subject: Some(Id("fixture/loopback")),
        details: Some(hash(76)),
        minimum_capacity: binding.allocation,
        satisfaction_proof: None,
    };
    let artifacts = [&artifact];
    let candidate = PlacementCandidate {
        manifest: &implementation,
        artifacts: &artifacts,
        report: &report,
        allocation: binding.allocation,
        capabilities: core::slice::from_ref(&requirement),
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let candidates = [candidate];
    let request = PlacementRequest {
        instance: InstancePath::new("root/transport").expect("instance path"),
        semantic_contract: implementation.semantic_contract,
        candidates: &candidates,
    };
    let mut policy = HostResolverPolicy {
        resolver: support::pin("fixture/host-resolver", 77),
        policy_hash: SemanticHash::from_bytes([0; 32]),
        time_basis: Id("fixture/clock"),
        current_tick: 20,
        plan_version: 10,
        trusted_reporters: &[report.reporter],
        trusted_report_trust: &[report.trust.semantic_hash],
        required_realm: None,
        trusted_entities: &[],
        trusted_status_reporters: &[],
        require_active_passport: false,
        allowed_implementations: &[binding.backend.id],
        implementation_preference: &[binding.backend.id],
        tie_policy: ResolverTiePolicy::RejectAmbiguous,
        maximum_search_states: 4,
    };
    policy.policy_hash = policy.computed_semantic_hash().expect("resolver policy");
    let resolved =
        resolve_host_placement(core::slice::from_ref(&request), policy).expect("resolve host");
    assert_eq!(resolved.bindings.len(), 1);
    let placement = &resolved.bindings[0];
    assert_eq!(placement.implementation_id, binding.backend.id.as_str());
    assert_eq!(
        placement.implementation_identity,
        binding.backend.semantic_hash
    );
    assert_eq!(
        placement.artifacts,
        vec![(planned_artifact.id.to_string(), planned_artifact.digest)]
    );
    assert_eq!(
        placement.capability_subjects,
        vec!["fixture/loopback".to_owned()]
    );
    validate_transport_selection(
        &binding,
        placement,
        selection(&binding, CarrierSecurityMode::Plaintext),
    )
    .expect("resolved placement agrees with exact transport binding");
}

#[test]
fn linux_and_pico_manifests_share_the_transport_contract_through_distinct_boundaries() {
    let endpoint = "tcp/127.0.0.1:7447";
    let binding = binding(endpoint, CarrierSecurityMode::Plaintext);
    let hosted_artifact_ref = ManifestArtifactRef {
        id: Id("artifact/zenoh-rust-1-9-0"),
        digest: ArtifactDigest::from_bytes([44; 32]),
        role: Id("executable"),
        required: true,
    };
    let hosted_artifact =
        artifact_manifest(hosted_artifact_ref.id.as_str(), hosted_artifact_ref.digest);
    let hosted = implementation_manifest(&binding, &hosted_artifact_ref);

    let target = Id("thumbv6m-none-eabi");
    let firmware_abi = Id(FIRMWARE_MESSAGE_ABI_ID);
    let firmware_artifact_ref = ManifestArtifactRef {
        id: Id("artifact/zenoh-pico-firmware"),
        digest: ArtifactDigest::from_bytes([81; 32]),
        role: Id("firmware"),
        required: true,
    };
    let mut firmware_artifact = artifact_manifest(
        firmware_artifact_ref.id.as_str(),
        firmware_artifact_ref.digest,
    );
    firmware_artifact.target = Some(target);
    firmware_artifact.abi = Some(firmware_abi);
    let mut artifact_scratch = [SemanticHash::from_bytes([0; 32]); 1];
    firmware_artifact.identity = firmware_artifact
        .computed_semantic_hash(&mut artifact_scratch)
        .expect("firmware artifact identity");
    let mut firmware = ImplementationManifest {
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id(ZENOH_PICO_IMPLEMENTATION_ID),
        implementation_version: "zenoh-pico",
        executor: ExecutorKind::Firmware,
        entrypoint: ManifestEntrypoint {
            name: Id("transport"),
            adapter: Id(FIRMWARE_HOST_SERVICE_ADAPTER_ID),
            abi: firmware_abi,
            protocol_version: 1,
        },
        execution_profile: support::pin("conduit/zenoh-pico-static", 82),
        artifacts: core::slice::from_ref(&firmware_artifact_ref),
        reproducibility: Some(ReproducibilityClaim {
            source_digest: ArtifactDigest::from_bytes([71; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([72; 32]),
            expected_artifact_digest: firmware_artifact_ref.digest,
        }),
        ..hosted
    };
    let mut implementation_scratch = [SemanticHash::from_bytes([0; 32]); 4];
    firmware.identity = firmware
        .computed_semantic_hash(&mut implementation_scratch)
        .expect("firmware implementation identity");
    assert_eq!(hosted.semantic_contract, firmware.semantic_contract);
    assert_ne!(hosted.identity, firmware.identity);
    assert_eq!(firmware.executor, ExecutorKind::Firmware);
    assert_eq!(
        firmware.entrypoint.adapter,
        Id(FIRMWARE_HOST_SERVICE_ADAPTER_ID)
    );
    assert_eq!(firmware.entrypoint.abi, Id(FIRMWARE_MESSAGE_ABI_ID));

    let interface = support::pin("conduit/host.zenoh-transport", 75);
    let hosted_capability = ReportCapability {
        interface,
        mode: Id("hosted"),
        subject: Id("fixture/loopback"),
        details: hash(76),
        capacity: binding.allocation,
    };
    let firmware_capability = ReportCapability {
        interface,
        mode: Id("firmware"),
        subject: Id("fixture/cyw43"),
        details: hash(83),
        capacity: binding.allocation,
    };
    let hosted_executors = [ExecutorKind::NativeInProcess];
    let firmware_executors = [ExecutorKind::Firmware];
    let firmware_targets = [target];
    let firmware_abis = [firmware_abi];
    let hosted_report = capability_report(
        "fixture/linux-zenoh-report",
        "fixture/linux-host",
        binding.allocation,
        core::slice::from_ref(&hosted_capability),
        &hosted_executors,
        &[],
        &[],
    );
    let firmware_report = capability_report(
        "fixture/pico-zenoh-report",
        "fixture/pico-host",
        binding.allocation,
        core::slice::from_ref(&firmware_capability),
        &firmware_executors,
        &firmware_targets,
        &firmware_abis,
    );
    let hosted_requirement = CapabilityPredicate {
        interface,
        mode: Id("hosted"),
        subject: Some(Id("fixture/loopback")),
        details: Some(hash(76)),
        minimum_capacity: binding.allocation,
        satisfaction_proof: None,
    };
    let firmware_requirement = CapabilityPredicate {
        interface,
        mode: Id("firmware"),
        subject: Some(Id("fixture/cyw43")),
        details: Some(hash(83)),
        minimum_capacity: binding.allocation,
        satisfaction_proof: None,
    };
    let hosted_artifacts = [&hosted_artifact];
    let firmware_artifacts = [&firmware_artifact];
    let hosted_candidate = PlacementCandidate {
        manifest: &hosted,
        artifacts: &hosted_artifacts,
        report: &hosted_report,
        allocation: binding.allocation,
        capabilities: core::slice::from_ref(&hosted_requirement),
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let firmware_candidate = PlacementCandidate {
        manifest: &firmware,
        artifacts: &firmware_artifacts,
        report: &firmware_report,
        allocation: binding.allocation,
        capabilities: core::slice::from_ref(&firmware_requirement),
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let trusted_reporters = [hosted_report.reporter];
    let trusted_report_trust = [hosted_report.trust.semantic_hash];
    let mut policy = HostResolverPolicy {
        resolver: support::pin("fixture/host-resolver", 77),
        policy_hash: SemanticHash::from_bytes([0; 32]),
        time_basis: Id("fixture/clock"),
        current_tick: 20,
        plan_version: 10,
        trusted_reporters: &trusted_reporters,
        trusted_report_trust: &trusted_report_trust,
        required_realm: None,
        trusted_entities: &[],
        trusted_status_reporters: &[],
        require_active_passport: false,
        allowed_implementations: &[],
        implementation_preference: &[],
        tie_policy: ResolverTiePolicy::RejectAmbiguous,
        maximum_search_states: 4,
    };
    policy.policy_hash = policy.computed_semantic_hash().expect("policy identity");
    for (instance, candidate, expected) in [
        ("root/linux-transport", hosted_candidate, hosted.id),
        ("root/pico-transport", firmware_candidate, firmware.id),
    ] {
        let candidates = [candidate];
        let request = PlacementRequest {
            instance: InstancePath::new(instance).expect("instance path"),
            semantic_contract: hosted.semantic_contract,
            candidates: &candidates,
        };
        let resolved =
            resolve_host_placement(core::slice::from_ref(&request), policy).expect("resolve");
        assert_eq!(resolved.bindings[0].implementation_id, expected.as_str());
    }

    let unsupported_report = capability_report(
        "fixture/pico-without-zenoh-report",
        "fixture/pico-host",
        binding.allocation,
        &[],
        &firmware_executors,
        &firmware_targets,
        &firmware_abis,
    );
    let unsupported_candidate = PlacementCandidate {
        report: &unsupported_report,
        ..firmware_candidate
    };
    let unsupported_candidates = [unsupported_candidate];
    let unsupported_request = PlacementRequest {
        instance: InstancePath::new("root/unsupported-pico").expect("instance path"),
        semantic_contract: hosted.semantic_contract,
        candidates: &unsupported_candidates,
    };
    let failure =
        resolve_host_placement(core::slice::from_ref(&unsupported_request), policy).unwrap_err();
    assert!(
        failure.candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::CapabilityMissing)
    );
}

#[test]
fn tls_modes_fail_closed_when_secret_handles_are_absent() {
    for mode in [CarrierSecurityMode::Tls, CarrierSecurityMode::MutualTls] {
        let endpoint = format!("tls/localhost:{}", unused_local_port());
        let binding = binding(&endpoint, mode);
        let placement = placement(&binding);
        let selected = selection(&binding, mode);
        for role in [ZenohEndpointRole::Listen, ZenohEndpointRole::Connect] {
            let result = ZenohDistributedCordBackend::prepare(
                &binding,
                &placement,
                selected,
                role,
                ZenohTlsMaterial::default(),
            );
            assert!(matches!(
                result,
                Err(ZenohBackendError::Transport(
                    TransportReason::SecretHandleMissing
                ))
            ));
        }
    }
}

#[test]
fn tls_and_mutual_tls_exchange_real_frames_without_downgrade() {
    for mode in [CarrierSecurityMode::Tls, CarrierSecurityMode::MutualTls] {
        assert_eq!(
            real_exchange(mode),
            json!({
                "accepted": true,
                "security": mode.as_str(),
                "authenticated": true,
                "mutual": mode == CarrierSecurityMode::MutualTls,
                "encrypted": true
            })
        );
    }
}

fn opened_oracle() -> (
    conduit_core::PlanDistributedCord<'static>,
    InMemoryDistributedCordBackend,
) {
    let binding = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext);
    let mut backend = InMemoryDistributedCordBackend::new();
    backend
        .open(
            &binding,
            handshake(binding),
            context(),
            authorities(binding),
        )
        .expect("open oracle");
    let _ = backend.take_evidence();
    (binding, backend)
}

fn value_frame(sequence: u64, payload: &'static [u8]) -> OutboundDistributedFrame<'static> {
    OutboundDistributedFrame {
        kind: DistributedFrameKind::Value,
        session_epoch: 1,
        sequence: Some(sequence),
        attempt: None,
        correlation: Some(hash(100)),
        payload,
    }
}

fn fixture_rejection(reason: TransportReason) -> Value {
    json!({"accepted": false, "code": reason.code()})
}

fn dummy_mtls_material() -> ZenohTlsMaterial {
    ZenohTlsMaterial {
        root_ca: Some(SecretFileHandle::new("/nonexistent/conduit-ca.pem")),
        listen_private_key: Some(SecretFileHandle::new("/nonexistent/conduit-server.key")),
        listen_certificate: Some(SecretFileHandle::new("/nonexistent/conduit-server.pem")),
        connect_private_key: None,
        connect_certificate: None,
    }
}

fn realm_transport_fixture(case: &str) -> Value {
    let endpoint = format!("tls/localhost:{}", unused_local_port());
    let mtls_binding = binding(&endpoint, CarrierSecurityMode::MutualTls);
    match case {
        "valid-mtls-wrong-realm" | "valid-realm-missing-grant" | "cloned-identity-conflict" => {
            let mut backend = ZenohDistributedCordBackend::prepare(
                &mtls_binding,
                &placement(&mtls_binding),
                selection(&mtls_binding, CarrierSecurityMode::MutualTls),
                ZenohEndpointRole::Listen,
                dummy_mtls_material(),
            )
            .unwrap();
            let mut offered = handshake(mtls_binding);
            let mut authority = authorities(mtls_binding);
            match case {
                "valid-mtls-wrong-realm" => {
                    offered.writer.status.realm = Id("fixture/wrong-realm");
                }
                "valid-realm-missing-grant" => {
                    authority.writer_grant.status = GrantStatus::Revoked {
                        at_tick: 20,
                        reason: Id("fixture/revoked"),
                    };
                }
                "cloned-identity-conflict" => offered.reader = offered.writer,
                _ => unreachable!(),
            }
            match backend.open(&mtls_binding, offered, context(), authority) {
                Err(reason) => json!({"accepted": false, "code": reason.code()}),
                Ok(()) => panic!("invalid realm/passport admission was accepted"),
            }
        }
        "key-rotation-during-reconnect" | "revocation-while-partitioned" => {
            let endpoint = format!("tcp/127.0.0.1:{}", unused_local_port());
            let binding = binding(&endpoint, CarrierSecurityMode::Plaintext);
            let mut backend = ZenohDistributedCordBackend::prepare(
                &binding,
                &placement(&binding),
                selection(&binding, CarrierSecurityMode::Plaintext),
                ZenohEndpointRole::Listen,
                ZenohTlsMaterial::default(),
            )
            .unwrap();
            backend
                .open(
                    &binding,
                    handshake(binding),
                    context(),
                    authorities(binding),
                )
                .unwrap();
            let mut offered = handshake(binding);
            if case == "key-rotation-during-reconnect" {
                offered.writer.key = Id("fixture/rotated-writer-key");
                offered.writer.key_epoch += 1;
            } else {
                offered.writer.status.status = PassportStatus::Revoked;
            }
            let result = backend.reauthenticate(
                &binding,
                offered,
                context(),
                Some(ResumeProof {
                    plan_identity: PLAN,
                    binding_identity: binding.identity,
                    session_epoch: 1,
                    writer_next_sequence: 0,
                    reader_next_sequence: 0,
                    acknowledged_through: None,
                    receipt: hash(103),
                }),
                authorities(binding),
            );
            backend.shutdown().unwrap();
            match result {
                Err(reason) => json!({"accepted": false, "code": reason.code()}),
                Ok(()) => panic!("stale reconnect credential was accepted"),
            }
        }
        "realm-root-rollover-needs-new-binding" => {
            let mut backend = ZenohDistributedCordBackend::prepare(
                &mtls_binding,
                &placement(&mtls_binding),
                selection(&mtls_binding, CarrierSecurityMode::MutualTls),
                ZenohEndpointRole::Listen,
                dummy_mtls_material(),
            )
            .unwrap();
            let mut rolled = mtls_binding;
            rolled.writer.realm_identity = hash(104);
            rolled.identity = rolled.semantic_hash().unwrap();
            match backend.open(&rolled, handshake(rolled), context(), authorities(rolled)) {
                Err(reason) => json!({"accepted": false, "code": reason.code()}),
                Ok(()) => panic!("rolled realm root reused an old exact binding"),
            }
        }
        "explicit-cross-realm-scope" | "federation-is-non-transitive" => {
            let transport = support::pin("conduit/distributed-cord-transport", 105);
            let allowed = [transport];
            let policy = FederationPolicy {
                id: Id("fixture/a-to-b"),
                local_realm: Id("fixture/realm-a"),
                remote_realm: Id("fixture/realm-b"),
                local_root_epoch: 1,
                remote_root_epoch: 1,
                time_basis: Id("fixture/clock"),
                expires_at_tick: 30,
                allow_identity: true,
                allow_event_verification: true,
                allow_transport_admission: true,
                allow_grant_delegation: false,
                allowed_streams: &allowed,
                receipt: hash(106),
            };
            let remote = if case == "federation-is-non-transitive" {
                Id("fixture/realm-c")
            } else {
                Id("fixture/realm-b")
            };
            match validate_transport_federation(
                policy,
                Id("fixture/realm-a"),
                remote,
                transport,
                Id("fixture/clock"),
                20,
                false,
            ) {
                Ok(()) => json!({"accepted": true}),
                Err(RealmReason::FederationDenied) => {
                    json!({"accepted": false, "code": "CND-RLM-007"})
                }
                Err(reason) => panic!("unexpected realm reason: {}", reason.code()),
            }
        }
        other => panic!("unknown realm transport fixture `{other}`"),
    }
}

fn zenoh_live_event_contract() -> EventStreamContract<'static> {
    let flow = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext).flow;
    EventStreamContract {
        id: Id("fixture/zenoh-event-stream"),
        event_class: EventClass::Domain,
        payload_type: TypeContractRef {
            contract_id: Id("fixture/event-value"),
            schema_version: 1,
            semantic_hash: hash(107),
        },
        retention: RetentionPolicy::Ring {
            maximum_events: 4,
            maximum_bytes: 256,
        },
        subscriber_coupling: SubscriberCoupling::Isolated(flow),
        delivery: ReplayDelivery::AtLeastOnce,
        maximum_publishers: 1,
        maximum_subscribers: 1,
        maximum_pending_operations: 1,
        maximum_projection_bytes: 64,
        provider: support::pin("conduit/zenoh-live-profile", 108),
        recording_authority: None,
        sensitivity: Sensitivity::Public,
        terminal_evidence_required: false,
    }
}

fn resonance_separation_fixture(case: &str) -> Value {
    let binding = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext);
    let cord = validate_transport_selection(
        &binding,
        &placement(&binding),
        selection(&binding, CarrierSecurityMode::Plaintext),
    )
    .is_ok();
    let event = validate_stream_contract(
        zenoh_live_event_contract(),
        ZENOH_LIVE_EVENT_PROVIDER_CAPABILITIES,
    );
    match case {
        "zenoh-live-cord-only" => json!({"cord": cord, "event_provider": false}),
        "zenoh-event-stream-only-rejected" => {
            let reason = event.unwrap_err();
            assert_eq!(reason, ResonanceError::ProviderIncapable);
            json!({"accepted": false, "code": reason.code()})
        }
        "zenoh-combined-contracts-stay-distinct" => {
            json!({"cord": cord, "event": event.is_ok()})
        }
        other => panic!("unknown Resonance separation fixture `{other}`"),
    }
}

fn execute_fixture(case: &str) -> Value {
    match case {
        "zenoh-live-cord-only"
        | "zenoh-event-stream-only-rejected"
        | "zenoh-combined-contracts-stay-distinct" => resonance_separation_fixture(case),
        "valid-mtls-wrong-realm"
        | "valid-realm-missing-grant"
        | "key-rotation-during-reconnect"
        | "revocation-while-partitioned"
        | "realm-root-rollover-needs-new-binding"
        | "cloned-identity-conflict"
        | "explicit-cross-realm-scope"
        | "federation-is-non-transitive" => realm_transport_fixture(case),
        "real-plaintext-exchange" => real_exchange(CarrierSecurityMode::Plaintext),
        "real-tls-exchange" => real_exchange(CarrierSecurityMode::Tls),
        "real-mtls-exchange" => real_exchange(CarrierSecurityMode::MutualTls),
        "tls-secret-handle-required" | "mtls-secret-handle-required" => {
            let mode = if case.starts_with("mtls") {
                CarrierSecurityMode::MutualTls
            } else {
                CarrierSecurityMode::Tls
            };
            let endpoint = format!("tls/localhost:{}", unused_local_port());
            let binding = binding(&endpoint, mode);
            let result = ZenohDistributedCordBackend::prepare(
                &binding,
                &placement(&binding),
                selection(&binding, mode),
                ZenohEndpointRole::Connect,
                ZenohTlsMaterial::default(),
            );
            match result {
                Err(ZenohBackendError::Transport(reason)) => fixture_rejection(reason),
                Err(ZenohBackendError::Distributed(reason)) => {
                    panic!("unexpected distributed reason: {}", reason.code())
                }
                Ok(_) => panic!("missing secret handles were accepted"),
            }
        }
        "exact-selection"
        | "query-reply-overclaim"
        | "artifact-drift"
        | "profile-drift"
        | "endpoint-drift"
        | "unsupported-security"
        | "resource-underaccounted" => {
            let binding = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext);
            let placement = placement(&binding);
            let mut selected = selection(&binding, CarrierSecurityMode::Plaintext);
            match case {
                "query-reply-overclaim" => {
                    selected.capabilities.query_reply = true;
                    return match ZenohDistributedCordBackend::prepare(
                        &binding,
                        &placement,
                        selected,
                        ZenohEndpointRole::Connect,
                        ZenohTlsMaterial::default(),
                    ) {
                        Err(ZenohBackendError::Transport(reason)) => fixture_rejection(reason),
                        Err(ZenohBackendError::Distributed(reason)) => {
                            panic!("unexpected distributed reason: {}", reason.code())
                        }
                        Ok(_) => panic!("unsupported query/reply capability was accepted"),
                    };
                }
                "artifact-drift" => {
                    selected.artifact.digest = ArtifactDigest::from_bytes([99; 32]);
                }
                "profile-drift" => {
                    selected.execution_profile.semantic_hash = hash(99);
                }
                "endpoint-drift" => selected.endpoint = "tcp/127.0.0.1:7555",
                "unsupported-security" => {
                    selected.security_mode = CarrierSecurityMode::MutualTls;
                    selected.capabilities.security.mutual_tls = false;
                }
                "resource-underaccounted" => {
                    selected.capabilities.session_state_bytes = binding.allocation.memory_bytes + 1;
                }
                "exact-selection" => {}
                _ => unreachable!(),
            }
            match validate_transport_selection(&binding, &placement, selected) {
                Ok(()) => json!({"accepted": true}),
                Err(reason) => fixture_rejection(reason),
            }
        }
        "envelope-round-trip"
        | "envelope-wrong-identity"
        | "envelope-malformed"
        | "envelope-oversized" => {
            let binding = binding("tcp/127.0.0.1:7447", CarrierSecurityMode::Plaintext);
            let frame = value_frame(7, b"bounded");
            let mut bytes = [0_u8; 512];
            if case == "envelope-oversized" {
                let oversized = OutboundDistributedFrame {
                    payload: &[0_u8; 65],
                    ..frame
                };
                return fixture_rejection(
                    encode_distributed_envelope(PLAN, &binding, oversized, &mut bytes).unwrap_err(),
                );
            }
            let used =
                encode_distributed_envelope(PLAN, &binding, frame, &mut bytes).expect("encode");
            if case == "envelope-malformed" {
                bytes[0] = b'X';
            }
            let expected_plan = if case == "envelope-wrong-identity" {
                hash(99)
            } else {
                PLAN
            };
            match decode_distributed_envelope(&bytes[..used], expected_plan, &binding) {
                Ok(decoded) => json!({
                    "accepted": true,
                    "sequence": decoded.frame.sequence
                }),
                Err(reason) => fixture_rejection(reason),
            }
        }
        "duplicate-next-value"
        | "reorder-next-pair"
        | "lost-acknowledgement"
        | "lost-terminal-acknowledgement"
        | "partition-is-explicit"
        | "reconnect-with-proof"
        | "cancellation-frame"
        | "oracle-oversized-frame" => {
            let (binding, mut backend) = opened_oracle();
            match case {
                "duplicate-next-value" => {
                    backend
                        .inject_fault(InMemoryTransportFault::DuplicateNextValue)
                        .unwrap();
                    backend
                        .send(&binding, value_frame(0, b"value"), authorities(binding))
                        .unwrap();
                    json!({"queued": backend.queued_items()})
                }
                "reorder-next-pair" => {
                    backend
                        .inject_fault(InMemoryTransportFault::ReorderNextValuePair)
                        .unwrap();
                    backend
                        .send(&binding, value_frame(0, b"zero"), authorities(binding))
                        .unwrap();
                    backend
                        .send(&binding, value_frame(1, b"one"), authorities(binding))
                        .unwrap();
                    let mut bytes = [0_u8; 64];
                    let first = backend
                        .receive(&binding, &mut bytes, authorities(binding))
                        .unwrap()
                        .unwrap();
                    let second = backend
                        .receive(&binding, &mut bytes, authorities(binding))
                        .unwrap()
                        .unwrap();
                    json!({"sequences": [first.sequence, second.sequence]})
                }
                "lost-acknowledgement" | "lost-terminal-acknowledgement" => {
                    let terminal = case.starts_with("lost-terminal");
                    backend
                        .inject_fault(if terminal {
                            InMemoryTransportFault::DropNextTerminalAcknowledgement
                        } else {
                            InMemoryTransportFault::DropNextAcknowledgement
                        })
                        .unwrap();
                    backend
                        .send(
                            &binding,
                            OutboundDistributedFrame {
                                kind: if terminal {
                                    DistributedFrameKind::TerminalAcknowledgement
                                } else {
                                    DistributedFrameKind::Acknowledgement
                                },
                                session_epoch: 1,
                                sequence: Some(1),
                                attempt: None,
                                correlation: None,
                                payload: &[],
                            },
                            authorities(binding),
                        )
                        .unwrap();
                    assert_eq!(
                        backend.take_evidence().unwrap().kind,
                        DistributedEvidenceKind::FrameDropped
                    );
                    json!({
                        "queued": backend.queued_items(),
                        "evidence": "frame-dropped"
                    })
                }
                "partition-is-explicit" => {
                    backend.set_partitioned(true);
                    let reason = backend
                        .send(&binding, value_frame(0, b"value"), authorities(binding))
                        .unwrap_err();
                    json!({"accepted": false, "code": reason.code()})
                }
                "reconnect-with-proof" => {
                    backend.set_partitioned(true);
                    backend
                        .reauthenticate(
                            &binding,
                            handshake(binding),
                            context(),
                            Some(ResumeProof {
                                plan_identity: PLAN,
                                binding_identity: binding.identity,
                                session_epoch: 1,
                                writer_next_sequence: 0,
                                reader_next_sequence: 0,
                                acknowledged_through: None,
                                receipt: hash(101),
                            }),
                            authorities(binding),
                        )
                        .unwrap();
                    json!({"accepted": true})
                }
                "cancellation-frame" => {
                    backend
                        .cancel(&binding, 1, 1, Some(hash(102)), authorities(binding))
                        .unwrap();
                    let mut bytes = [0_u8; 1];
                    let frame = backend
                        .receive(&binding, &mut bytes, authorities(binding))
                        .unwrap()
                        .unwrap();
                    assert_eq!(frame.kind, DistributedFrameKind::Cancellation);
                    json!({"kind": "cancellation"})
                }
                "oracle-oversized-frame" => {
                    let reason = backend
                        .send(
                            &binding,
                            OutboundDistributedFrame {
                                payload: &[0_u8; 65],
                                ..value_frame(0, b"")
                            },
                            authorities(binding),
                        )
                        .unwrap_err();
                    json!({"accepted": false, "code": reason.code()})
                }
                _ => unreachable!(),
            }
        }
        "replacement-new-epoch" | "security-downgrade-rejected" => {
            let current = binding("tls/host-a.example:7447", CarrierSecurityMode::MutualTls);
            let mut next = current;
            next.carrier_endpoint = Some("tls/host-b.example:7447");
            next.initial_session_epoch = 2;
            next.identity = next.semantic_hash().unwrap();
            let next_security = if case == "security-downgrade-rejected" {
                CarrierSecurityMode::Tls
            } else {
                CarrierSecurityMode::MutualTls
            };
            match validate_transport_transition(
                &current,
                CarrierSecurityMode::MutualTls,
                &next,
                next_security,
            ) {
                Ok(transition) => {
                    assert_eq!(transition, TransportTransition::NewSessionEpoch);
                    json!({"accepted": true, "transition": "new-session-epoch"})
                }
                Err(reason) => fixture_rejection(reason),
            }
        }
        other => panic!("fixture case `{other}` has no executable reference assertion"),
    }
}

#[test]
fn every_zenoh_transport_fixture_case_executes_independently() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("Zenoh fixture JSON");
    let cases = fixture["cases"].as_array().expect("fixture cases");
    assert_eq!(cases.len(), 37);
    for case in cases {
        let id = case["id"].as_str().expect("case ID");
        assert_eq!(execute_fixture(id), case["expected"], "case `{id}`");
    }
}
