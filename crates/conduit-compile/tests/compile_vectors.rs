use std::collections::{BTreeMap, BTreeSet};

use bumpalo::Bump;
use conduit_compile::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, ExactPlanDocument, ExecutionLaneObservationDocument,
    ExecutionLimitsDocument, ExecutionPlacementObservationDocument, ExecutionProfileDocument,
    HostReportDocument, ImplementationDocument, InstalledProfile, MemoryClaimDocument, PinDocument,
    WatchAdmissionDocument, builtin_catalog_document, compile_source,
    fixed_hosted_execution_arrangement_policy,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactDigest, CAPABILITY_REPORT_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION, EvidenceCursorStatus, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION,
    ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::{
    AvailabilityState, ExactHostedBinding, ExactHostedBindings, ExactRunContext, ExactRunIo,
    ExactRunSessionRegistry, ExactWatchMaterial, ExactWatchOperation, ExactWatchUseAuthority,
    HostedPrimitiveImplementation, Registry, RunIo, SchedulerReservation,
};

const FIXTURE: &str = include_str!("../../../conformance/c5/compile-package.json");
const SOURCE_LIMIT_FIXTURE: &str =
    include_str!("../../../conformance/c5/compile-source-limits.json");
const SOURCE: &str = "panel 0\n\
source: std/literal { value = \"hello\" }\n\
upper: text/uppercase using ready\n\
sink: display/text\n\
source.value > upper.text\n\
upper.text > sink.text\n";

fn hash(byte: u8) -> String {
    SemanticHash::from_bytes([byte; 32]).to_string()
}

fn pin(id: &str, byte: u8) -> PinDocument {
    PinDocument {
        id: id.to_owned(),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn profile(ordinal: u8) -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: format!("fixture/execution-profile-{ordinal}"),
        schema_version: 0,
        semantic_hash: hash(30),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 4,
            max_input_leases: 1,
            max_input_bytes: 1024,
            max_output_reservations: 1,
            max_output_bytes: 1024,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_host_buffer_bytes: 1024,
            implementation_memory_bytes: 3 * 1024,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 1024,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 2048,
            },
        ],
        checkpoint: None,
    }
}

fn candidate(ordinal: u8, contract_id: &str, contract_hash: SemanticHash) -> CandidateDocument {
    let artifact_id = format!("fixture/artifact-{ordinal}");
    let artifact_digest = ArtifactDigest::from_bytes([ordinal; 32]).to_string();
    CandidateDocument {
        implementation: ImplementationDocument {
            schema_version: IMPLEMENTATION_MANIFEST_SCHEMA_VERSION,
            identity: String::new(),
            id: format!("fixture/implementation-{ordinal}"),
            implementation_version: "1.0.0".to_owned(),
            semantic_contract: PinDocument {
                id: contract_id.to_owned(),
                schema_version: 0,
                semantic_hash: contract_hash.to_string(),
            },
            executor: "native-in-process".to_owned(),
            entrypoint_name: "run".to_owned(),
            entrypoint_adapter: "conduit/native-step".to_owned(),
            entrypoint_abi: "conduit/native".to_owned(),
            runtime_protocol_version: 0,
            execution_profile: pin("fixture/execution-profile", 30),
            artifacts: vec![ArtifactReferenceDocument {
                id: artifact_id.clone(),
                digest: artifact_digest.clone(),
                role: "implementation".to_owned(),
                required: true,
            }],
            required_interfaces: Vec::new(),
            provided_interfaces: Vec::new(),
            required_authorities: Vec::new(),
            required_effects: Vec::new(),
            minimum_plan_version: 0,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            minimum_runtime_protocol: 1,
            maximum_runtime_protocol: 1,
            coexistence_memory_bytes: 0,
        },
        execution_profile: profile(ordinal),
        artifacts: vec![ArtifactDocument {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            identity: String::new(),
            id: artifact_id,
            digest: artifact_digest,
            media_type: "application/octet-stream".to_owned(),
            byte_size: 1,
            target: None,
            abi: None,
            builder: "fixture/builder".to_owned(),
            source_digest: ArtifactDigest::from_bytes([40; 32]).to_string(),
            build_recipe_digest: ArtifactDigest::from_bytes([41; 32]).to_string(),
            reproducible: false,
            license_expressions: vec!["MIT".to_owned()],
        }],
        host_report: HostReportDocument {
            schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
            identity: String::new(),
            id: format!("fixture/report-{ordinal}"),
            host: "fixture/host-local".to_owned(),
            boot_id: "fixture/host-local-boot".to_owned(),
            reporter: pin("fixture/reporter", 50),
            trust: pin("fixture/report-trust", 51),
            membership: None,
            time_basis: "clock/compile".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
            available: BudgetDocument {
                memory_bytes: 2 * 1024 * 1024,
                storage_bytes: 4096,
                cpu_units: 16,
                timers: 4,
                transports: 4,
                checkpoints: 4,
                evidence_bytes: 4096,
            },
            capabilities: Vec::new(),
            resources: Vec::new(),
            topology: Vec::new(),
            execution_placements: vec![ExecutionPlacementObservationDocument {
                id: format!("placement/compile-{ordinal}"),
                provider: pin("provider/fixed-hosted-lanes", 52),
                authority_boundary: pin("boundary/compile-authority", 53),
                resource_boundary: pin("boundary/compile-resource", 54),
                lifecycle_boundary: pin("boundary/compile-lifecycle", 55),
                failure_boundary: pin("boundary/compile-failure", 56),
                generation: 1,
                isolation: "step-native".to_owned(),
                memory_containment: "observed".to_owned(),
                regain_control: "observed".to_owned(),
                effect_fencing: "unsupported".to_owned(),
                stop_execution: "unsupported".to_owned(),
                reclaim_resources: "unsupported".to_owned(),
                maximum_regain_control_ticks: 0,
            }],
            execution_lanes: vec![ExecutionLaneObservationDocument {
                id: format!("lane/compile-{ordinal}"),
                placement: format!("placement/compile-{ordinal}"),
                placement_generation: 1,
                generation: 1,
                independent_progress: "guaranteed".to_owned(),
                simultaneous_execution: "guaranteed".to_owned(),
                preemption: "observed".to_owned(),
                termination: "unsupported".to_owned(),
                ready_slots: 64,
                wake_slots: 64,
                proposal_slots: 64,
                commit_slots: 64,
                timer_slots: 4,
                scratch_bytes: 128 * 1024,
                stack_bytes: 64 * 1024,
                evidence_slots: 512,
            }],
            supported_executors: vec!["native-in-process".to_owned()],
            supported_targets: Vec::new(),
            supported_abis: Vec::new(),
            minimum_plan_version: 0,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            current_constraints: Vec::new(),
        },
        allocation: BudgetDocument {
            memory_bytes: 3 * 1024,
            cpu_units: 1,
            ..BudgetDocument::default()
        },
        lifecycle_policy: pin("conduit/finite-lifecycle", 60),
        capabilities: Vec::new(),
        resources: Vec::new(),
        topology: Vec::new(),
        granted_authorities: Vec::new(),
        authorities: Vec::new(),
    }
}

fn input(source: &str) -> CompileInput {
    let panel = parse(source).unwrap();
    let mut executable = panel.clone();
    for node in &mut executable.nodes {
        if node.constraint.as_deref() == Some("ready") {
            node.constraint = None;
            node.constraint_span = None;
        }
    }
    let topology = Registry::compatibility_demo()
        .resolve(&executable)
        .unwrap()
        .exact_topology()
        .unwrap();
    let mut contracts = BTreeMap::new();
    for node in &topology.nodes {
        contracts
            .entry(node.contract_id.clone())
            .or_insert(node.contract_hash);
    }
    let candidates = contracts
        .into_iter()
        .enumerate()
        .map(|(index, (id, hash))| candidate(index as u8 + 1, &id, hash))
        .collect();
    let mut input = CompileInput {
        schema: COMPILE_INPUT_SCHEMA.to_owned(),
        schema_version: COMPILE_INPUT_SCHEMA_VERSION,
        identity: String::new(),
        entry_uri: "mem://compile/entry.panel".to_owned(),
        selected_root: panel.selected_root,
        source_limits: CompileSourceLimits::default(),
        modules: vec![CompileModuleDocument {
            canonical_uri: "mem://compile/entry.panel".to_owned(),
            content_hash: String::new(),
            source: source.to_owned(),
        }],
        catalog: builtin_catalog_document().unwrap(),
        pool_bindings: Vec::new(),
        supervision_bindings: Vec::new(),
        hazard_closure: None,
        distribution: None,
        evidence_provider: None,
        watch_admissions: Vec::new(),
        source_semantic_hash: topology.source_semantic_hash.to_string(),
        resolver: pin("conduit/exact-compiler-resolver", 70),
        resolver_policy_hash: String::new(),
        time_basis: "clock/compile".to_owned(),
        current_tick: 12,
        plan_budget: BudgetDocument {
            memory_bytes: 2 * 1024 * 1024,
            storage_bytes: 16 * 1024,
            cpu_units: 64,
            timers: 16,
            transports: 16,
            checkpoints: 16,
            evidence_bytes: 16 * 1024,
        },
        execution_arrangement: fixed_hosted_execution_arrangement_policy(),
        maximum_authority_bindings: 64,
        maximum_transition_memory_bytes: 1024 * 1024,
        maximum_search_states: 128,
        tie_policy: "lowest-canonical-identity".to_owned(),
        required_realm: None,
        trusted_entities: Vec::new(),
        trusted_status_reporters: Vec::new(),
        require_active_passport: false,
        implementation_preference: Vec::new(),
        candidates,
    };
    input.seal().unwrap();
    input
}

fn compile_case(id: &str) {
    match id {
        "deterministic-explicit-compile" => {
            let sealed = input(SOURCE);
            let first = compile_source(SOURCE, &sealed).unwrap();
            let second = compile_source(SOURCE, &sealed).unwrap();
            assert_eq!(
                serde_json::to_vec(&first).unwrap(),
                serde_json::to_vec(&second).unwrap()
            );
        }
        "using-ready-is-resolved" => {
            let plan = compile_source(SOURCE, &input(SOURCE)).unwrap();
            assert!(plan.unresolved_selectors.is_empty());
            assert_eq!(plan.nodes.len(), 3);
            assert_eq!(plan.schema, "conduit.execution-plan");
            assert!(
                plan.nodes
                    .iter()
                    .all(|node| node.execution_profile.semantic_hash.starts_with("sha256:"))
            );
        }
        "unresolved-selector-rejected" => {
            let source = SOURCE.replace("using ready", "using unavailable");
            let mut sealed = input(SOURCE);
            sealed.modules[0].source.clone_from(&source);
            sealed.seal().unwrap();
            assert_eq!(
                compile_source(&source, &sealed).unwrap_err().code(),
                "CND-CMP-005"
            );
        }
        "absent-or-stale-host-report" => {
            let mut sealed = input(SOURCE);
            for candidate in &mut sealed.candidates {
                candidate.host_report.valid_until_tick = 11;
            }
            sealed.seal().unwrap();
            assert_eq!(
                compile_source(SOURCE, &sealed).unwrap_err().code(),
                "CND-CMP-006"
            );
        }
        "missing-or-incompatible-implementation-artifact" => {
            let mut sealed = input(SOURCE);
            sealed.candidates[0].artifacts[0].target = Some("linux/x86_64".to_owned());
            sealed.seal().unwrap();
            assert_eq!(
                compile_source(SOURCE, &sealed).unwrap_err().code(),
                "CND-CMP-006"
            );
        }
        "memory-resource-authority-transition-over-budget" => {
            let mut sealed = input(SOURCE);
            sealed.plan_budget.memory_bytes = 1;
            sealed.seal().unwrap();
            assert_eq!(
                compile_source(SOURCE, &sealed).unwrap_err().code(),
                "CND-CMP-007"
            );
        }
        "no-provisioning-or-implicit-fetch" => {
            let sealed = input(SOURCE);
            assert!(sealed.modules.iter().all(|module| {
                module.canonical_uri.starts_with("mem://")
                    && !module.source.contains("http://")
                    && !module.source.contains("https://")
            }));
            compile_source(SOURCE, &sealed).unwrap();
        }
        "minimal-plan-round-trip" => {
            let plan = compile_source(SOURCE, &input(SOURCE)).unwrap();
            let bytes = serde_json::to_vec(&plan).unwrap();
            let decoded: ExactPlanDocument = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(decoded, plan);
            decoded.validate().unwrap();
        }
        other => panic!("unhandled compile vector {other}"),
    }
}

#[test]
fn every_compile_vector_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let compile_ids = cases
        .iter()
        .filter(|case| case["runner"] == "compile")
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(compile_ids.len(), 8);
    assert_eq!(
        compile_ids.iter().copied().collect::<BTreeSet<_>>().len(),
        compile_ids.len()
    );
    for id in compile_ids {
        compile_case(id);
    }
}

#[test]
fn compile_input_rejects_displaced_host_report_without_execution_observations() {
    let encoded = serde_json::to_value(input(SOURCE)).unwrap();
    let mut without_placements = encoded.clone();
    let report = without_placements["candidates"][0]["host_report"]
        .as_object_mut()
        .unwrap();
    report.remove("execution_placements");
    assert!(serde_json::from_value::<CompileInput>(without_placements).is_err());

    let mut without_lanes = encoded;
    let report = without_lanes["candidates"][0]["host_report"]
        .as_object_mut()
        .unwrap();
    report.remove("execution_lanes");
    assert!(serde_json::from_value::<CompileInput>(without_lanes).is_err());
}

#[test]
fn cross_host_compile_fails_closed_without_distributed_session_input() {
    let mut sealed = input(SOURCE);
    for (index, candidate) in sealed.candidates.iter_mut().enumerate() {
        candidate.host_report.host = format!("fixture/host-{index}");
    }
    sealed.seal().unwrap();
    assert_eq!(
        compile_source(SOURCE, &sealed).unwrap_err().code(),
        "CND-CMP-008"
    );
}

fn compile_source_limit_case(id: &str) -> serde_json::Value {
    let source_len = u64::try_from(SOURCE.len()).unwrap();
    let mut sealed = input(SOURCE);
    match id {
        "oversized-explicit-module-is-rejected" => {
            sealed.source_limits = CompileSourceLimits {
                maximum_entry_source_bytes: source_len,
                maximum_module_source_bytes: source_len,
                maximum_module_closure_bytes: source_len * 3,
                maximum_modules: 2,
            };
            sealed.modules.push(CompileModuleDocument {
                canonical_uri: "mem://compile/oversized.panel".to_owned(),
                content_hash: String::new(),
                source: "x".repeat(usize::try_from(source_len + 1).unwrap()),
            });
        }
        "aggregate-module-closure-limit-is-enforced" => {
            sealed.source_limits = CompileSourceLimits {
                maximum_entry_source_bytes: source_len,
                maximum_module_source_bytes: source_len,
                maximum_module_closure_bytes: source_len * 2 - 1,
                maximum_modules: 2,
            };
            sealed.modules.push(CompileModuleDocument {
                canonical_uri: "mem://compile/aggregate.panel".to_owned(),
                content_hash: String::new(),
                source: SOURCE.to_owned(),
            });
        }
        "schema-one-requires-explicit-limit-migration" => {
            sealed.schema = "conduit.compile-input".to_owned();
            sealed.schema_version = 1;
        }
        other => panic!("compile source-limit case `{other}` is not implemented"),
    }
    let error = sealed.seal().unwrap_err();
    serde_json::json!({"accepted": false, "code": error.code()})
}

#[test]
fn every_compile_module_limit_vector_executes() {
    let fixture: serde_json::Value = serde_json::from_str(SOURCE_LIMIT_FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut executed = 0;
    for case in cases.iter().filter(|case| case["runner"] == "compile") {
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            compile_source_limit_case(id),
            case["expected"],
            "case `{id}`"
        );
        executed += 1;
    }
    assert_eq!(executed, 3);
}

#[test]
fn source_limits_are_part_of_compile_input_identity() {
    let original = input(SOURCE);
    let mut changed = original.clone();
    changed.source_limits.maximum_entry_source_bytes -= 1;
    changed.seal().unwrap();
    assert_ne!(original.identity, changed.identity);
}

#[test]
fn sealed_document_exposes_the_exact_core_plan_without_replanning() {
    let document = compile_source(SOURCE, &input(SOURCE)).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();

    assert_eq!(plan.identity.to_string(), document.identity);
    assert_eq!(
        plan.source_semantic_hash.to_string(),
        document.source_semantic_hash
    );
    assert_eq!(plan.nodes.len(), document.nodes.len());
    assert_eq!(plan.cords.len(), document.cords.len());
    assert_eq!(
        document.execution_arrangement.plan_identity,
        document.identity
    );
    assert_eq!(
        document.execution_arrangement.regions.len(),
        plan.nodes.len()
    );
    assert_eq!(
        document.execution_arrangement.boundaries.len(),
        plan.cords.len()
    );
    assert!(
        document
            .execution_arrangement
            .regions
            .iter()
            .all(|region| region.independent)
    );
    document
        .execution_arrangement()
        .unwrap()
        .validate_for_plan(&plan)
        .unwrap();
    for (planned, encoded) in plan.nodes.iter().zip(&document.nodes) {
        assert_eq!(planned.contract.id.as_str(), encoded.contract.id);
        assert_eq!(
            planned.implementation.id.as_str(),
            encoded.implementation.id
        );
        assert_eq!(planned.artifact.as_str(), encoded.artifact);
        assert_eq!(planned.host.as_str(), encoded.host);
        assert_eq!(planned.host_observation.as_str(), encoded.host_observation);
    }
}

#[test]
fn physical_arrangement_is_required_and_tampering_fails_validation() {
    let document = compile_source(SOURCE, &input(SOURCE)).unwrap();
    let mut encoded = serde_json::to_value(&document).unwrap();
    encoded
        .as_object_mut()
        .unwrap()
        .remove("execution_arrangement");
    assert!(serde_json::from_value::<ExactPlanDocument>(encoded).is_err());

    let mut changed = document;
    changed.execution_arrangement.lanes[0].generation += 1;
    assert_eq!(changed.validate().unwrap_err().code(), "CND-CMP-010");
}

#[test]
fn physical_lane_capacity_and_observed_simultaneity_fail_closed() {
    const TWO_BRANCHES: &str = "panel 0\n\
left: std/literal { value = \"left\" }\n\
left_sink: display/text\n\
right: std/literal { value = \"right\" }\n\
right_sink: display/text\n\
left.value > left_sink.text\n\
right.value > right_sink.text\n";
    let mut exhausted = input(TWO_BRANCHES);
    for candidate in &mut exhausted.candidates {
        candidate.host_report.execution_lanes[0].proposal_slots = 1;
        candidate.host_report.execution_lanes[0].commit_slots = 1;
    }
    exhausted.seal().unwrap();
    assert_eq!(
        compile_source(TWO_BRANCHES, &exhausted).unwrap_err().code(),
        "CND-CMP-010"
    );

    let mut observed_only = input(SOURCE);
    for candidate in &mut observed_only.candidates {
        candidate.host_report.execution_lanes[0].independent_progress = "observed".to_owned();
        candidate.host_report.execution_lanes[0].simultaneous_execution = "observed".to_owned();
    }
    observed_only.seal().unwrap();
    let serial = compile_source(SOURCE, &observed_only).unwrap();
    assert!(
        serial
            .execution_arrangement
            .regions
            .iter()
            .all(|region| !region.independent)
    );
}

#[test]
fn sealed_document_drives_the_exact_hosted_executor() {
    let source = SOURCE.replace(" using ready", "");
    let panel = parse(&source).unwrap();
    let registry = Registry::compatibility_demo();
    let resolved = registry.resolve(&panel).unwrap();
    let document = compile_source(&source, &input(&source)).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let execution_arrangement = document.execution_arrangement().unwrap();
    let binding_documents = plan
        .nodes
        .iter()
        .map(|node| {
            let artifact = plan
                .artifacts
                .iter()
                .find(|artifact| artifact.id == node.artifact)
                .unwrap();
            let implementation = match node.contract.id.as_str() {
                "std/literal" => HostedPrimitiveImplementation::Literal,
                "text/uppercase" => HostedPrimitiveImplementation::Uppercase,
                "display/text" => HostedPrimitiveImplementation::DisplayText,
                other => panic!("unexpected exact test contract `{other}`"),
            };
            ExactHostedBinding {
                implementation_id: node.implementation.id.to_string(),
                implementation_version: "1.0.0".to_owned(),
                implementation_identity: node.implementation.semantic_hash,
                artifact_id: node.artifact.to_string(),
                artifact_digest: artifact.digest,
                artifacts: vec![conduit_runtime::ManagedArtifactIdentity {
                    id: node.artifact.to_string(),
                    digest: artifact.digest.to_string(),
                }],
                implementation,
                managed_lifecycle: None,
            }
        })
        .collect::<Vec<_>>();
    let bindings = ExactHostedBindings::new(binding_documents.clone()).unwrap();
    let mut rejected_input = &b""[..];
    let mut rejected_output = Vec::new();
    let mut rejected_error = Vec::new();
    let missing_binding = resolved
        .run_exact(
            &plan,
            &ExactHostedBindings::default(),
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 1,
                run_id: conduit_core::Id("fixture/run"),
                grant_observations: &[],
                validation: conduit_core::PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 128,
                    max_tick: 256,
                    max_consecutive_yields: 8,
                    max_events: 64,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &mut RunIo {
                input: &mut rejected_input,
                output: &mut rejected_output,
                error: &mut rejected_error,
                display: &mut Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(missing_binding.code, "CND-RUN-007");

    let mut wrong_binding_documents = binding_documents;
    wrong_binding_documents[0].artifact_digest = ArtifactDigest::from_bytes([0xff; 32]);
    let wrong_bindings = ExactHostedBindings::new(wrong_binding_documents).unwrap();
    let mut rejected_input = &b""[..];
    let wrong_binding = resolved
        .run_exact(
            &plan,
            &wrong_bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 1,
                run_id: conduit_core::Id("fixture/run"),
                grant_observations: &[],
                validation: conduit_core::PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 128,
                    max_tick: 256,
                    max_consecutive_yields: 8,
                    max_events: 64,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &mut RunIo {
                input: &mut rejected_input,
                output: &mut rejected_output,
                error: &mut rejected_error,
                display: &mut Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(wrong_binding.code, "CND-RUN-008");

    let changed_source = source.replace("\"hello\"", "\"changed\"");
    let changed_panel = parse(&changed_source).unwrap();
    let changed_resolved = registry.resolve(&changed_panel).unwrap();
    let mut rejected_input = &b""[..];
    let source_mismatch = changed_resolved
        .run_exact(
            &plan,
            &bindings,
            ExactRunContext {
                semantic_source_hash: plan.source_semantic_hash,
                plan_epoch: 1,
                run_id: conduit_core::Id("fixture/run"),
                grant_observations: &[],
                validation: conduit_core::PlanValidationContext {
                    supported_schema_version: plan.schema_version,
                    now: plan.created_at,
                },
                scheduler_policy: SchedulerPolicy {
                    schema_version: SCHEDULER_CONTRACT_VERSION,
                    ready_queue: ReadyQueueDiscipline::RoundRobin,
                    max_decisions: 128,
                    max_tick: 256,
                    max_consecutive_yields: 8,
                    max_events: 64,
                },
                reservation: SchedulerReservation {
                    available_runtime_memory_bytes: plan.budget.memory_bytes,
                    executor_overhead_limit_bytes: plan.budget.memory_bytes,
                },
            },
            &mut RunIo {
                input: &mut rejected_input,
                output: &mut Vec::new(),
                error: &mut Vec::new(),
                display: &mut Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(source_mismatch.code, "CND-RUN-009");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let context = ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: 1,
        run_id: conduit_core::Id("fixture/run"),
        grant_observations: &[],
        validation: conduit_core::PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 128,
            max_tick: 256,
            max_consecutive_yields: 8,
            max_events: 64,
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    };
    let wrong_epoch = resolved
        .run_exact_report_arranged(
            &plan,
            &execution_arrangement,
            &bindings,
            ExactRunContext {
                plan_epoch: 2,
                ..context
            },
            &mut RunIo {
                input: &mut &b""[..],
                output: &mut Vec::new(),
                error: &mut Vec::new(),
                display: &mut Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(wrong_epoch.code, "CND-RUN-012");
    let report = resolved
        .run_exact_report_arranged(
            &plan,
            &execution_arrangement,
            &bindings,
            context,
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
        .unwrap();
    let summary = report.summary;

    assert_eq!(summary.nodes_completed, 3);
    assert_eq!(summary.cords_conducted, 2);
    assert!(output.is_empty());
    assert_eq!(display, b"HELLO");
    assert!(error.is_empty());
}

#[test]
fn typed_text_format_compiles_runs_cancels_and_retains_bounded_evidence() {
    const FORMAT_SOURCE: &str = include_str!("../../../examples/formatted-greeting.panel");

    let installed = InstalledProfile::observe(FORMAT_SOURCE).unwrap();
    let document = compile_source(FORMAT_SOURCE, &installed.input).unwrap();
    let arena = Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let format = plan
        .nodes
        .iter()
        .find(|node| node.contract.id.as_str() == "std/text/format")
        .unwrap();
    let profile = format.execution_profile.unwrap();
    assert_eq!(profile.limits.max_input_leases, 2);
    assert_eq!(profile.limits.max_retained_values, 3);
    assert_eq!(
        profile.limits.max_retained_bytes,
        conduit_std::FORMAT_MAX_RETAINED_BYTES as u64
    );
    assert_eq!(
        profile.limits.max_step_work,
        conduit_std::FORMAT_MAX_WORK as u32
    );
    assert_eq!(
        profile.limits.max_output_bytes,
        conduit_std::FORMAT_MAX_OUTPUT_BYTES as u64
    );
    assert_eq!(plan.cords.len(), 3);
    assert!(plan.cords.iter().all(|cord| {
        cord.flow.capacity.items() == 1
            && cord.queue_memory_bytes > 0
            && cord.flow.capacity.max_value_bytes() > 0
    }));

    let panel = parse(FORMAT_SOURCE).unwrap();
    let registry = Registry::hosted_primitives();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan).unwrap();
    let context = |run_id| ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: 121,
        run_id,
        grant_observations: &[],
        validation: conduit_core::PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 256,
            max_tick: 512,
            max_consecutive_yields: 8,
            max_events: 128,
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    };
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let report = resolved
        .run_exact_report(
            &plan,
            &bindings,
            context(conduit_core::Id("fixture/format-run")),
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
        .unwrap();
    assert!(output.is_empty());
    assert_eq!(display, b"Hello, operator. Payload: {status = ready}\n");
    assert!(error.is_empty());
    assert_eq!(report.terminal, conduit_core::TerminalClass::Succeeded);
    assert!(
        report.evidence_bytes <= plan.budget.evidence_bytes,
        "{} > {}",
        report.evidence_bytes,
        plan.budget.evidence_bytes
    );
    assert!(report.evidence.iter().any(|event| {
        event.node_id.as_deref() == Some("root/greeting")
            && event.implementation_id.as_deref() == Some(format.implementation.id.as_str())
            && event.implementation_identity.as_deref()
                == Some(format.implementation.semantic_hash.to_string().as_str())
            && event.plan_identity == plan.identity.to_string()
            && event.run_id == "fixture/format-run"
    }));
    assert_ne!(
        format.contract.semantic_hash,
        format.implementation.semantic_hash
    );
    assert_ne!(format.implementation.semantic_hash, plan.identity);

    let sessions = ExactRunSessionRegistry::new(1, plan.budget.memory_bytes).unwrap();
    let insufficient_io = resolved
        .start_exact_session(
            &plan,
            &bindings,
            context(conduit_core::Id("fixture/format-insufficient-io")),
            &sessions,
            ExactRunIo::new(0).unwrap(),
        )
        .err()
        .expect("persistent Start must reject I/O outside the exact plan capacity");
    assert_eq!(insufficient_io.code, "CND-RUN-009");
    assert_eq!(sessions.active_sessions(), 0);
    let mut session = resolved
        .start_exact_session(
            &plan,
            &bindings,
            context(conduit_core::Id("fixture/format-run")),
            &sessions,
            ExactRunIo::for_plan(&plan).unwrap(),
        )
        .unwrap();
    assert_eq!(sessions.active_sessions(), 1);
    assert_eq!(
        session.high_water().decisions,
        0,
        "start must not execute a node step"
    );
    while matches!(session.state(), conduit_runtime::ExactRunState::Active) {
        session.pump(1, &[]).unwrap();
    }
    assert_eq!(
        session.state(),
        conduit_runtime::ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
    );
    assert_eq!(session.exact_evidence(), report.evidence);
    assert!(session.finalize().is_ok());
    assert_eq!(sessions.active_sessions(), 0);
    assert_eq!(
        session.with_io(|io| io.display().to_vec()),
        b"Hello, operator. Payload: {status = ready}\n"
    );

    let mut input = &b""[..];
    let mut cancelled_output = Vec::new();
    let mut cancelled_error = Vec::new();
    let cancelled = resolved
        .cancel_exact_report(
            &plan,
            &bindings,
            context(conduit_core::Id("fixture/format-cancel")),
            conduit_core::StopPolicy::Abort,
            &mut RunIo {
                input: &mut input,
                output: &mut cancelled_output,
                error: &mut cancelled_error,
                display: &mut Vec::new(),
            },
        )
        .unwrap();
    assert_eq!(cancelled.terminal, conduit_core::TerminalClass::Cancelled);
    assert!(cancelled.evidence_bytes <= plan.budget.evidence_bytes);
}

#[test]
fn persistent_hosted_session_owns_io_after_the_plan_arena_is_released() {
    const SOURCE: &str = "panel 0\n\
source: std/literal { value = \"owned session\" }\n\
sink: display/text\n\
source.value > sink.text\n";

    let mut installed = InstalledProfile::observe(SOURCE).unwrap();
    let panel = parse(SOURCE).unwrap();
    let topology = Registry::hosted_primitives()
        .resolve(&panel)
        .unwrap()
        .exact_topology()
        .unwrap();
    let watched_cord = topology.cords[0].id.clone();
    let representation = PinDocument {
        id: topology.cords[0]
            .from_port
            .value_type
            .contract_id
            .to_string(),
        schema_version: topology.cords[0].from_port.value_type.schema_version,
        semantic_hash: topology.cords[0]
            .from_port
            .value_type
            .semantic_hash
            .to_string(),
    };
    installed.input.watch_admissions = vec![WatchAdmissionDocument {
        id: "watch/owned-source".to_owned(),
        subject_kind: "cord".to_owned(),
        operator: "operator/fixture".to_owned(),
        control_grant_hash: SemanticHash::from_bytes([91; 32]).to_string(),
        lease: "lease/watch-owned-source".to_owned(),
        cord: Some(watched_cord),
        node: None,
        port: None,
        direction: None,
        representation,
        maximum_preview_bytes: 5,
        maximum_history: 1,
        minimum_tick_interval: 1,
        retention: "latest".to_owned(),
        sensitivity_ceiling: "public".to_owned(),
        reveal_action: None,
        reveal_grant_hash: None,
    }];
    installed.input.seal().unwrap();
    let document = compile_source(SOURCE, &installed.input).unwrap();
    document.validate().unwrap();
    let sessions = ExactRunSessionRegistry::new(1, document.budget.memory_bytes).unwrap();
    let mut session = {
        let arena = Bump::new();
        let plan = document.as_plan(&arena).unwrap();
        let panel = parse(SOURCE).unwrap();
        let registry = Registry::hosted_primitives();
        let resolved = registry.resolve(&panel).unwrap();
        let bindings = installed.bindings(&plan).unwrap();
        resolved
            .start_exact_session(
                &plan,
                &bindings,
                ExactRunContext {
                    semantic_source_hash: plan.source_semantic_hash,
                    plan_epoch: 41,
                    run_id: conduit_core::Id("fixture/owned-plan-arena"),
                    grant_observations: &[],
                    validation: conduit_core::PlanValidationContext {
                        supported_schema_version: plan.schema_version,
                        now: plan.created_at,
                    },
                    scheduler_policy: SchedulerPolicy {
                        schema_version: SCHEDULER_CONTRACT_VERSION,
                        ready_queue: ReadyQueueDiscipline::RoundRobin,
                        max_decisions: 128,
                        max_tick: 256,
                        max_consecutive_yields: 8,
                        max_events: 64,
                    },
                    reservation: SchedulerReservation {
                        available_runtime_memory_bytes: plan.budget.memory_bytes,
                        executor_overhead_limit_bytes: plan.budget.memory_bytes,
                    },
                },
                &sessions,
                ExactRunIo::for_plan(&plan).unwrap(),
            )
            .unwrap()
    };
    assert_eq!(session.high_water().decisions, 0);
    let exact_identity = session.identity().clone();
    let authority = |operation| ExactWatchUseAuthority {
        operation,
        operator_id: "operator/fixture".to_owned(),
        control_grant_hash: SemanticHash::from_bytes([91; 32]),
        control_grant_active: true,
        run_id: exact_identity.run_id.clone(),
        plan_epoch: exact_identity.plan_epoch,
        watch_id: "watch/owned-source".to_owned(),
        lease_id: "lease/watch-owned-source".to_owned(),
        lease_epoch: exact_identity.plan_epoch,
        lease_available: true,
        reveal_grant_hash: None,
        reveal_grant_active: false,
        time_basis: "clock/conduct-host".to_owned(),
        validated_at_tick: 12,
        valid_until_tick: u64::MAX,
    };
    session
        .attach_watch(
            "watch/owned-source",
            &authority(ExactWatchOperation::Attach),
        )
        .unwrap();
    let mut last_pump = None;
    while matches!(session.state(), conduit_runtime::ExactRunState::Active) {
        last_pump = Some(session.pump(1, &[]).unwrap());
    }
    assert_eq!(
        session.state(),
        conduit_runtime::ExactRunState::Terminal(conduit_core::TerminalClass::Succeeded)
    );
    assert_eq!(
        session.with_io(|io| io.display().to_vec()),
        b"owned session"
    );
    assert_eq!(session.identity(), &exact_identity);
    let watched = session
        .read_watch(
            "watch/owned-source",
            0,
            1,
            &authority(ExactWatchOperation::Read),
        )
        .unwrap();
    assert_eq!(watched.status, EvidenceCursorStatus::Available);
    assert_eq!(watched.records.len(), 1);
    assert_eq!(watched.records[0].watch_id, "watch/owned-source");
    assert_eq!(watched.records[0].original_bytes, 13);
    assert_eq!(
        watched.records[0].material,
        ExactWatchMaterial::Preview(b"owned".to_vec())
    );
    assert!(watched.records[0].truncated);
    assert!(watched.records[0].content_hash.is_some());
    assert_eq!(session.watch_usage().attached_slots, 1);
    assert_eq!(session.watch_usage().retained_observations, 1);
    session
        .detach_watch(
            "watch/owned-source",
            &authority(ExactWatchOperation::Detach),
        )
        .unwrap();
    assert_eq!(session.watch_usage().attached_slots, 0);
    assert_eq!(session.watch_usage().retained_observations, 1);
    let values = last_pump
        .expect("active hosted session completed through at least one pump")
        .value_storage
        .expect("hosted session exposes its fixed value arena");
    assert_eq!(values.resident_slots, 0);
    assert_eq!(values.resident_bytes, 0);
    assert!(values.high_water_slots > 0);
    assert!(values.high_water_bytes > 0);
    assert!(values.high_water_slots <= values.maximum_slots);
    assert!(values.high_water_bytes <= values.maximum_bytes);
    assert_eq!(session.value_storage_usage(), Some(values));
    assert!(session.finalize().is_ok());
    assert_eq!(sessions.active_sessions(), 0);
}

#[test]
fn text_format_availability_and_stale_provider_fail_closed_separately() {
    const FORMAT_SOURCE: &str = include_str!("../../../examples/formatted-greeting.panel");

    let contract_only = Registry::default();
    assert_eq!(
        contract_only.node_availability("std/text/format").state,
        AvailabilityState::ContractOnly
    );
    assert_eq!(
        contract_only.node_availability("std/format").state,
        AvailabilityState::Unsupported
    );
    let panel = parse(FORMAT_SOURCE).unwrap();
    assert_eq!(
        contract_only.resolve(&panel).unwrap_err().code,
        "CND-IMP-001"
    );

    let hosted = Registry::hosted_primitives();
    assert_eq!(
        hosted.node_availability("std/text/format").state,
        AvailabilityState::ProviderAvailable
    );
    let installed = InstalledProfile::observe(FORMAT_SOURCE).unwrap();
    let mut stale = installed.input.clone();
    let candidate = stale
        .candidates
        .iter_mut()
        .find(|candidate| candidate.implementation.semantic_contract.id == "std/text/format")
        .unwrap();
    candidate.host_report.valid_until_tick = stale.current_tick - 1;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(FORMAT_SOURCE, &stale).unwrap_err().code(),
        "CND-CMP-006"
    );
}
