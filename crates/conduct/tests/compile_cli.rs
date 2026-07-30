use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use conduit_compile::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, DistributionProviderDocument, ExecutionLimitsDocument,
    ExecutionProfileDocument, HostReportDocument, ImplementationDocument, PinDocument,
    ProviderRequirementDocument, ProviderRiskTraitsDocument, ReferenceDistributionDocument,
    builtin_catalog_document, compile_source,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactDigest, CAPABILITY_REPORT_SCHEMA_VERSION,
    DISTRIBUTION_PROFILE_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION_V3, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::Registry;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const FIXTURE: &str = include_str!("../../../conformance/c5/compile-package-v1.json");
const SOURCE_LIMIT_FIXTURE: &str =
    include_str!("../../../conformance/c5/compile-source-limits-v1.json");

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

fn absent_provider_input(source: &str) -> CompileInput {
    let firmware_traits = ProviderRiskTraitsDocument {
        firmware_mutation: true,
        ..ProviderRiskTraitsDocument::default()
    };
    let mut input = input(source);
    input.distribution = Some(ReferenceDistributionDocument {
        schema: conduit_compile::REFERENCE_DISTRIBUTION_DOCUMENT_SCHEMA.to_owned(),
        schema_version: DISTRIBUTION_PROFILE_SCHEMA_VERSION,
        identity: String::new(),
        descriptor: pin("distribution.reference", 180),
        kind: "hosted".to_owned(),
        genesis_profile: hash(181),
        control_recorder: pin("recorder.genesis", 182),
        provider_enablement_effect_class: pin("effect.provider-enable", 183),
        provider_enablement_operation: pin("operation.provider-enable", 184),
        providers: vec![DistributionProviderDocument {
            provider: pin("provider.firmware", 185),
            artifact: None,
            availability: "absent".to_owned(),
            traits: firmware_traits,
        }],
        maximum_provider_enablement_ticks: 20,
        maximum_provider_install_attempts: 2,
        maximum_evidence_events: 16,
        requirements: Vec::new(),
    });
    input.seal().unwrap();
    input.distribution.as_mut().unwrap().requirements = vec![ProviderRequirementDocument {
        provider: pin("provider.firmware", 185),
        traits: firmware_traits,
    }];
    input.identity = input.computed_identity().unwrap();
    input
}

fn exhausted_policy_budget_input(source: &str) -> CompileInput {
    let mut value = serde_json::to_value(input(source)).unwrap();
    let candidate = value["candidates"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|candidate| {
            let id = &candidate["implementation"]["semantic_contract"]["id"];
            id == "conduit/literal" || id == "conduit.std/literal"
        })
        .unwrap();
    candidate["implementation"]["maximum_plan_version"] =
        serde_json::json!(EXECUTION_PLAN_SCHEMA_VERSION);
    candidate["host_report"]["maximum_plan_version"] =
        serde_json::json!(EXECUTION_PLAN_SCHEMA_VERSION);
    let host = candidate["host_report"]["host"]
        .as_str()
        .unwrap()
        .to_owned();
    let budget_class = serde_json::to_value(pin("class.executable-installation", 119)).unwrap();
    candidate["authorities"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "requirement": hash(126),
            "effect_hash": "",
            "grant_hash": "",
            "effect": {
                "id": "fixture/governed",
                "policy_budget_class": budget_class,
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "requester": "root/greeting",
                "audience": "fixture/run",
                "constraints": [],
                "check_at_use": true
            },
            "capability": {
                "id": "fixture/governed-capability",
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "host": host.clone(),
                "time_basis": "clock/compile",
                "observed_at_tick": 10,
                "valid_until_tick": 20
            },
            "grant": {
                "id": "fixture/governed-grant",
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "scope_root": "root/greeting",
                "scope_descendants": false,
                "audience": "fixture/run",
                "constraints": [],
                "time_basis": "clock/compile",
                "not_before_tick": 10,
                "expires_at_tick": 20,
                "issued_for_host": host,
                "delegation": "none",
                "audit_id": "fixture/governed-audit",
                "terminal_policy": "abort"
            },
            "status": "active",
            "policy_budgets": [{
                "policy": {
                    "schema_version": 1,
                    "identity": "",
                    "descriptor": pin("budget.installation", 120),
                    "owner": pin("owner.site-operations", 121),
                    "subject": pin("subject.executable", 122),
                    "anchor_kind": "host",
                    "anchor_id": "fixture/host-local",
                    "action": "fixture/read",
                    "resource_class": pin("class.executable-installation", 119),
                    "time_basis": "clock/compile",
                    "limits": {
                        "current_stock": 1,
                        "rolling_units": 1,
                        "rolling_window_ticks": 100,
                        "lifetime": 1
                    },
                    "reservation_ttl_ticks": 5,
                    "lease": null,
                    "audit_id": "audit.installation",
                    "persistence_profile": pin("persistence.atomic", 123),
                    "maximum_reservations": 4,
                    "maximum_evidence_events": 16
                },
                "status": {
                    "schema_version": 1,
                    "identity": "",
                    "policy_identity": "",
                    "ledger": pin("ledger.host-installation", 124),
                    "checkpoint": hash(125),
                    "sequence": 4,
                    "current_stock": 0,
                    "rolling_window_start": 10,
                    "rolling_committed": 0,
                    "lifetime_committed": 1,
                    "reserved": 0,
                    "evidence_remaining": 12,
                    "availability": "available",
                    "time_basis": "clock/compile",
                    "observed_at_tick": 10,
                    "valid_until_tick": 20
                },
                "lease": null,
                "required_units": 1,
                "check_at_use": true
            }]
        }));
    let mut input: CompileInput = serde_json::from_value(value).unwrap();
    input.seal().unwrap();
    input
}

fn toxic_hazard_input(source: &str) -> CompileInput {
    let mut value = serde_json::to_value(input(source)).unwrap();
    let candidate = value["candidates"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|candidate| {
            let id = &candidate["implementation"]["semantic_contract"]["id"];
            id == "conduit/literal" || id == "conduit.std/literal"
        })
        .unwrap();
    candidate["implementation"]["maximum_plan_version"] =
        serde_json::json!(EXECUTION_PLAN_SCHEMA_VERSION);
    candidate["host_report"]["maximum_plan_version"] =
        serde_json::json!(EXECUTION_PLAN_SCHEMA_VERSION);
    let host = candidate["host_report"]["host"]
        .as_str()
        .unwrap()
        .to_owned();
    let effect_class = pin("class.cli-toxic", 130);
    candidate["authorities"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "requirement": hash(131),
            "effect_hash": "",
            "grant_hash": "",
            "effect": {
                "id": "fixture/cli-toxic",
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "requester": "root/greeting",
                "audience": "fixture/run",
                "constraints": [{
                    "id": effect_class.id,
                    "semantic_hash": effect_class.semantic_hash
                }],
                "check_at_use": true
            },
            "capability": {
                "id": "fixture/cli-toxic-capability",
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "host": host.clone(),
                "time_basis": "clock/compile",
                "observed_at_tick": 10,
                "valid_until_tick": 20
            },
            "grant": {
                "id": "fixture/cli-toxic-grant",
                "action": "fixture/read",
                "resource_kind": "fixture/device",
                "resource_id": "fixture/device-a",
                "scope_root": "root/greeting",
                "scope_descendants": false,
                "audience": "fixture/run",
                "constraints": [{
                    "id": effect_class.id,
                    "semantic_hash": effect_class.semantic_hash
                }],
                "time_basis": "clock/compile",
                "not_before_tick": 10,
                "expires_at_tick": 20,
                "issued_for_host": host,
                "delegation": "none",
                "audit_id": "fixture/cli-toxic-audit",
                "terminal_policy": "abort"
            },
            "status": "active"
        }));
    let mut baseline: CompileInput = serde_json::from_value(value).unwrap();
    baseline.seal().unwrap();
    let exact = compile_source(source, &baseline).unwrap();
    let plan_subject = exact.effect_closure_subject(1, &[]).unwrap();
    let mut value = serde_json::to_value(baseline).unwrap();
    value["hazard_closure"] = serde_json::json!({
        "epoch": 1,
        "plan_subject": plan_subject,
        "policy": {
            "schema_version": 1,
            "identity": "",
            "descriptor": pin("policy.cli-hazard", 132),
            "permit_class": pin("effect.cli-permit", 133),
            "classes": [{
                "identity": "",
                "descriptor": effect_class,
                "persistence": false,
                "delegation": false,
                "distributed": false,
                "administrative": false
            }],
            "rules": [{
                "identity": "",
                "descriptor": pin("rule.cli-toxic", 134),
                "patterns": [{
                    "id": "stage.cli-toxic",
                    "class": pin("class.cli-toxic", 130),
                    "resource_kind": null,
                    "resource_id": null,
                    "audience": null,
                    "host": null,
                    "realm": null,
                    "budget": null,
                    "persistence": "any",
                    "delegation": "any",
                    "distributed": "any",
                    "administrative": "any"
                }],
                "flows": []
            }],
            "limits": {
                "maximum_effects": 8,
                "maximum_classes": 4,
                "maximum_rules": 4,
                "maximum_patterns_per_rule": 4,
                "maximum_flows": 4,
                "maximum_permits": 4,
                "maximum_proof_nodes": 8,
                "maximum_search_steps": 64
            }
        },
        "flows": [],
        "permits": [],
        "decision_identity": hash(135)
    });
    let mut input: CompileInput = serde_json::from_value(value).unwrap();
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
fn check_and_explain_validate_the_explicit_compile_snapshot() {
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

    for mode in ["--check", "--explain"] {
        let output = command()
            .arg(mode)
            .arg("--compile-input")
            .arg(&input_path)
            .arg(&panel)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn check_and_explain_name_persistent_denial_not_plan_resource_exhaustion() {
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    let panel = root.join("hello.panel");
    let input_path = root.join("compile-input.json");
    std::fs::write(&panel, source).unwrap();
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&exhausted_policy_budget_input(source)).unwrap(),
    )
    .unwrap();

    for mode in ["--check", "--explain"] {
        let output = command()
            .arg(mode)
            .arg("--compile-input")
            .arg(&input_path)
            .arg(&panel)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{mode}");
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("CND-PBG-008"), "{mode}: {diagnostic}");
        assert!(
            diagnostic.contains("persistent policy budget denied the protected effect"),
            "{mode}: {diagnostic}"
        );
        assert!(
            !diagnostic.contains("resource budget"),
            "{mode}: {diagnostic}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_and_explain_name_the_whole_plan_toxic_combination() {
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    let panel = root.join("hello.panel");
    let input_path = root.join("compile-input.json");
    std::fs::write(&panel, source).unwrap();
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&toxic_hazard_input(source)).unwrap(),
    )
    .unwrap();

    for mode in ["--check", "--explain"] {
        let output = command()
            .arg(mode)
            .arg("--compile-input")
            .arg(&input_path)
            .arg(&panel)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{mode}");
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("CND-HZD-010"), "{mode}: {diagnostic}");
        assert!(
            diagnostic.contains("whole-plan effect closure"),
            "{mode}: {diagnostic}"
        );
        assert!(
            diagnostic.contains("rule rule.cli-toxic"),
            "{mode}: {diagnostic}"
        );
        assert!(
            diagnostic.contains("effects fixture/cli-toxic"),
            "{mode}: {diagnostic}"
        );
        assert!(
            !diagnostic.contains("resource budget"),
            "{mode}: {diagnostic}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_and_explain_name_intentionally_absent_dangerous_provider() {
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    let panel = root.join("hello.panel");
    let input_path = root.join("compile-input.json");
    std::fs::write(&panel, source).unwrap();
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&absent_provider_input(source)).unwrap(),
    )
    .unwrap();

    for mode in ["--check", "--explain"] {
        let output = command()
            .arg(mode)
            .arg("--compile-input")
            .arg(&input_path)
            .arg(&panel)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{mode}");
        assert!(output.stdout.is_empty(), "{mode}");
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("CND-GEN-010"), "{mode}: {diagnostic}");
        assert!(
            diagnostic
                .contains("required provider is intentionally absent, disabled, or unsupported"),
            "{mode}: {diagnostic}"
        );
        assert!(
            diagnostic.contains("provider provider.firmware"),
            "{mode}: {diagnostic}"
        );
        assert!(
            diagnostic.contains("availability absent"),
            "{mode}: {diagnostic}"
        );
        assert!(
            !diagnostic.contains("implementation, artifact, host, or authority resolution failed"),
            "{mode}: {diagnostic}"
        );
    }
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

fn run_source_limit_case(id: &str) -> serde_json::Value {
    let root = temporary_directory();
    let source = include_str!("../../../examples/hello.panel");
    let source_bytes = source.as_bytes();
    let source_len = u64::try_from(source_bytes.len()).unwrap();
    let panel = root.join("entry.panel");
    let input_path = root.join("compile-input.json");
    let mut compile_input = input(source);
    compile_input.source_limits = CompileSourceLimits {
        maximum_entry_source_bytes: source_len,
        maximum_module_source_bytes: source_len,
        maximum_module_closure_bytes: source_len,
        maximum_modules: 1,
    };
    compile_input.seal().unwrap();
    std::fs::write(
        &input_path,
        serde_json::to_vec_pretty(&compile_input).unwrap(),
    )
    .unwrap();
    let (entry_bytes, use_stdin) = match id {
        "exact-entry-source-limit-succeeds" => (source_bytes.to_vec(), false),
        "oversized-entry-source-is-not-truncated" | "oversized-stdin-source-is-not-truncated" => {
            let mut oversized = source_bytes.to_vec();
            oversized.push(b'#');
            (oversized, id == "oversized-stdin-source-is-not-truncated")
        }
        "truncated-entry-source-is-rejected" => {
            (source_bytes[..source_bytes.len() - 1].to_vec(), false)
        }
        "invalid-utf8-entry-source-is-rejected" => (vec![0xff, b'\n'], false),
        other => panic!("compile CLI source-limit case `{other}` is not implemented"),
    };
    if !use_stdin {
        std::fs::write(&panel, &entry_bytes).unwrap();
    }
    let mut process = command();
    process
        .args([
            "compile",
            "--format=json",
            "--diagnostic-format=json",
            "--input",
        ])
        .arg(&input_path);
    let output = if use_stdin {
        let mut child = process
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&entry_bytes).unwrap();
        child.wait_with_output().unwrap()
    } else {
        process.arg(&panel).output().unwrap()
    };
    let actual = if output.status.success() {
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        serde_json::json!({"accepted": true})
    } else {
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        serde_json::json!({"accepted": false, "code": diagnostic["code"]})
    };
    std::fs::remove_dir_all(root).unwrap();
    actual
}

#[test]
fn every_compile_cli_source_limit_vector_executes() {
    let fixture: serde_json::Value = serde_json::from_str(SOURCE_LIMIT_FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut executed = 0;
    for case in cases.iter().filter(|case| case["runner"] == "compile-cli") {
        let id = case["id"].as_str().unwrap();
        assert_eq!(run_source_limit_case(id), case["expected"], "case `{id}`");
        executed += 1;
    }
    assert_eq!(executed, 5);
}
