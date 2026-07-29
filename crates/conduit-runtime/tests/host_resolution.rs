use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, AuthorityTime,
    CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, CompatibilityOutcome, DescriptorRef,
    ExecutionPlan, ExecutorKind, ExplicitSatisfactionRequirement, Id, ImplementationManifest,
    InstancePath, ManifestArtifactRef, ManifestEntrypoint, PassportStatus,
    PassportStatusObservation, PinnedDescriptor, PlanArtifact, PlanHostObservation,
    PlanResourceBudget, PlanValidationContext, ReportCapability, ReportMembership, ReportResource,
    ReproducibilityClaim, ResolvedPlanNode, ResourceRef, SatisfactionFacet, SatisfactionMethod,
    SatisfactionObligation, SatisfactionPin, SatisfactionProof, SatisfactionReason,
    SatisfactionRole, SemanticHash,
};
use conduit_runtime::{
    CandidateRejectionReason, CapabilityPredicate, HostResolverPolicy, PlacementCandidate,
    PlacementRequest, ResolverTiePolicy, ResourcePredicate, resolve_host_placement,
    seal_resolved_execution_plan,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c5/host-resolution-v1.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const CONTRACT: PinnedDescriptor<'static> = pin("fixture/wifi-network", 1);
const PROFILE: PinnedDescriptor<'static> = pin("fixture/execution-profile", 2);
const CAPABILITY: PinnedDescriptor<'static> = pin("conduit/host.wifi-network", 3);
const REPORTER: PinnedDescriptor<'static> = pin("fixture/reporter", 4);
const TRUST: PinnedDescriptor<'static> = pin("fixture/trust", 5);
const RESOLVER: PinnedDescriptor<'static> = pin("fixture/resolver", 6);
const STATUS_REPORTER: PinnedDescriptor<'static> = pin("fixture/status-reporter", 7);
const BROWSER_PLACEMENT: PinnedDescriptor<'static> = pin("conduit/browser-placement", 8);
const REALM: Id<'static> = Id("fixture/realm");
const ENTITY: Id<'static> = Id("fixture/entity");
const PASSPORT: SemanticHash = SemanticHash::from_bytes([8; 32]);
static TRUSTED_ENTITIES: [Id<'static>; 1] = [ENTITY];
static OTHER_ENTITIES: [Id<'static>; 1] = [Id("fixture/other-entity")];
static TRUSTED_STATUS_REPORTERS: [SemanticHash; 1] = [STATUS_REPORTER.semantic_hash];
static OTHER_STATUS_REPORTERS: [SemanticHash; 1] = [SemanticHash::from_bytes([9; 32])];
const LINUX_DIGEST: ArtifactDigest = ArtifactDigest::from_bytes([10; 32]);
const PICO_DIGEST: ArtifactDigest = ArtifactDigest::from_bytes([11; 32]);
const LINUX_REF: ManifestArtifactRef<'static> = artifact_ref("fixture/linux-blob", LINUX_DIGEST);
const PICO_REF: ManifestArtifactRef<'static> = artifact_ref("fixture/pico-blob", PICO_DIGEST);
const LINUX_CAPABILITY: ReportCapability<'static> = ReportCapability {
    interface: CAPABILITY,
    mode: Id("ap"),
    subject: Id("wlan0"),
    details: SemanticHash::from_bytes([20; 32]),
    capacity: budget(8, 1, 1),
};
const PICO_CAPABILITY: ReportCapability<'static> = ReportCapability {
    interface: CAPABILITY,
    mode: Id("ap"),
    subject: Id("cyw43"),
    details: SemanticHash::from_bytes([21; 32]),
    capacity: budget(8, 1, 1),
};
static BROWSER_PLACEMENTS: [ReportCapability<'static>; 7] = [
    browser_capability("window", 31),
    browser_capability("dedicated-worker", 32),
    browser_capability("shared-worker", 33),
    browser_capability("service-worker", 34),
    browser_capability("audio-worklet", 35),
    browser_capability("wasm", 36),
    browser_capability("webgpu", 37),
];
static BROWSER_IMPLEMENTATIONS: [&str; 7] = [
    "fixture/browser-window",
    "fixture/browser-dedicated-worker",
    "fixture/browser-shared-worker",
    "fixture/browser-service-worker",
    "fixture/browser-audio-worklet",
    "fixture/browser-wasm",
    "fixture/browser-webgpu",
];

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

const fn artifact_ref(id: &'static str, digest: ArtifactDigest) -> ManifestArtifactRef<'static> {
    ManifestArtifactRef {
        id: Id(id),
        digest,
        role: Id("executable"),
        required: true,
    }
}

const fn browser_capability(mode: &'static str, details: u8) -> ReportCapability<'static> {
    ReportCapability {
        interface: BROWSER_PLACEMENT,
        mode: Id(mode),
        subject: Id(mode),
        details: SemanticHash::from_bytes([details; 32]),
        capacity: budget(8, 1, 1),
    }
}

const fn budget(memory_bytes: u64, cpu_units: u32, transports: u16) -> PlanResourceBudget {
    PlanResourceBudget {
        memory_bytes,
        storage_bytes: 0,
        cpu_units,
        timers: 0,
        transports,
        checkpoints: 0,
        evidence_bytes: 0,
    }
}

fn artifact(id: &'static str, digest: ArtifactDigest) -> ArtifactManifest<'static> {
    let mut manifest = ArtifactManifest {
        schema_version: 1,
        identity: ZERO,
        id: Id(id),
        digest,
        media_type: "application/octet-stream",
        byte_size: 64,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([12; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([13; 32]),
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
    let mut scratch = [ZERO; 1];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

fn implementation(
    id: &'static str,
    executor: ExecutorKind,
    artifact: &'static ManifestArtifactRef<'static>,
    authorities: &'static [SemanticHash],
) -> ImplementationManifest<'static> {
    let mut manifest = ImplementationManifest {
        schema_version: 1,
        identity: ZERO,
        id: Id(id),
        implementation_version: "1",
        semantic_contract: CONTRACT,
        executor,
        entrypoint: ManifestEntrypoint {
            name: Id("run"),
            adapter: Id("conduit-step-v1"),
            abi: Id("fixture-abi-v1"),
            protocol_version: 1,
        },
        execution_profile: PROFILE,
        artifacts: core::slice::from_ref(artifact),
        required_interfaces: &[],
        provided_interfaces: &[],
        required_authorities: authorities,
        required_effects: &[],
        minimum_plan_version: 1,
        maximum_plan_version: 8,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        replacement: conduit_core::ReplacementSupport::Cold,
        coexistence_memory_bytes: 0,
        reproducibility: Some(ReproducibilityClaim {
            source_digest: ArtifactDigest::from_bytes([12; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([13; 32]),
            expected_artifact_digest: artifact.digest,
        }),
    };
    let mut scratch = [ZERO; 4];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

fn report<'a>(
    id: &'a str,
    host: &'a str,
    valid_until_tick: u64,
    available: PlanResourceBudget,
    capabilities: &'a [ReportCapability<'a>],
    executors: &'a [ExecutorKind],
) -> CapabilityReport<'a> {
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: ZERO,
        id: Id(id),
        host: Id(host),
        reporter: REPORTER,
        trust: TRUST,
        membership: None,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick,
        available,
        capabilities,
        resources: &[],
        topology: &[],
        supported_executors: executors,
        supported_targets: &[],
        supported_abis: &[],
        minimum_plan_version: 1,
        maximum_plan_version: 8,
        current_constraints: &[],
    };
    let mut scratch = [ZERO; 8];
    report.identity = report.computed_semantic_hash(&mut scratch).unwrap();
    report
}

fn policy(
    preference: &'static [Id<'static>],
    tie_policy: ResolverTiePolicy,
) -> HostResolverPolicy<'static> {
    let mut policy = HostResolverPolicy {
        resolver: RESOLVER,
        policy_hash: ZERO,
        time_basis: Id("fixture/clock"),
        current_tick: 20,
        plan_version: 1,
        trusted_reporters: &[REPORTER.semantic_hash],
        trusted_report_trust: &[TRUST.semantic_hash],
        required_realm: None,
        trusted_entities: &[],
        trusted_status_reporters: &[],
        require_active_passport: false,
        allowed_implementations: &[],
        implementation_preference: preference,
        tie_policy,
        maximum_search_states: 64,
    };
    policy.policy_hash = policy.computed_semantic_hash().unwrap();
    policy
}

fn capability_requirement() -> CapabilityPredicate<'static> {
    CapabilityPredicate {
        interface: CAPABILITY,
        mode: Id("ap"),
        subject: None,
        details: None,
        minimum_capacity: budget(8, 1, 1),
        satisfaction_proof: None,
    }
}

fn membership(status: PassportStatus) -> ReportMembership<'static> {
    ReportMembership {
        realm: REALM,
        entity: ENTITY,
        passport: PASSPORT,
        status: PassportStatusObservation {
            passport: PASSPORT,
            realm: REALM,
            entity: ENTITY,
            reporter: STATUS_REPORTER,
            time_basis: Id("fixture/clock"),
            observed_at_tick: 10,
            valid_until_tick: 30,
            status,
        },
    }
}

fn identify_report(report: &mut CapabilityReport<'_>) {
    let mut scratch = [ZERO; 8];
    report.identity = report.computed_semantic_hash(&mut scratch).unwrap();
}

#[test]
fn resolver_enforces_realm_entity_and_fresh_passport_status() {
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let manifest = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let mut authenticated_report = report(
        "fixture/authenticated-report",
        "linux",
        30,
        budget(16, 2, 2),
        &[LINUX_CAPABILITY],
        &[ExecutorKind::NativeInProcess],
    );
    authenticated_report.membership = Some(membership(PassportStatus::Active));
    identify_report(&mut authenticated_report);
    let artifacts = [&linux_artifact];
    let required = [capability_requirement()];
    let candidate = PlacementCandidate {
        manifest: &manifest,
        artifacts: &artifacts,
        report: &authenticated_report,
        allocation: budget(8, 1, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let candidates = [candidate];
    let requests = [PlacementRequest {
        instance: InstancePath::new("root/wifi").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &candidates,
    }];
    let mut authenticated_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    authenticated_policy.required_realm = Some(REALM);
    authenticated_policy.trusted_entities = &TRUSTED_ENTITIES;
    authenticated_policy.trusted_status_reporters = &TRUSTED_STATUS_REPORTERS;
    authenticated_policy.require_active_passport = true;
    authenticated_policy.policy_hash = authenticated_policy.computed_semantic_hash().unwrap();
    assert!(resolve_host_placement(&requests, authenticated_policy).is_ok());

    let mut missing_report = authenticated_report;
    missing_report.membership = None;
    identify_report(&mut missing_report);
    let missing_candidate = PlacementCandidate {
        report: &missing_report,
        ..candidate
    };
    let missing_candidates = [missing_candidate];
    let missing_requests = [PlacementRequest {
        candidates: &missing_candidates,
        ..requests[0]
    }];
    assert!(
        resolve_host_placement(&missing_requests, authenticated_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::PassportStatusRejected)
    );
    let mut entity_only_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    entity_only_policy.trusted_entities = &TRUSTED_ENTITIES;
    entity_only_policy.policy_hash = entity_only_policy.computed_semantic_hash().unwrap();
    assert!(
        resolve_host_placement(&missing_requests, entity_only_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::PassportStatusRejected)
    );

    let mut wrong_realm_policy = authenticated_policy;
    wrong_realm_policy.required_realm = Some(Id("fixture/other-realm"));
    wrong_realm_policy.policy_hash = wrong_realm_policy.computed_semantic_hash().unwrap();
    assert!(
        resolve_host_placement(&requests, wrong_realm_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::RealmMismatch)
    );

    let mut wrong_entity_policy = authenticated_policy;
    wrong_entity_policy.trusted_entities = &OTHER_ENTITIES;
    wrong_entity_policy.policy_hash = wrong_entity_policy.computed_semantic_hash().unwrap();
    assert!(
        resolve_host_placement(&requests, wrong_entity_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::EntityRejected)
    );

    let mut untrusted_status_policy = authenticated_policy;
    untrusted_status_policy.trusted_status_reporters = &OTHER_STATUS_REPORTERS;
    untrusted_status_policy.policy_hash = untrusted_status_policy.computed_semantic_hash().unwrap();
    assert!(
        resolve_host_placement(&requests, untrusted_status_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::PassportStatusRejected)
    );

    let mut revoked_report = authenticated_report;
    revoked_report.membership = Some(membership(PassportStatus::Revoked));
    identify_report(&mut revoked_report);
    let revoked_candidate = PlacementCandidate {
        report: &revoked_report,
        ..candidate
    };
    let revoked_candidates = [revoked_candidate];
    let revoked_requests = [PlacementRequest {
        candidates: &revoked_candidates,
        ..requests[0]
    }];
    assert!(
        resolve_host_placement(&revoked_requests, authenticated_policy)
            .unwrap_err()
            .candidates[0]
            .reasons
            .contains(&CandidateRejectionReason::PassportStatusRejected)
    );
}

#[test]
fn resolver_never_enrolls_prompts_or_mutates_membership_inputs() {
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let manifest = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let mut authenticated_report = report(
        "fixture/pure-resolution-report",
        "linux",
        30,
        budget(16, 2, 2),
        &[LINUX_CAPABILITY],
        &[ExecutorKind::NativeInProcess],
    );
    authenticated_report.membership = Some(membership(PassportStatus::Active));
    identify_report(&mut authenticated_report);
    let original_report = authenticated_report;
    let artifacts = [&linux_artifact];
    let required = [capability_requirement()];
    let candidate = PlacementCandidate {
        manifest: &manifest,
        artifacts: &artifacts,
        report: &authenticated_report,
        allocation: budget(8, 1, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let candidates = [candidate];
    let requests = [PlacementRequest {
        instance: InstancePath::new("root/wifi").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &candidates,
    }];
    let mut authenticated_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    authenticated_policy.required_realm = Some(REALM);
    authenticated_policy.trusted_entities = &TRUSTED_ENTITIES;
    authenticated_policy.trusted_status_reporters = &TRUSTED_STATUS_REPORTERS;
    authenticated_policy.require_active_passport = true;
    authenticated_policy.policy_hash = authenticated_policy.computed_semantic_hash().unwrap();
    let original_policy_hash = authenticated_policy.policy_hash;

    let resolved = resolve_host_placement(&requests, authenticated_policy).unwrap();
    assert_eq!(resolved.bindings.len(), 1);
    assert_eq!(authenticated_report, original_report);
    assert_eq!(authenticated_policy.policy_hash, original_policy_hash);

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c2/realms-passports-v1.json"
    ))
    .unwrap();
    let expected = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "resolver-never-enrolls-or-prompts")
        .unwrap()["expected"]
        .clone();
    assert_eq!(
        serde_json::json!({
            "enrolled": false,
            "prompted": false,
            "mutated": false
        }),
        expected
    );
}

#[test]
fn browser_placements_use_generic_resolution_and_partition_with_linux() {
    let browser_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let mut browser_report = report(
        "fixture/browser-report",
        "browser",
        30,
        budget(64, 8, 8),
        &BROWSER_PLACEMENTS,
        &[ExecutorKind::WasmComponent],
    );
    browser_report.membership = Some(membership(PassportStatus::Active));
    identify_report(&mut browser_report);
    let artifacts = [&browser_artifact];
    let mut authenticated_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    authenticated_policy.required_realm = Some(REALM);
    authenticated_policy.trusted_entities = &TRUSTED_ENTITIES;
    authenticated_policy.trusted_status_reporters = &TRUSTED_STATUS_REPORTERS;
    authenticated_policy.require_active_passport = true;
    authenticated_policy.policy_hash = authenticated_policy.computed_semantic_hash().unwrap();

    for (index, capability) in BROWSER_PLACEMENTS.iter().enumerate() {
        let manifest = implementation(
            BROWSER_IMPLEMENTATIONS[index],
            ExecutorKind::WasmComponent,
            &LINUX_REF,
            &[],
        );
        let required = [CapabilityPredicate {
            interface: BROWSER_PLACEMENT,
            mode: capability.mode,
            subject: Some(capability.subject),
            details: Some(capability.details),
            minimum_capacity: budget(8, 1, 1),
            satisfaction_proof: None,
        }];
        let candidate = PlacementCandidate {
            manifest: &manifest,
            artifacts: &artifacts,
            report: &browser_report,
            allocation: budget(8, 1, 1),
            capabilities: &required,
            resources: &[],
            topology: &[],
            authorities: &[],
        };
        let candidates = [candidate];
        let requests = [PlacementRequest {
            instance: InstancePath::new("root/browser-placement").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &candidates,
        }];
        let resolved = resolve_host_placement(&requests, authenticated_policy).unwrap();
        assert_eq!(
            resolved.bindings[0].implementation_id,
            BROWSER_IMPLEMENTATIONS[index]
        );
        assert_eq!(resolved.bindings[0].host, "browser");
        assert_eq!(
            resolved.bindings[0].capability_subjects,
            [capability.mode.as_str()]
        );
    }

    let browser_wasm = implementation(
        "fixture/browser-wasm-portable-transform",
        ExecutorKind::WasmComponent,
        &LINUX_REF,
        &[],
    );
    let native_fake = implementation(
        "fixture/native-fake-portable-transform",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let native_fake_report = report(
        "fixture/native-fake-report",
        "native-test-host",
        30,
        budget(64, 8, 8),
        &[],
        &[ExecutorKind::NativeInProcess],
    );
    let browser_wasm_candidate = PlacementCandidate {
        manifest: &browser_wasm,
        artifacts: &artifacts,
        report: &browser_report,
        allocation: budget(8, 1, 1),
        capabilities: &[],
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let native_fake_candidate = PlacementCandidate {
        manifest: &native_fake,
        artifacts: &artifacts,
        report: &native_fake_report,
        allocation: budget(8, 1, 1),
        capabilities: &[],
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let browser_wasm_candidates = [browser_wasm_candidate];
    let native_fake_candidates = [native_fake_candidate];
    let browser_wasm_requests = [PlacementRequest {
        instance: InstancePath::new("root/portable-transform").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &browser_wasm_candidates,
    }];
    let native_fake_requests = [PlacementRequest {
        instance: InstancePath::new("root/portable-transform").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &native_fake_candidates,
    }];
    let browser_wasm_plan = resolve_host_placement(
        &browser_wasm_requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap();
    let native_fake_plan = resolve_host_placement(
        &native_fake_requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap();
    assert_eq!(
        browser_wasm.semantic_contract,
        native_fake.semantic_contract
    );
    assert_eq!(
        browser_wasm_plan.bindings[0].implementation_id,
        "fixture/browser-wasm-portable-transform"
    );
    assert_eq!(
        native_fake_plan.bindings[0].implementation_id,
        "fixture/native-fake-portable-transform"
    );
    assert_ne!(
        browser_wasm_plan.bindings[0].implementation_id,
        native_fake_plan.bindings[0].implementation_id
    );

    let linux_artifact = artifact("fixture/pico-blob", PICO_DIGEST);
    let browser_manifest = implementation(
        "fixture/browser-audio-worklet",
        ExecutorKind::WasmComponent,
        &LINUX_REF,
        &[],
    );
    let linux_manifest = implementation(
        "fixture/linux-speech",
        ExecutorKind::RemoteEndpoint,
        &PICO_REF,
        &[],
    );
    let remote_capability = browser_capability("remote-session", 38);
    let linux_report = report(
        "fixture/linux-speech-report",
        "linux",
        30,
        budget(64, 8, 8),
        core::slice::from_ref(&remote_capability),
        &[ExecutorKind::RemoteEndpoint],
    );
    let browser_artifacts = [&browser_artifact];
    let linux_artifacts = [&linux_artifact];
    let audio_required = [CapabilityPredicate {
        interface: BROWSER_PLACEMENT,
        mode: Id("audio-worklet"),
        subject: Some(Id("audio-worklet")),
        details: Some(SemanticHash::from_bytes([35; 32])),
        minimum_capacity: budget(8, 1, 1),
        satisfaction_proof: None,
    }];
    let remote_required = [CapabilityPredicate {
        interface: BROWSER_PLACEMENT,
        mode: Id("remote-session"),
        subject: Some(Id("remote-session")),
        details: Some(SemanticHash::from_bytes([38; 32])),
        minimum_capacity: budget(8, 1, 1),
        satisfaction_proof: None,
    }];
    let browser_candidate = PlacementCandidate {
        manifest: &browser_manifest,
        artifacts: &browser_artifacts,
        report: &browser_report,
        allocation: budget(8, 1, 1),
        capabilities: &audio_required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let linux_candidate = PlacementCandidate {
        manifest: &linux_manifest,
        artifacts: &linux_artifacts,
        report: &linux_report,
        allocation: budget(8, 1, 1),
        capabilities: &remote_required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let browser_candidates = [browser_candidate];
    let linux_candidates = [linux_candidate];
    let requests = [
        PlacementRequest {
            instance: InstancePath::new("root/preprocess").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &browser_candidates,
        },
        PlacementRequest {
            instance: InstancePath::new("root/speech").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &linux_candidates,
        },
    ];
    let distributed = resolve_host_placement(
        &requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap();
    assert_eq!(distributed.bindings[0].host, "browser");
    assert_eq!(distributed.bindings[1].host, "linux");

    let source = include_str!("../../../examples/browser-linux-partition.panel");
    conduit_panel::parse(source)
        .expect("browser/Linux reference panel uses ordinary bounded source");
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/c5/browser-host-v1.json")).unwrap();
    let expected = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "browser-linux-partition-uses-generic-resolver")
        .unwrap()["expected"]
        .clone();
    assert_eq!(
        serde_json::json!({"accepted": true, "hosts": ["browser", "linux"]}),
        expected
    );
    let portable_expected = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "browser-wasm-native-fake-same-contract")
        .unwrap()["expected"]
        .clone();
    assert_eq!(
        serde_json::json!({
            "accepted": true,
            "same_semantic_contract": true,
            "distinct_implementations": true
        }),
        portable_expected
    );
}

#[test]
fn linux_and_pico_resolve_identically_when_candidate_input_is_shuffled() {
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let pico_artifact = artifact("fixture/pico-blob", PICO_DIGEST);
    let linux = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let pico = implementation("fixture/pico", ExecutorKind::Firmware, &PICO_REF, &[]);
    let linux_report = report(
        "fixture/linux-report",
        "linux",
        30,
        budget(16, 2, 2),
        &[LINUX_CAPABILITY],
        &[ExecutorKind::NativeInProcess],
    );
    let pico_report = report(
        "fixture/pico-report",
        "pico",
        30,
        budget(16, 2, 2),
        &[PICO_CAPABILITY],
        &[ExecutorKind::Firmware],
    );
    let required = [capability_requirement()];
    let linux_artifacts = [&linux_artifact];
    let pico_artifacts = [&pico_artifact];
    let linux_candidate = PlacementCandidate {
        manifest: &linux,
        artifacts: &linux_artifacts,
        report: &linux_report,
        allocation: budget(8, 1, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let pico_candidate = PlacementCandidate {
        manifest: &pico,
        artifacts: &pico_artifacts,
        report: &pico_report,
        allocation: budget(8, 1, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    };
    let forward_candidates = [linux_candidate, pico_candidate];
    let reverse_candidates = [pico_candidate, linux_candidate];
    let forward = [PlacementRequest {
        instance: InstancePath::new("root/wifi").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &forward_candidates,
    }];
    let reverse = [PlacementRequest {
        candidates: &reverse_candidates,
        ..forward[0]
    }];
    let canonical_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    let first = resolve_host_placement(&forward, canonical_policy).unwrap();
    let second = resolve_host_placement(&reverse, canonical_policy).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.bindings[0].implementation_id, "fixture/linux");
    assert_eq!(first.bindings[0].capability_subjects, ["wlan0".to_owned()]);
    let observations = [PlanHostObservation {
        id: linux_report.id,
        host: linux_report.host,
        semantic_hash: linux_report.identity,
        time_basis: linux_report.time_basis,
        observed_at_tick: linux_report.observed_at_tick,
        valid_until_tick: linux_report.valid_until_tick,
    }];
    let artifacts = [PlanArtifact {
        id: LINUX_REF.id,
        digest: LINUX_REF.digest,
    }];
    let nodes = [ResolvedPlanNode {
        instance: forward[0].instance,
        contract: CONTRACT,
        implementation: PinnedDescriptor {
            id: linux.id,
            schema_version: linux.schema_version,
            semantic_hash: linux.identity,
        },
        lifecycle_policy: pin("fixture/lifecycle", 50),
        execution_profile: None,
        artifact: LINUX_REF.id,
        host_observation: linux_report.id,
        host: linux_report.host,
        allocation: budget(8, 1, 1),
        required_resources: &[],
        required_effects: &[],
    }];
    let mut plan = ExecutionPlan {
        schema_version: 1,
        identity: ZERO,
        source_semantic_hash: SemanticHash::from_bytes([51; 32]),
        resolver: RESOLVER,
        resolver_policy_hash: canonical_policy.policy_hash,
        created_at: AuthorityTime {
            basis: Id("fixture/clock"),
            tick: 20,
        },
        budget: budget(8, 1, 1),
        host_observations: &observations,
        resources: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &[],
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &[],
        hazard_closure: None,
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        supervisions: &[],
        unresolved: &[],
    };
    let mut plan_scratch = [ZERO; 16];
    plan.identity = plan.semantic_hash(&mut plan_scratch).unwrap();
    assert_eq!(
        seal_resolved_execution_plan(
            &first,
            &plan,
            PlanValidationContext {
                supported_schema_version: 1,
                now: AuthorityTime {
                    basis: Id("fixture/clock"),
                    tick: 20,
                },
            },
        ),
        Ok(())
    );

    let preferred = resolve_host_placement(
        &forward,
        policy(
            &[Id("fixture/pico")],
            ResolverTiePolicy::LowestCanonicalIdentity,
        ),
    )
    .unwrap();
    assert_eq!(preferred.bindings[0].implementation_id, "fixture/pico");
    assert_eq!(
        resolve_host_placement(&forward, policy(&[], ResolverTiePolicy::RejectAmbiguous))
            .unwrap_err()
            .global_reasons,
        [CandidateRejectionReason::Ambiguous]
    );
}

#[test]
fn aggregate_capacity_produces_deterministic_distributed_placement() {
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let pico_artifact = artifact("fixture/pico-blob", PICO_DIGEST);
    let linux = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let pico = implementation("fixture/pico", ExecutorKind::Firmware, &PICO_REF, &[]);
    let linux_report = report(
        "fixture/linux-report",
        "linux",
        30,
        budget(8, 2, 2),
        &[LINUX_CAPABILITY],
        &[ExecutorKind::NativeInProcess],
    );
    let pico_report = report(
        "fixture/pico-report",
        "pico",
        30,
        budget(8, 2, 2),
        &[PICO_CAPABILITY],
        &[ExecutorKind::Firmware],
    );
    let required = [capability_requirement()];
    let linux_artifacts = [&linux_artifact];
    let pico_artifacts = [&pico_artifact];
    let candidates = [
        PlacementCandidate {
            manifest: &linux,
            artifacts: &linux_artifacts,
            report: &linux_report,
            allocation: budget(8, 1, 1),
            capabilities: &required,
            resources: &[],
            topology: &[],
            authorities: &[],
        },
        PlacementCandidate {
            manifest: &pico,
            artifacts: &pico_artifacts,
            report: &pico_report,
            allocation: budget(8, 1, 1),
            capabilities: &required,
            resources: &[],
            topology: &[],
            authorities: &[],
        },
    ];
    let requests = [
        PlacementRequest {
            instance: InstancePath::new("root/a").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &candidates,
        },
        PlacementRequest {
            instance: InstancePath::new("root/b").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &candidates,
        },
    ];
    let resolved = resolve_host_placement(
        &requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap();
    assert_eq!(resolved.bindings.len(), 2);
    assert_eq!(resolved.bindings[0].host, "linux");
    assert_eq!(resolved.bindings[1].host, "pico");
}

#[test]
fn every_candidate_rejection_is_retained_without_host_mutation() {
    static AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([40; 32])];
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let manifest = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &AUTHORITY,
    );
    let stale = report(
        "fixture/stale-report",
        "linux",
        19,
        budget(4, 1, 0),
        &[],
        &[ExecutorKind::NativeInProcess],
    );
    let artifacts = [&linux_artifact];
    let required = [capability_requirement()];
    let candidates = [PlacementCandidate {
        manifest: &manifest,
        artifacts: &artifacts,
        report: &stale,
        allocation: budget(8, 2, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    }];
    let requests = [PlacementRequest {
        instance: InstancePath::new("root/wifi").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &candidates,
    }];
    let failure = resolve_host_placement(
        &requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap_err();
    assert_eq!(failure.candidates.len(), 1);
    for reason in [
        CandidateRejectionReason::StaleReport,
        CandidateRejectionReason::CapabilityMissing,
        CandidateRejectionReason::InsufficientCapacity,
        CandidateRejectionReason::AuthorityDenied,
    ] {
        assert!(failure.candidates[0].reasons.contains(&reason));
    }
}

#[test]
fn c5_fixture_owns_host_network_wifi_and_no_provisioning_boundaries() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.host-resolution/v1");
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 58);
    for required in [
        "single-match",
        "deterministic-tie",
        "explicit-policy-preference",
        "same-inputs-shuffled",
        "stale-report-rejected",
        "report-membership-binds-realm-entity-passport-status",
        "stale-passport-status",
        "required-realm-mismatch",
        "trusted-entity-rejected",
        "missing-revoked-or-untrusted-passport-status",
        "capability-present-resource-insufficient",
        "authority-denial",
        "linux-pico-equivalent-capability",
        "distributed-placement",
        "full-candidate-rejection-tree",
        "exact-plan-sealed",
        "ap-sta-supported-not-concurrent",
        "association-without-dhcp",
        "tcp-connect-without-dns",
        "regulatory-channel-rejection",
        "resolver-performs-no-host-effects",
    ] {
        assert!(
            cases.iter().any(|case| case["id"] == required),
            "missing {required}"
        );
    }
}

#[test]
fn structural_capability_matching_retains_the_directional_proof_and_policy() {
    let offered_capability = pin("fixture/pico-wifi-realization", 60);
    let offered = [ReportCapability {
        interface: offered_capability,
        ..PICO_CAPABILITY
    }];
    let pico_report = report(
        "fixture/pico-report",
        "pico",
        30,
        budget(16, 2, 2),
        &offered,
        &[ExecutorKind::Firmware],
    );
    let pico_artifact = artifact("fixture/pico-blob", PICO_DIGEST);
    let pico = implementation("fixture/pico", ExecutorKind::Firmware, &PICO_REF, &[]);
    let resolver_policy = policy(&[], ResolverTiePolicy::LowestCanonicalIdentity);
    let obligations = [
        "semantic-capability",
        "observation-freshness",
        "resources",
        "effects",
        "authority",
        "boundedness",
    ]
    .map(|id| SatisfactionObligation {
        id: Id(id),
        required_hash: SemanticHash::from_bytes([61; 32]),
        offered_hash: SemanticHash::from_bytes([61; 32]),
        outcome: CompatibilityOutcome::Compatible,
        reason: Id("fixture/accepted"),
    });
    let facets = [SatisfactionFacet {
        id: Id("network/wifi-profile"),
        required_hash: SemanticHash::from_bytes([62; 32]),
        offered_hash: SemanticHash::from_bytes([62; 32]),
    }];
    let mut proof = SatisfactionProof {
        schema_version: 1,
        identity: ZERO,
        role: SatisfactionRole::HostCapability,
        method: SatisfactionMethod::StructuralFacets,
        required: DescriptorRef {
            kind: CAPABILITY.id,
            schema_version: CAPABILITY.schema_version,
            semantic_hash: CAPABILITY.semantic_hash,
        },
        offered: DescriptorRef {
            kind: offered_capability.id,
            schema_version: offered_capability.schema_version,
            semantic_hash: offered_capability.semantic_hash,
        },
        provider: Some(SatisfactionPin {
            descriptor: DescriptorRef {
                kind: Id("fixture/wifi-profile-provider"),
                schema_version: 1,
                semantic_hash: SemanticHash::from_bytes([63; 32]),
            },
        }),
        provider_rule: Some(Id("fixture/wifi-profile-v1")),
        policy: Some(SatisfactionPin {
            descriptor: DescriptorRef {
                kind: Id("fixture/resolver-policy"),
                schema_version: 1,
                semantic_hash: resolver_policy.policy_hash,
            },
        }),
        facets: &facets,
        obligations: &obligations,
        outcome: CompatibilityOutcome::Compatible,
        reason: SatisfactionReason::Satisfied,
        explanation: Id("fixture/structural-match"),
        explicit_requirement: ExplicitSatisfactionRequirement::None,
    };
    let mut proof_scratch = [ZERO; 8];
    proof.identity = proof.semantic_hash(&mut proof_scratch).unwrap();
    let required = [CapabilityPredicate {
        satisfaction_proof: Some(&proof),
        ..capability_requirement()
    }];
    let artifacts = [&pico_artifact];
    let candidates = [PlacementCandidate {
        manifest: &pico,
        artifacts: &artifacts,
        report: &pico_report,
        allocation: budget(8, 1, 1),
        capabilities: &required,
        resources: &[],
        topology: &[],
        authorities: &[],
    }];
    let requests = [PlacementRequest {
        instance: InstancePath::new("root/wifi").unwrap(),
        semantic_contract: CONTRACT,
        candidates: &candidates,
    }];
    let resolved = resolve_host_placement(&requests, resolver_policy).unwrap();
    assert_eq!(resolved.bindings[0].capability_proofs, [proof.identity]);
}

#[test]
fn exclusive_resource_candidates_are_bound_once() {
    let linux_artifact = artifact("fixture/linux-blob", LINUX_DIGEST);
    let linux = implementation(
        "fixture/linux",
        ExecutorKind::NativeInProcess,
        &LINUX_REF,
        &[],
    );
    let capabilities = [ReportCapability {
        capacity: budget(16, 2, 2),
        ..LINUX_CAPABILITY
    }];
    let executors = [ExecutorKind::NativeInProcess];
    let resources = [
        ReportResource {
            resource: ResourceRef {
                kind: Id("network/radio"),
                id: Id("radio-a"),
            },
            descriptor: pin("fixture/radio-pool", 70),
            capacity: budget(8, 1, 1),
            exclusive: true,
        },
        ReportResource {
            resource: ResourceRef {
                kind: Id("network/radio"),
                id: Id("radio-b"),
            },
            descriptor: pin("fixture/radio-pool", 70),
            capacity: budget(8, 1, 1),
            exclusive: true,
        },
    ];
    let mut linux_report = report(
        "fixture/linux-report",
        "linux",
        30,
        budget(16, 2, 2),
        &capabilities,
        &executors,
    );
    linux_report.resources = &resources;
    let mut report_scratch = [ZERO; 8];
    linux_report.identity = linux_report
        .computed_semantic_hash(&mut report_scratch)
        .unwrap();
    let capability = [capability_requirement()];
    let radio_a = [ResourcePredicate {
        kind: Id("network/radio"),
        id: Some(Id("radio-a")),
        descriptor: Some(pin("fixture/radio-pool", 70)),
        minimum_capacity: budget(8, 1, 1),
        require_exclusive: true,
    }];
    let radio_b = [ResourcePredicate {
        id: Some(Id("radio-b")),
        ..radio_a[0]
    }];
    let artifacts = [&linux_artifact];
    let candidate_a = PlacementCandidate {
        manifest: &linux,
        artifacts: &artifacts,
        report: &linux_report,
        allocation: budget(8, 1, 1),
        capabilities: &capability,
        resources: &radio_a,
        topology: &[],
        authorities: &[],
    };
    let candidates = [
        candidate_a,
        PlacementCandidate {
            resources: &radio_b,
            ..candidate_a
        },
    ];
    let requests = [
        PlacementRequest {
            instance: InstancePath::new("root/a").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &candidates,
        },
        PlacementRequest {
            instance: InstancePath::new("root/b").unwrap(),
            semantic_contract: CONTRACT,
            candidates: &candidates,
        },
    ];
    let resolved = resolve_host_placement(
        &requests,
        policy(&[], ResolverTiePolicy::LowestCanonicalIdentity),
    )
    .unwrap();
    assert_eq!(resolved.bindings[0].resource_ids, ["radio-a"]);
    assert_eq!(resolved.bindings[1].resource_ids, ["radio-b"]);
}
