use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use conduit_compile::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    ExecutionLimitsDocument, ExecutionProfileDocument, HostReportDocument, ImplementationDocument,
    PinDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactDigest, CAPABILITY_REPORT_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION_V3, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::Registry;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const FIXTURE: &str = include_str!("../../../conformance/c5/compile-package-v1.json");

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    for variable in [
        "NO_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "CI",
        "COLUMNS",
    ] {
        command.env_remove(variable);
    }
    command
}

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "conduct-compile-cli-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

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
                memory_bytes: 4096,
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
    let topology = Registry::default()
        .resolve(&panel)
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
        selected_root: panel.selected_root.clone(),
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

fn assert_fixture_case(id: &str) {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert!(
        fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| case["id"] == id && case["runner"] == "compile-cli"),
        "missing compile-cli vector {id}"
    );
}

#[test]
fn compile_emits_only_a_finite_validated_plan_result() {
    assert_fixture_case("compile-machine-stream-separation");
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    let panel = root.join("hello.panel");
    let input_path = root.join("compile-input.json");
    std::fs::write(&panel, source).unwrap();
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&input(source)).unwrap(),
    )
    .unwrap();
    let output = command()
        .args(["compile", "--format=json", "--input"])
        .arg(&input_path)
        .arg(&panel)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema"], "conduit.result/v1");
    assert_eq!(result["operation"], "compile");
    assert_eq!(result["result"]["schema"], "conduit.execution-plan/v3");
    assert_eq!(
        result["result"]["unresolved_selectors"],
        serde_json::json!([])
    );
    assert_eq!(result["result"]["nodes"].as_array().unwrap().len(), 3);

    let mut stale = input(source);
    for candidate in &mut stale.candidates {
        candidate.host_report.valid_until_tick = 11;
    }
    stale.seal().unwrap();
    std::fs::write(&input_path, serde_json::to_vec_pretty(&stale).unwrap()).unwrap();
    let rejected = command()
        .args([
            "compile",
            "--format=json",
            "--diagnostic-format=json",
            "--input",
        ])
        .arg(&input_path)
        .arg(&panel)
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-CMP-006");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn compile_diagnostics_stay_on_stderr_and_reserved_path_is_disambiguated() {
    assert_fixture_case("reserved-word-path-after-double-dash");
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    std::fs::write(root.join("compile"), source).unwrap();
    let disambiguated = command()
        .current_dir(&root)
        .args(["--run", "--", "compile"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(disambiguated.status.success());
    assert_eq!(disambiguated.stdout, b"HELLO FROM CONDUIT.\n");
    assert!(disambiguated.stderr.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
