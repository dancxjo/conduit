use conduit_patchbay::{
    EditOperation, EditRequest, HostConformanceProjectionInput, NodePosition,
    PATCHBAY_PROTOCOL_VERSION, PlanSnapshot, PoolProjectionInput, ProjectionLog, ProjectionUpdate,
    RunSnapshot, RunState, SubjectPath, Workspace, project_host_conformance,
    project_library_catalog, project_pool, project_supervision,
};

const SOURCE: &str = "panel 0\nnode greeting : std/literal { value = \"hello\\n\" }\nnode output : display/text\ncord greeting.value -> output.text\n";
const FIXTURE: &str = include_str!("../../../conformance/c8/patchbay-protocol.json");

fn request(workspace: &Workspace, operations: Vec<EditOperation>) -> EditRequest {
    EditRequest {
        protocol_version: PATCHBAY_PROTOCOL_VERSION,
        document_id: workspace.source().document_id.clone(),
        expected_source_revision: workspace.source().revision,
        expected_presentation_revision: workspace.presentation().revision,
        operations,
    }
}

#[test]
fn checked_contract_import_projection_keeps_alias_identity_and_hash_distinct() {
    let panel = conduit_panel::parse(include_str!(
        "../../../fixtures/contract-package-imports/alias.panel"
    ))
    .unwrap();
    let lock: conduit_panel::ContractPackageLock = serde_json::from_str(include_str!(
        "../../../fixtures/contract-package-imports/contract-package-lock.json"
    ))
    .unwrap();
    let bytes = include_bytes!("../../../fixtures/contract-package-imports/conduit-dev-std.json");
    let resolution = conduit_panel::resolve_package_imports(
        &panel,
        &lock,
        &[conduit_panel::ContractPackageArtifact {
            bytes,
            mirror: Some("repository"),
        }],
    )
    .unwrap();
    let projection = conduit_patchbay::project_contract_imports(&resolution);
    assert_eq!(projection[0].local_name, "split");
    assert_eq!(projection[0].canonical_id, "conduit.dev/std/tee");
    assert_eq!(
        projection[0].descriptor_hash,
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn library_catalog_projection_keeps_provider_bundles_separate_from_observation() {
    let projection =
        project_library_catalog(include_str!("../../../library/catalog.json")).unwrap();
    assert_eq!(projection.schema, "conduit.library-catalog");
    assert_eq!(projection.entries.len(), 114);
    let literal = projection
        .entries
        .iter()
        .find(|entry| entry.semantic_identity == "std/literal")
        .unwrap();
    assert_eq!(literal.classification, "portable-standard");
    assert_eq!(literal.package_owner, "conduit.std");
    assert!(literal.compiler_exported);
    assert_eq!(literal.known_provider_bundles.len(), 1);
    assert_eq!(
        literal.current_provider_observation,
        "not-recorded-in-catalog"
    );
    assert_eq!(literal.standalone_lesson.status, "published");
    let file = projection
        .entries
        .iter()
        .find(|entry| entry.semantic_identity == "fs/read")
        .unwrap();
    assert_eq!(file.classification, "optional-host-boundary");
    assert!(file.known_provider_bundles.is_empty());
    assert_eq!(file.standalone_lesson.status, "required");
}

#[test]
fn library_catalog_projection_rejects_duplicate_and_observation_claims() {
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../library/catalog.json")).unwrap();
    let first = document["entries"][0]["semantic_identity"]
        .as_str()
        .unwrap()
        .to_owned();
    document["entries"][1]["semantic_identity"] = first.into();
    let error = project_library_catalog(&document.to_string()).unwrap_err();
    assert_eq!(error.code, "CND-PBY-014");

    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../library/catalog.json")).unwrap();
    document["entries"][0]["current_provider_observation"] = "available".into();
    let error = project_library_catalog(&document.to_string()).unwrap_err();
    assert_eq!(error.code, "CND-PBY-014");
}

#[test]
fn host_projection_keeps_contract_inventory_observation_and_exact_binding_separate() {
    use conduit_core::{
        ExactProviderBinding, HostClass, HostConformanceProfile, HostExecutionMode, HostExtension,
        HostExtensionKind, Id, PinnedDescriptor, ProviderBoundary, ProviderBounds,
        ProviderConformanceOutcome, ProviderConformanceResult, ProviderInventory,
        ProviderInventoryState, ProviderObservation, ProviderObservationState, SemanticHash,
    };
    let hash = |byte| SemanticHash::from_bytes([byte; 32]);
    let pin = |id, byte| PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    };
    let profile_pin = pin("acme/profile/linux", 1);
    let contract = pin("acme/contract/weather", 2);
    let bundle = pin("acme/provider/weather", 3);
    let adapter = pin("acme/adapter/celsius-to-kelvin", 4);
    let mandatory = [pin("conduit/host/minimal-execution", 5)];
    let providers = [ProviderInventory {
        contract,
        provider_bundle: bundle,
        state: ProviderInventoryState::Linked,
    }];
    let extensions = [HostExtension {
        kind: HostExtensionKind::Adapter,
        descriptor: adapter,
    }];
    let profile = HostConformanceProfile {
        schema_version: 0,
        identity: profile_pin.semantic_hash,
        id: profile_pin.id,
        class: HostClass::LinuxHosted,
        execution_mode: HostExecutionMode::Executable,
        mandatory_facts: &mandatory,
        optional_providers: &providers,
        extensions: &extensions,
    };
    let observation = ProviderObservation {
        id: Id("acme/observation/weather"),
        identity: hash(6),
        profile: profile_pin,
        provider_bundle: bundle,
        host_report: pin("acme/host-report/linux", 7),
        state: ProviderObservationState::Available,
        time_basis: Id("clock/test"),
        observed_at_tick: 10,
        valid_until_tick: 20,
    };
    let facets = [pin("acme/facet/weather", 8)];
    let implementation = pin("acme/implementation/weather", 9);
    let artifact = pin("acme/artifact/weather", 10);
    let bounds = ProviderBounds {
        maximum_in_flight: 2,
        maximum_foreign_queue: 0,
        maximum_memory_bytes: 4096,
        maximum_cancellation_ticks: 5,
        maximum_evidence_events: 8,
    };
    let conformance = ProviderConformanceResult {
        schema_version: 0,
        identity: hash(11),
        required_contract: contract,
        implementation,
        artifact,
        adapter,
        profile: profile_pin,
        fixture_suite: pin("acme/fixtures/weather", 12),
        offered_facets: &facets,
        satisfaction_proof: hash(13),
        boundary: ProviderBoundary::Native,
        outcome: ProviderConformanceOutcome::Passed,
        bounds,
        time_basis: Id("clock/test"),
        observed_at_tick: 10,
        valid_until_tick: 20,
    };
    let binding = ExactProviderBinding {
        profile: profile_pin,
        required_contract: contract,
        provider_bundle: bundle,
        implementation,
        artifact,
        adapter,
        host_report: observation.host_report,
        observation: observation.identity,
        satisfaction_proof: conformance.satisfaction_proof,
        conformance_result: conformance.identity,
        bounds,
    };
    let projection = project_host_conformance(HostConformanceProjectionInput {
        profile_pin,
        profile,
        observations: &[observation],
        conformance_results: &[conformance],
        bindings: &[binding],
    });
    assert_eq!(projection.mandatory_facts.len(), 1);
    assert_eq!(projection.optional_providers.len(), 1);
    assert_eq!(projection.extensions.len(), 1);
    assert_eq!(projection.exact_bindings.len(), 1);
    assert_eq!(
        projection.optional_providers[0]
            .observation_state
            .as_deref(),
        Some("available")
    );
    assert_eq!(projection.exact_bindings[0].offered_facets.len(), 1);
    assert_eq!(projection.exact_bindings[0].maximum_foreign_queue, 0);
}

#[test]
fn move_only_changes_presentation_identity() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let semantic = workspace.semantic();
    let original_presentation = workspace.presentation().identity.clone();
    let result = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::MoveNode {
                node_id: "greeting".to_owned(),
                position: NodePosition { x: 24, y: -8 },
            }],
        ))
        .expect("move applies");
    assert_eq!(result.semantic, semantic);
    assert_eq!(result.source.revision, 0);
    assert_eq!(result.presentation.revision, 1);
    assert_ne!(result.presentation.identity, original_presentation);
}

#[test]
fn source_edit_changes_semantics_but_not_an_existing_run() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let old = workspace
        .semantic()
        .source_semantic_hash
        .expect("valid fixture has a semantic identity");
    let plan = PlanSnapshot {
        identity: "sha256:plan".to_owned(),
        source_semantic_hash: old.clone(),
        bindings: Vec::new(),
        value_envelopes: Vec::new(),
        clock_conversions: Vec::new(),
        feedback_boundaries: Vec::new(),
        resource_leases: Vec::new(),
        effect_commit_profiles: Vec::new(),
        workloads: Vec::new(),
    };
    let run = RunSnapshot {
        run_id: "run/1".to_owned(),
        plan_identity: plan.identity,
        source_semantic_hash: plan.source_semantic_hash,
        state: RunState::Running,
    };
    let result = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::ReplaceSource {
                source: SOURCE.replace("hello", "goodbye"),
            }],
        ))
        .expect("source edit applies");
    assert_ne!(result.semantic.source_semantic_hash, Some(old.clone()));
    assert_eq!(run.source_semantic_hash, old);
    assert_eq!(run.plan_identity, "sha256:plan");
}

#[test]
fn stale_transactions_reject_while_raw_invalid_source_becomes_current() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let stale = EditRequest {
        expected_source_revision: 1,
        ..request(&workspace, Vec::new())
    };
    assert_eq!(
        workspace.apply(stale).expect_err("stale rejected").code,
        "CND-PBY-003"
    );
    let result = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::ReplaceSource {
                source: "panel nope".to_owned(),
            }],
        ))
        .expect("raw editor source commits independently of semantic validity");
    assert_eq!(result.source.revision, 1);
    assert_eq!(result.source.semantic_hash, None);
    assert!(!result.compatibility.compatible);
    assert_eq!(result.compatibility.code, "CND-SRC-001");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(workspace.source(), &result.source);
}

#[test]
fn protocol_version_and_unknown_visual_subject_fail_closed() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let unsupported = EditRequest {
        protocol_version: PATCHBAY_PROTOCOL_VERSION.saturating_add(1),
        ..request(&workspace, Vec::new())
    };
    assert_eq!(
        workspace
            .apply(unsupported)
            .expect_err("version rejected")
            .code,
        "CND-PBY-001"
    );
    let unknown = request(
        &workspace,
        vec![EditOperation::MoveNode {
            node_id: "missing".to_owned(),
            position: NodePosition { x: 0, y: 0 },
        }],
    );
    assert_eq!(
        workspace
            .apply(unknown)
            .expect_err("unknown node rejected")
            .code,
        "CND-PBY-005"
    );
}

#[test]
fn projection_gaps_require_explicit_resynchronization() {
    let mut projection = ProjectionLog::new("evidence/run-1", 2).expect("bounded retention");
    projection.append(SubjectPath::Logical("greeting".to_owned()));
    projection.append(SubjectPath::Expanded("greeting#0".to_owned()));
    projection.append(SubjectPath::Logical("output".to_owned()));
    assert_eq!(
        projection.observe_from(0),
        vec![ProjectionUpdate::Gap {
            requested: 0,
            earliest_available: 2
        }]
    );
    assert_eq!(projection.observe_from(1).len(), 2);
    assert_eq!(
        ProjectionLog::new("evidence/run-1", 0)
            .expect_err("unbounded retention rejected")
            .code,
        "CND-PBY-006"
    );
}

#[test]
fn fixture_names_each_required_protocol_boundary() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let ids = fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| case["id"].as_str().expect("case id"))
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "move-only-preserves-semantic-identity",
        "source-edit-creates-new-semantic-revision",
        "stale-edit-is-atomic-conflict",
        "invalid-source-is-current-diagnostic",
        "run-remains-pinned-while-source-changes",
        "logical-and-expanded-subjects-are-distinct",
        "projection-gap-requires-resync",
        "protocol-version-mismatch-fails-closed",
        "persistent-session-rejects-stale-concurrent-revision",
        "typed-connect-validates-through-resolver",
        "incompatible-connection-is-atomic",
        "hidden-composite-port-is-inaccessible",
        "invalid-config-span-is-atomic",
        "projection-truncation-is-explicit",
        "renderer-failure-preserves-headless-control",
        "direct-active-plan-mutation-is-not-a-protocol-operation",
        "candidate-plan-does-not-mutate-active-run",
        "authoritative-view-has-no-browser-inference",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }
}

#[test]
fn exact_plan_projection_preserves_authoritative_binding_state() {
    use conduit_core::{
        AuthorityGrant, AuthorityScope, AuthorityTime, ClockRounding, DelegationPolicy,
        EffectCommitProfile, EffectDiscontinuity, EffectIdempotency, EffectRequirement,
        ExecutionPlan, FeedbackBoundaryKind, FeedbackInitialization, FeedbackReplayGapPolicy,
        FeedbackTerminalPolicy, ForeignRetention, HostCapability, Id, InstancePath,
        PinnedDescriptor, PlanAuthority, PlanClockConversion, PlanFeedbackBoundary,
        PlanHostObservation, PlanResourceBinding, PlanResourceBudget, ResolvedAuthorityBinding,
        ResolvedPlanNode, ResourceLeaseContract, ResourceRef, ResourceSelector,
        ResourceSharingMode, SemanticHash, Sensitivity, StopPolicy, UnknownCommitPolicy,
        ValueEnvelopePolicy,
    };

    const fn hash(byte: u8) -> SemanticHash {
        SemanticHash::from_bytes([byte; 32])
    }

    let hosts = [PlanHostObservation {
        id: Id("report/browser"),
        host: Id("host/browser"),
        semantic_hash: hash(9),
        time_basis: Id("clock/test"),
        observed_at_tick: 1,
        valid_until_tick: 100,
    }];
    let nodes = [ResolvedPlanNode {
        instance: InstancePath::new("greeting").unwrap(),
        contract: PinnedDescriptor {
            id: Id("std/literal"),
            schema_version: 0,
            semantic_hash: hash(2),
        },
        implementation: PinnedDescriptor {
            id: Id("std/literal.native"),
            schema_version: 0,
            semantic_hash: hash(3),
        },
        lifecycle_policy: PinnedDescriptor {
            id: Id("conduit/lifecycle"),
            schema_version: 0,
            semantic_hash: hash(4),
        },
        execution_profile: None,
        artifact: Id("artifact/literal"),
        host_observation: Id("report/browser"),
        host: Id("host/browser"),
        allocation: PlanResourceBudget::ZERO,
        required_resources: &[],
        required_effects: &[],
    }];
    let clocks = [Id("clock/device")];
    let value_envelopes = [ValueEnvelopePolicy {
        cord: Id("cord/value"),
        representation: PinnedDescriptor {
            id: Id("representation/bytes"),
            schema_version: 0,
            semantic_hash: hash(10),
        },
        maximum_payload_bytes: 64,
        maximum_envelope_bytes: 32,
        maximum_fragments: 2,
        maximum_fragment_bytes: 32,
        maximum_timestamps: 1,
        clock_domains: &clocks,
        identity_allowed: true,
        correlation_allowed: true,
        causation_allowed: true,
        provenance_allowed: true,
        sensitivity_ceiling: Sensitivity::Restricted,
    }];
    let clock_conversions = [PlanClockConversion {
        id: Id("conversion/device-host"),
        source: clocks[0],
        destination: Id("clock/host"),
        numerator: 1,
        denominator: 1,
        offset_ticks: 2,
        rounding: ClockRounding::Exact,
        maximum_uncertainty_ticks: 1,
        observed_at: AuthorityTime {
            basis: Id("clock/test"),
            tick: 10,
        },
        valid_until_tick: 20,
        authority: Id("host/browser"),
    }];
    let feedback_boundaries = [PlanFeedbackBoundary {
        id: Id("feedback/state"),
        node: nodes[0].instance,
        cord: Id("cord/value"),
        kind: FeedbackBoundaryKind::State,
        initialization: FeedbackInitialization::InitialValue,
        initial_items: 1,
        initial_bytes: 8,
        maximum_retained_items: 1,
        maximum_retained_bytes: 64,
        delay_ticks: 0,
        clock: None,
        replay_gap: FeedbackReplayGapPolicy::Fail,
        cancellation: PinnedDescriptor {
            id: Id("cancellation/bounded"),
            schema_version: 0,
            semantic_hash: hash(11),
        },
        terminal: FeedbackTerminalPolicy::DropRetained,
    }];
    let resource = ResourceRef {
        kind: Id("fixture/file"),
        id: Id("fixture/output"),
    };
    let lease = ResourceLeaseContract {
        schema_version: 0,
        id: Id("lease/output"),
        resource_binding: Id("resource/output"),
        holder: nodes[0].instance,
        run: Id("run/patchbay"),
        epoch: 2,
        scope: Id("scope/write"),
        sharing: ResourceSharingMode::Exclusive,
        reservation: PlanResourceBudget {
            memory_bytes: 16,
            ..PlanResourceBudget::ZERO
        },
        time_basis: Id("clock/test"),
        issued_at_tick: 1,
        expires_at_tick: 20,
        revocation_grace_ticks: 2,
        cleanup_ticks: 4,
        maximum_operations: 1,
        maximum_evidence_events: 4,
        cleanup_escalation: PinnedDescriptor {
            id: Id("cleanup/force-close"),
            schema_version: 0,
            semantic_hash: hash(12),
        },
        foreign_retention: ForeignRetention::Unsupported,
    };
    let resources = [PlanResourceBinding {
        id: Id("resource/output"),
        node: nodes[0].instance,
        resource,
        host_observation: hosts[0].id,
        lease: Some(lease),
    }];
    let effect = EffectRequirement {
        id: Id("effect/write"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("file/write"),
        resource: ResourceSelector::Exact(resource),
        requester: nodes[0].instance,
        audience: Id("run/patchbay"),
        constraints: &[],
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id("capability/write"),
        action: effect.action,
        resource,
        host: nodes[0].host,
        time_basis: Id("clock/test"),
        observed_at_tick: 1,
        valid_until_tick: 20,
    };
    let grant = AuthorityGrant {
        id: Id("grant/write"),
        action: effect.action,
        resource,
        scope: AuthorityScope {
            root: nodes[0].instance,
            descendants: false,
        },
        audience: effect.audience,
        constraints: &[],
        time_basis: Id("clock/test"),
        not_before_tick: 1,
        expires_at_tick: 20,
        issued_for_host: nodes[0].host,
        delegation: DelegationPolicy::None,
        audit_id: Id("audit/write"),
        terminal_policy: StopPolicy::Abort,
    };
    let authorities = [PlanAuthority {
        node: nodes[0].instance,
        effect_hash: hash(13),
        grant_hash: hash(14),
        effect,
        capability,
        grant,
        binding: ResolvedAuthorityBinding {
            effect_id: effect.id,
            capability_id: capability.id,
            grant_id: grant.id,
            resource,
            host: nodes[0].host,
            audit_id: grant.audit_id,
            time_basis: Id("clock/test"),
            validated_at_tick: 10,
            check_at_use: true,
        },
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
        commit_profile: Some(EffectCommitProfile {
            schema_version: 0,
            id: Id("commit/write"),
            operation: effect.action,
            resource_lease: lease.id,
            commit_boundary: PinnedDescriptor {
                id: Id("commit/fsync"),
                schema_version: 0,
                semantic_hash: hash(15),
            },
            idempotency: EffectIdempotency::ReconcileBeforeRetry,
            unknown_commit: UnknownCommitPolicy::Reconcile,
            discontinuity: EffectDiscontinuity::ReconcileRequired,
            cleanup: PinnedDescriptor {
                id: Id("cleanup/unlink"),
                schema_version: 0,
                semantic_hash: hash(16),
            },
            maximum_attempts: 2,
            evidence_events_per_attempt: 2,
        }),
    }];
    let workload_budget = conduit_core::WorkloadBudget {
        work_units: conduit_core::WorkloadLimit::Finite(100),
        tasks: conduit_core::WorkloadLimit::Finite(1),
        processes: conduit_core::WorkloadLimit::Unsupported,
        descriptors: conduit_core::WorkloadLimit::Finite(1),
        connections: conduit_core::WorkloadLimit::Unsupported,
        storage_bytes: conduit_core::WorkloadLimit::Finite(1024),
        device_operations: conduit_core::WorkloadLimit::Unsupported,
        network_bytes: conduit_core::WorkloadLimit::Unsupported,
        callbacks: conduit_core::WorkloadLimit::Finite(2),
        foreign_queue_items: conduit_core::WorkloadLimit::Finite(1),
        transition_overlap_work_units: conduit_core::WorkloadLimit::Finite(20),
    };
    let workloads = [conduit_core::PlanWorkload {
        contract: conduit_core::WorkloadContract {
            schema_version: conduit_core::WORKLOAD_CONTRACT_SCHEMA_VERSION,
            id: Id("workload/greeting"),
            service: Id("service/greeting"),
            node: nodes[0].instance,
            guarantee: conduit_core::WorkloadGuarantee::Hard,
            budget: workload_budget,
            deadline: Some(conduit_core::DeadlineContract {
                time_basis: Id("clock/test"),
                relative_deadline_ticks: 5,
                maximum_jitter_ticks: 1,
            }),
            maximum_evidence_events: 4,
        },
        capability: conduit_core::WorkloadCapability {
            id: Id("capability/greeting-deadline"),
            identity: hash(17),
            host_observation: hosts[0].id,
            evidence_kind: conduit_core::WorkloadEvidenceKind::ExactEnforcement,
            time_basis: Id("clock/test"),
            observed_at_tick: 1,
            valid_until_tick: 20,
            capacity: conduit_core::WorkloadBudget {
                work_units: conduit_core::WorkloadLimit::Finite(200),
                ..workload_budget
            },
            maximum_deadline_ticks: 10,
            maximum_jitter_ticks: 1,
        },
    }];
    let plan = ExecutionPlan {
        schema_version: 0,
        identity: hash(1),
        source_semantic_hash: hash(5),
        resolver: PinnedDescriptor {
            id: Id("resolver/test"),
            schema_version: 0,
            semantic_hash: hash(6),
        },
        resolver_policy_hash: hash(7),
        created_at: AuthorityTime {
            basis: Id("clock/test"),
            tick: 10,
        },
        budget: PlanResourceBudget::ZERO,
        host_observations: &hosts,
        resources: &resources,
        workloads: &workloads,
        artifacts: &[],
        nodes: &nodes,
        cords: &[],
        value_envelopes: &value_envelopes,
        clock_conversions: &clock_conversions,
        feedback_boundaries: &feedback_boundaries,
        distributed_cords: &[],
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

    let projection = PlanSnapshot::from_exact_plan(&plan);
    let binding = &projection.bindings[0];
    assert_eq!(binding.availability_state, "bound-in-this-plan");
    assert_eq!(binding.reason_code, "CND-AVL-004");
    assert_eq!(binding.contract_id, "std/literal");
    assert_eq!(binding.implementation_id, "std/literal.native");
    assert_eq!(binding.host_id, "host/browser");
    assert_eq!(binding.host_observation_id, "report/browser");
    assert_eq!(binding.host_observation_identity, hash(9).to_string());
    assert_eq!(projection.value_envelopes[0].cord, "cord/value");
    assert_eq!(
        projection.value_envelopes[0].sensitivity_ceiling,
        "restricted"
    );
    assert_eq!(projection.clock_conversions[0].maximum_uncertainty_ticks, 1);
    assert_eq!(projection.feedback_boundaries[0].kind, "state");
    assert_eq!(projection.feedback_boundaries[0].maximum_retained_bytes, 64);
    assert_eq!(projection.resource_leases[0].run, "run/patchbay");
    assert_eq!(projection.resource_leases[0].cleanup_ticks, 4);
    assert_eq!(
        projection.effect_commit_profiles[0].commit_boundary_id,
        "commit/fsync"
    );
    assert_eq!(
        projection.effect_commit_profiles[0].unknown_commit,
        "reconcile"
    );
    assert_eq!(projection.workloads[0].guarantee, "hard");
    assert_eq!(projection.workloads[0].budget.processes, None);
    assert_eq!(projection.workloads[0].evidence_kind, "exact-enforcement");
}

#[test]
fn workspace_semantic_does_not_emit_contract_only_by_default() {
    let workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let semantic = workspace.semantic();
    assert!(
        semantic
            .availabilities
            .iter()
            .any(|availability| availability.availability_state == "unsupported")
    );
    assert!(
        semantic
            .availabilities
            .iter()
            .all(|availability| availability.availability_state != "contract-only")
    );
}

#[test]
fn typed_source_edits_are_atomic_and_history_is_finite() {
    let source = "panel 0\n\
node greeting : std/literal {\n\
  value = \"hello\"\n\
}\n\
node output : display/text\n";
    let mut workspace =
        Workspace::new_with_history("tour/typed", source, 3).expect("source parses");
    let configured = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::SetConfig {
                node_id: "greeting".to_owned(),
                key: "value".to_owned(),
                value: conduit_patchbay::EditValue::Text("goodbye".to_owned()),
            }],
        ))
        .expect("typed config applies");
    assert!(configured.source.source.contains("value = \"goodbye\""));
    assert_eq!(
        configured.disposition,
        conduit_patchbay::EditDisposition::Committed
    );
    assert!(configured.compatibility.compatible);

    let connected = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::Connect {
                from_node: "greeting".to_owned(),
                from_port: "value".to_owned(),
                to_node: "output".to_owned(),
                to_port: "text".to_owned(),
                bounds: conduit_patchbay::CordEditBounds {
                    capacity_items: 1,
                    max_value_bytes: 64,
                    max_queued_bytes: 64,
                    low_watermark_items: 0,
                    high_watermark_items: 1,
                    pressure: "block".to_owned(),
                },
            }],
        ))
        .expect("bounded connection applies");
    let connected_panel = conduit_panel::parse(&connected.source.source).unwrap();
    assert_eq!(connected_panel.cords.len(), 1);
    let cord_id = connected_panel.cords[0].id.clone();
    let disconnected = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::Disconnect { cord_id }],
        ))
        .expect("typed disconnect applies");
    assert!(
        conduit_panel::parse(&disconnected.source.source)
            .unwrap()
            .cords
            .is_empty()
    );

    for x in 0..4 {
        workspace
            .apply(request(
                &workspace,
                vec![EditOperation::MoveNode {
                    node_id: "greeting".to_owned(),
                    position: NodePosition { x, y: 0 },
                }],
            ))
            .expect("move applies");
    }
    assert_eq!(workspace.history().len(), 3);
}

#[test]
fn invalid_typed_edits_do_not_mutate_the_workspace() {
    let mut workspace = Workspace::new("tour/negative", SOURCE).expect("source parses");
    let before = workspace.source().clone();
    let invalid_span = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::SetConfig {
                node_id: "output".to_owned(),
                key: "missing".to_owned(),
                value: conduit_patchbay::EditValue::Text("no".to_owned()),
            }],
        ))
        .expect_err("missing config is rejected");
    assert_eq!(invalid_span.code, "CND-PBY-012");
    assert_eq!(workspace.source(), &before);

    let unbounded = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::Connect {
                from_node: "greeting".to_owned(),
                from_port: "value".to_owned(),
                to_node: "output".to_owned(),
                to_port: "text".to_owned(),
                bounds: conduit_patchbay::CordEditBounds {
                    capacity_items: 0,
                    max_value_bytes: 64,
                    max_queued_bytes: 64,
                    low_watermark_items: 0,
                    high_watermark_items: 0,
                    pressure: "block".to_owned(),
                },
            }],
        ))
        .expect_err("unbounded connection is rejected");
    assert_eq!(unbounded.code, "CND-PBY-010");
    assert_eq!(workspace.source(), &before);
}

#[test]
fn supervision_projection_retains_exact_origins_actions_rejection_and_gap() {
    use conduit_core::{
        AdmittedSupervisionAction, EvidenceCursor, EvidenceCursorStatus, Id, InstancePath,
        RecoveryBudget, RetryDeclaration, SemanticHash, StopPolicy, SupervisionActionKind,
        SupervisionContract, SupervisionEvidence, SupervisionEvidenceKind, SupervisionFailureMode,
        SupervisionLimits, SupervisionReason, SupervisionScope, TerminalCauseCode, TerminalClass,
        TerminalContext, TerminalObservation, TerminalPhase,
    };
    let actions = [AdmittedSupervisionAction {
        kind: SupervisionActionKind::ActivateDeclaredFallback,
        target: Some(Id("fallback")),
        maximum_uses: 1,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: true,
    }];
    let contract = SupervisionContract {
        schema_version: 0,
        id: Id("supervision.subject"),
        scope: SupervisionScope::Child,
        subject: InstancePath::new("root/subject").unwrap(),
        handler: InstancePath::new("root/handler").unwrap(),
        members: &[],
        failure_mode: SupervisionFailureMode::FailTogether,
        outer: None,
        actions: &actions,
        limits: SupervisionLimits {
            maximum_observations: 2,
            maximum_decisions: 2,
            maximum_in_flight: 1,
            maximum_cause_depth: 2,
            maximum_nested_depth: 2,
            maximum_handler_ticks: 8,
            maximum_recovery_ticks: 16,
            restart_window_ticks: 8,
            backoff_ticks: 2,
            cooldown_ticks: 2,
            operator_wait_ticks: 8,
            maximum_evidence_events: 8,
            observation_bytes: 256,
            decision_bytes: 64,
            scratch_bytes: 64,
        },
        cleanup: StopPolicy::Abort,
        required_behavior: true,
    };
    let observation = TerminalObservation {
        semantic_subject: contract.subject,
        expanded_subject: InstancePath::new("root/subject-3").unwrap(),
        run: Id("run-1"),
        plan_identity: SemanticHash::from_bytes([7; 32]),
        plan_epoch: 4,
        generation: 3,
        attempt: 2,
        class: TerminalClass::Failed,
        code: TerminalCauseCode::NodeFailed,
        phase: TerminalPhase::Step,
        caused_by: &[],
        retry: RetryDeclaration::RestartOnly,
        context: TerminalContext {
            resource: Some(Id("gpu")),
            host: Some(Id("browser")),
            artifact: Some(Id("worker")),
            ..TerminalContext::default()
        },
        evidence: EvidenceCursor {
            stream: Id("evidence-run-1"),
            sequence: 17,
        },
        budget: RecoveryBudget {
            remaining_observations: 1,
            remaining_decisions: 1,
            remaining_attempts: 0,
            remaining_evidence_events: 2,
            now_tick: 10,
            deadline_tick: 20,
        },
    };
    let evidence = [SupervisionEvidence {
        sequence: 18,
        kind: SupervisionEvidenceKind::DecisionRejected,
        action_index: Some(0),
        reason: Some(SupervisionReason::CandidateEpochRequired),
    }];
    let projection = project_supervision(
        "sha256:source",
        observation,
        contract,
        &evidence,
        EvidenceCursorStatus::Gap { resume_at: 15 },
    );
    assert_eq!(projection.semantic_subject, "root/subject");
    assert_eq!(projection.expanded_subject, "root/subject-3");
    assert_eq!(projection.plan_epoch, 4);
    assert_eq!(projection.evidence_gap_resume_at, Some(15));
    assert_eq!(projection.actions[0].target.as_deref(), Some("fallback"));
    assert!(projection.actions[0].requires_new_epoch);
    assert_eq!(
        projection
            .latest_evidence
            .unwrap()
            .rejection_code
            .as_deref(),
        Some("CND-SUP-013")
    );
}

#[test]
fn pool_projection_retains_plan_run_generation_and_evidence_origins() {
    use conduit_core::{
        InstancePath, PlanResourceBudget, PoolAdmissionDisposition, PoolAdmissionFacts,
        PoolAdmissionPolicy, PoolCleanupPolicy, PoolContract, PoolController, PoolGeneration,
        PoolReservationProfile, PoolSupervisionPolicy, PoolWorkIdentity, SemanticHash,
    };
    let hash = |byte| SemanticHash::from_bytes([byte; 32]);
    let reservation = PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 128,
            timers: 2,
            evidence_bytes: 64,
            ..PlanResourceBudget::ZERO
        },
        child_nodes: 2,
        child_cords: 1,
        state_bytes: 32,
        scheduler_slots: 3,
        host_operations: 1,
        cancellation_scopes: 2,
    };
    let contract = PoolContract {
        pool: InstancePath::new("root/pool.workers").unwrap(),
        template_hash: hash(1),
        implementation_set_hash: hash(6),
        maximum_live: 1,
        maximum_queued: 0,
        admission: PoolAdmissionPolicy::Reject,
        supervision: PoolSupervisionPolicy::Isolate,
        cleanup: PoolCleanupPolicy::Abort,
        deadline_ticks: 100,
        idle_timeout_ticks: 20,
        cleanup_ticks: 5,
        reservation,
        total_reservation: reservation.checked_mul(3).unwrap(),
        maximum_evidence_events: 16,
    };
    let generation = PoolGeneration {
        plan: hash(2),
        epoch: 4,
        generation: 3,
        template_hash: hash(1),
    };
    let mut runtime = PoolController::<1, 16>::new(contract, generation).unwrap();
    let PoolAdmissionDisposition::Started { slot } = runtime
        .offer(
            PoolWorkIdentity {
                request: hash(3),
                work_unit: hash(4),
                correlation: hash(5),
            },
            PoolAdmissionFacts {
                authority_granted: true,
                sensitivity_allowed: true,
                template_hash: hash(1),
                implementation_set_hash: hash(6),
                available: reservation,
            },
            10,
        )
        .unwrap()
    else {
        panic!("pool starts")
    };
    runtime.mark_running(slot, 11).unwrap();
    let projection = project_pool(PoolProjectionInput {
        source_semantic_hash: "sha256:source",
        plan_identity: hash(2),
        plan_epoch: 4,
        run_id: "run-1",
        evidence_stream_id: "evidence/run-1",
        generation,
        generation_identity: runtime.generation_identity(),
        contract,
        population: runtime.population(),
        evidence: runtime.evidence(),
    });
    assert_eq!(projection.source_semantic_hash, "sha256:source");
    assert_eq!(projection.run_id, "run-1");
    assert_eq!(projection.pool, "root/pool.workers");
    assert_eq!(projection.generation, 3);
    assert_eq!(projection.live, 1);
    assert_eq!(projection.evidence_cursor, 2);
    assert_eq!(projection.latest_evidence.unwrap().to, "running");
}
