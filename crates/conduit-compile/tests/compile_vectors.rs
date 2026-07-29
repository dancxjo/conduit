use std::collections::{BTreeMap, BTreeSet};

use conduit_compile::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, ExactPlanDocument, ExecutionLimitsDocument, ExecutionProfileDocument,
    HostReportDocument, ImplementationDocument, PinDocument, builtin_catalog_document,
    compile_source,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactDigest, CAPABILITY_REPORT_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION_V3, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::Registry;

const FIXTURE: &str = include_str!("../../../conformance/c5/compile-package-v1.json");
const SOURCE_LIMIT_FIXTURE: &str =
    include_str!("../../../conformance/c5/compile-source-limits-v1.json");
const SOURCE: &str = "panel 1\n\
node source : conduit/literal { value = \"hello\" }\n\
node upper : conduit/uppercase using ready\n\
node sink : conduit/stdout\n\
cord source.out -> upper.in\n\
cord upper.out -> sink.in\n";

fn hash(byte: u8) -> String {
    SemanticHash::from_bytes([byte; 32]).to_string()
}

fn pin(id: &str, byte: u8) -> PinDocument {
    PinDocument {
        id: id.to_owned(),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn profile(ordinal: u8) -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: format!("fixture/execution-profile-{ordinal}"),
        schema_version: 1,
        semantic_hash: hash(30),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 4,
            max_transactions: 1,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: Vec::new(),
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
                schema_version: 1,
                semantic_hash: contract_hash.to_string(),
            },
            executor: "native-in-process".to_owned(),
            entrypoint_name: "run".to_owned(),
            entrypoint_adapter: "conduit/native-step".to_owned(),
            entrypoint_abi: "conduit/native-v1".to_owned(),
            runtime_protocol_version: 1,
            execution_profile: pin("fixture/execution-profile", 30),
            artifacts: vec![ArtifactReferenceDocument {
                id: artifact_id.clone(),
                digest: artifact_digest.clone(),
                role: "implementation".to_owned(),
                required: true,
            }],
            required_authorities: Vec::new(),
            required_effects: Vec::new(),
            minimum_plan_version: 1,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
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
            supported_executors: vec!["native-in-process".to_owned()],
            supported_targets: Vec::new(),
            supported_abis: Vec::new(),
            minimum_plan_version: 1,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
            current_constraints: Vec::new(),
        },
        allocation: BudgetDocument {
            memory_bytes: 32,
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
    let topology = Registry::default()
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
            assert_eq!(plan.schema, "conduit.execution-plan/v3");
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
            sealed.schema = "conduit.compile-input/v1".to_owned();
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
