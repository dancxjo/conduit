use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use conduit_core::{
    ArtifactDigest, ArtifactLocation, ArtifactLocationKind, ArtifactManifest, ArtifactProvenance,
    AuthorityTime, BlockingFairness, CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, Direction,
    ExecutionPlan, ExecutorKind, FlowCapacity, FlowPolicy, FlowWatermarks, Id, InstancePath,
    PassportStatus, PassportStatusObservation, PinnedDescriptor, PlanArtifact, PlanHostObservation,
    PlanResourceBudget, PlanValidationContext, Pressure, ReportMembership, ResolvedPlanCord,
    ResolvedPlanNode, ResolvedPlanPort, SemanticHash, TypeContractRef,
};
use conduit_inspect::{
    ArtifactKind, InspectLimits, RequestedKind, inspect_artifact_manifest, inspect_bytes,
    inspect_capability_report, inspect_conformance_manifest_path, inspect_execution_plan,
    inspect_lowered_source,
};
use conduit_runtime::LoweredSourceV2;
use sha2::Digest as _;

const INSPECTION_FIXTURE: &str = include_str!("../../../conformance/c3/inspection-v1.json");
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const ZERO_HASH: SemanticHash = SemanticHash::from_bytes([0; 32]);
const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([10; 32]),
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn time(tick: u64) -> AuthorityTime<'static> {
    AuthorityTime {
        basis: Id("clock/monotonic"),
        tick,
    }
}

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "conduit-inspection-library-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn artifact_manifest_inspection_exposes_license_provenance_and_remote_hint_without_fetching() {
    let mut manifest = ArtifactManifest {
        schema_version: 1,
        identity: ZERO_HASH,
        id: Id("fixture/artifact"),
        digest: ArtifactDigest::from_bytes([31; 32]),
        media_type: "application/wasm",
        byte_size: 42,
        target: Some(Id("wasm32-wasip2")),
        abi: Some(Id("component-v1")),
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([32; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([33; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &["Apache-2.0"],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[ArtifactLocation {
            kind: ArtifactLocationKind::RemoteUri,
            locator: "https://artifacts.invalid/module.wasm",
        }],
    };
    let mut scratch = [ZERO_HASH; 1];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();

    let report = inspect_artifact_manifest(&manifest, DIGEST, InspectLimits::default()).unwrap();
    assert_eq!(report.kind, ArtifactKind::ArtifactManifest);
    assert_eq!(report.budgets["artifact_bytes"], 42);
    assert!(
        report.references.iter().any(|reference| {
            reference.category == "license" && reference.value == "Apache-2.0"
        })
    );
    assert!(report.references.iter().any(|reference| {
        reference.category == "provenance-builder" && reference.value == "fixture/builder"
    }));
    assert!(report.references.iter().any(|reference| {
        reference.category == "remote-location"
            && reference.value == "https://artifacts.invalid/module.wasm"
    }));
}

#[test]
fn capability_report_inspection_never_refreshes_or_provisions_the_host() {
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: ZERO_HASH,
        id: Id("fixture/browser-report"),
        host: Id("browser/a"),
        reporter: pin("fixture/reporter", 41),
        trust: pin("fixture/trust", 42),
        membership: Some(ReportMembership {
            realm: Id("fixture/realm"),
            entity: Id("fixture/browser"),
            passport: hash(43),
            status: PassportStatusObservation {
                passport: hash(43),
                realm: Id("fixture/realm"),
                entity: Id("fixture/browser"),
                reporter: pin("fixture/status-reporter", 44),
                time_basis: Id("clock/monotonic"),
                observed_at_tick: 9,
                valid_until_tick: 20,
                status: PassportStatus::Active,
            },
        }),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        available: PlanResourceBudget {
            memory_bytes: 1024,
            transports: 2,
            ..PlanResourceBudget::ZERO
        },
        capabilities: &[],
        resources: &[],
        topology: &[],
        supported_executors: &[ExecutorKind::WasmComponent],
        supported_targets: &[Id("wasm32-unknown-unknown")],
        supported_abis: &[Id("component-v1")],
        minimum_plan_version: 1,
        maximum_plan_version: 8,
        current_constraints: &[],
    };
    let mut scratch = [ZERO_HASH; 3];
    report.identity = report.computed_semantic_hash(&mut scratch).unwrap();
    let inspected = inspect_capability_report(&report, DIGEST, InspectLimits::default()).unwrap();
    assert_eq!(inspected.kind, ArtifactKind::CapabilityReport);
    assert_eq!(inspected.budgets["available_memory_bytes"], 1024);
    assert_eq!(inspected.counts["membership_bindings"], 1);
    assert!(inspected.references.iter().any(|reference| {
        reference.category == "passport-identity" && reference.value == hash(43).to_string()
    }));
    assert!(
        inspected
            .notes
            .iter()
            .any(|note| note.contains("does not refresh"))
    );
}

fn with_minimal_plan(test: impl FnOnce(ExecutionPlan<'_>)) {
    let observations = [PlanHostObservation {
        id: Id("observation/a"),
        host: Id("host/a"),
        semantic_hash: hash(4),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 1,
        valid_until_tick: 100,
    }];
    let artifacts = [PlanArtifact {
        id: Id("artifact/a"),
        digest: ArtifactDigest::from_bytes([5; 32]),
    }];
    let allocation = PlanResourceBudget {
        memory_bytes: 64,
        cpu_units: 1,
        ..PlanResourceBudget::ZERO
    };
    let nodes = [ResolvedPlanNode {
        instance: InstancePath::new("root/node").unwrap(),
        contract: pin("fixture/contract", 6),
        implementation: pin("fixture/implementation", 7),
        lifecycle_policy: pin("fixture/lifecycle", 8),
        execution_profile: None,
        artifact: Id("artifact/a"),
        host_observation: Id("observation/a"),
        host: Id("host/a"),
        allocation,
        required_resources: &[],
        required_effects: &[],
    }];
    let capacity = FlowCapacity::new(1, 8, 8).unwrap();
    let cords = [ResolvedPlanCord {
        id: Id("cord/a"),
        from: ResolvedPlanPort {
            node: InstancePath::new("root/node").unwrap(),
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(11),
            value_type: VALUE_TYPE,
        },
        to: ResolvedPlanPort {
            node: InstancePath::new("root/node").unwrap(),
            port: Id("in"),
            direction: Direction::Input,
            port_contract_hash: hash(12),
            value_type: VALUE_TYPE,
        },
        flow: FlowPolicy::new(
            capacity,
            Pressure::Block(BlockingFairness::Fifo),
            FlowWatermarks::new(0, 1, capacity).unwrap(),
        )
        .unwrap(),
        queue_memory_bytes: 8,
    }];
    let mut plan = ExecutionPlan {
        schema_version: 1,
        identity: ZERO_HASH,
        source_semantic_hash: hash(1),
        resolver: pin("fixture/resolver", 2),
        resolver_policy_hash: hash(3),
        created_at: time(10),
        budget: PlanResourceBudget {
            memory_bytes: 128,
            cpu_units: 2,
            evidence_bytes: 32,
            ..PlanResourceBudget::ZERO
        },
        host_observations: &observations,
        resources: &[],
        artifacts: &artifacts,
        nodes: &nodes,
        cords: &cords,
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &[],
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        unresolved: &[],
    };
    let mut scratch = [ZERO_HASH; 8];
    plan.identity = plan.semantic_hash(&mut scratch).unwrap();
    test(plan);
}

#[test]
fn panel_detection_is_comment_safe_and_never_reproduces_secrets() {
    let source = br#"# comment
panel 1
node value : conduit/literal { value = secret("credential/material") }
"#;
    let report = inspect_bytes(
        source,
        RequestedKind::Auto,
        Some("panel"),
        InspectLimits::default(),
    )
    .unwrap();
    assert_eq!(report.kind, ArtifactKind::PanelSource);
    assert_eq!(report.redacted_fields, 1);
    let human = report.render_human();
    assert!(!human.contains("credential"));
    assert!(!serde_json::to_string(&report).unwrap().contains("material"));
}

#[test]
fn evidence_and_diagnostics_validate_without_reproducing_payloads() {
    let evidence = include_bytes!("../../../conformance/c2/execution-event-v1.ndjson");
    let report = inspect_bytes(
        evidence,
        RequestedKind::Auto,
        Some("ndjson"),
        InspectLimits::default(),
    )
    .unwrap();
    assert_eq!(report.kind, ArtifactKind::ExecutionEvidence);
    assert_eq!(report.counts["records"], 3);
    assert_eq!(report.redacted_fields, 1);
    assert!(!report.render_human().contains("[104,101"));

    let diagnostic = br#"{"schema_version":1,"code":"CND-TST-001","severity":"error","message":"must not echo secret","arguments":[{"name":"token","value":{"disposition":"redacted","sensitivity":"secret","value_type":"fixture/token"}}]}"#;
    let report = inspect_bytes(
        diagnostic,
        RequestedKind::Diagnostic,
        Some("json"),
        InspectLimits::default(),
    )
    .unwrap();
    assert_eq!(report.kind, ArtifactKind::StructuredDiagnostic);
    assert_eq!(report.redacted_fields, 1);
    assert!(!report.render_human().contains("must not echo"));
}

#[test]
fn type_detection_and_allocation_limits_fail_closed() {
    let ambiguous =
        br#"{"suite":"fixture/v1","schema_version":1,"code":"CND-TST-001","severity":"error"}"#;
    assert_eq!(
        inspect_bytes(
            ambiguous,
            RequestedKind::Auto,
            Some("json"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-002"
    );
    assert_eq!(
        inspect_bytes(
            b"\0asm\x01\0\0\0",
            RequestedKind::Auto,
            None,
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-001"
    );
    let small = InspectLimits {
        max_input_bytes: 3,
        ..InspectLimits::default()
    };
    assert_eq!(
        inspect_bytes(b"panel 1\n", RequestedKind::Auto, None, small)
            .unwrap_err()
            .code,
        "CND-INSP-005"
    );
    let records = InspectLimits {
        max_records: 1,
        ..InspectLimits::default()
    };
    let evidence = include_bytes!("../../../conformance/c2/execution-event-v1.ndjson");
    assert_eq!(
        inspect_bytes(evidence, RequestedKind::Evidence, None, records)
            .unwrap_err()
            .code,
        "CND-INSP-007"
    );
}

#[test]
fn nested_malformed_and_extension_conflicts_are_bounded() {
    let nested = format!(
        "{{\"schema_version\":1,\"code\":\"CND-TST-001\",\"severity\":\"error\",\"extra\":{} }}",
        "[".repeat(65) + &"]".repeat(65)
    );
    assert_eq!(
        inspect_bytes(
            nested.as_bytes(),
            RequestedKind::Diagnostic,
            Some("json"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-007"
    );
    assert_eq!(
        inspect_bytes(
            b"panel 1\n",
            RequestedKind::Panel,
            Some("ndjson"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-003"
    );
    assert_eq!(
        inspect_bytes(
            b"{not-json",
            RequestedKind::Diagnostic,
            Some("json"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-006"
    );
}

#[test]
fn typed_plan_validation_reports_budgets_and_staleness_without_loading_artifacts() {
    with_minimal_plan(|plan| {
        let context = PlanValidationContext {
            supported_schema_version: 1,
            now: time(20),
        };
        let report =
            inspect_execution_plan(&plan, context, DIGEST, InspectLimits::default()).unwrap();
        assert_eq!(report.kind, ArtifactKind::ExecutionPlan);
        assert_eq!(report.identity, Some(plan.identity.to_string()));
        assert_eq!(report.budgets["memory_bytes"], 128);
        assert_eq!(report.counts["artifacts"], 1);
        assert!(
            report
                .notes
                .iter()
                .any(|note| note.contains("not artifact executability"))
        );

        let stale = PlanValidationContext {
            supported_schema_version: 1,
            now: time(100),
        };
        assert_eq!(
            inspect_execution_plan(&plan, stale, DIGEST, InspectLimits::default())
                .unwrap_err()
                .code,
            "CND-HST-002"
        );

        let dangling_cords = [ResolvedPlanCord {
            to: ResolvedPlanPort {
                node: InstancePath::new("root/missing").unwrap(),
                ..plan.cords[0].to
            },
            ..plan.cords[0]
        }];
        let dangling = ExecutionPlan {
            cords: &dangling_cords,
            ..plan
        };
        assert_eq!(
            inspect_execution_plan(&dangling, context, DIGEST, InspectLimits::default())
                .unwrap_err()
                .code,
            "CND-PLN-004"
        );
    });
}

#[test]
fn typed_lowering_retains_its_distinct_identity() {
    let source = LoweredSourceV2 {
        schema_version: 2,
        source_ast_schema_version: 2,
        root_selection: None,
        nodes: Vec::new(),
        cords: Vec::new(),
        composites: Vec::new(),
        composite_children: Vec::new(),
        exports: Vec::new(),
        bindings: Vec::new(),
        group_ports: Vec::new(),
        pools: Vec::new(),
        source_map: Vec::new(),
        semantic_hash: hash(9),
    };
    let report = inspect_lowered_source(&source, DIGEST, InspectLimits::default()).unwrap();
    assert_eq!(report.kind, ArtifactKind::LoweredSource);
    assert_eq!(report.identity, Some(hash(9).to_string()));
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("distinct from an exact execution plan"))
    );
}

#[test]
fn local_manifest_references_are_digest_verified_without_running_tests() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/v1/manifest.json");
    let report = inspect_conformance_manifest_path(&path, InspectLimits::default()).unwrap();
    assert_eq!(report.kind, ArtifactKind::ConformanceManifest);
    assert!(report.counts["verified_artifacts"] > 0);
    assert!(report.counts["referenced_bytes"] > 0);
}

#[test]
fn module_graph_and_reference_traversal_limits_are_enforced() {
    let directory = temporary_directory();
    let panel_root = directory.join("panels");
    std::fs::create_dir_all(&panel_root).unwrap();
    std::fs::write(panel_root.join("child.panel"), b"panel 1\n").unwrap();
    std::fs::write(
        panel_root.join("root.panel"),
        b"panel 1\nimport \"./child.panel\" as child\n",
    )
    .unwrap();

    let report = conduit_inspect::inspect_panel_path(
        &panel_root.join("root.panel"),
        InspectLimits::default(),
    )
    .unwrap();
    assert_eq!(report.counts["modules"], 2);

    let module_limit = InspectLimits {
        max_modules: 1,
        ..InspectLimits::default()
    };
    assert_eq!(
        conduit_inspect::inspect_panel_path(&panel_root.join("root.panel"), module_limit)
            .unwrap_err()
            .code,
        "CND-SRC-003"
    );

    let byte_limit = InspectLimits {
        max_total_module_bytes: 10,
        ..InspectLimits::default()
    };
    assert_eq!(
        conduit_inspect::inspect_panel_path(&panel_root.join("root.panel"), byte_limit)
            .unwrap_err()
            .code,
        "CND-SRC-003"
    );

    std::fs::write(directory.join("outside.panel"), b"panel 1\n").unwrap();
    std::fs::write(
        panel_root.join("escape.panel"),
        b"panel 1\nimport \"../outside.panel\" as outside\n",
    )
    .unwrap();
    assert_eq!(
        conduit_inspect::inspect_panel_path(
            &panel_root.join("escape.panel"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-SRC-003"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn manifest_digest_and_path_mutations_fail_closed() {
    let directory = temporary_directory();
    let conformance = directory.join("conformance");
    let version = conformance.join("v1");
    let cases = conformance.join("c3");
    std::fs::create_dir_all(&version).unwrap();
    std::fs::create_dir_all(&cases).unwrap();
    let case_path = cases.join("cases.json");
    let case_bytes = br#"{"suite":"fixture/v1","cases":[]}"#;
    std::fs::write(&case_path, case_bytes).unwrap();
    let digest = format!("sha256:{:x}", sha2::Sha256::digest(case_bytes));
    let manifest = |path: &str, digest: &str| {
        serde_json::json!({
            "fixture_version": "conduit.conformance/v1",
            "manifest_revision": 1,
            "protocol_version": 1,
            "deterministic_environment": {
                "clock": {"basis": "fixture/clock", "tick": 1},
                "seed": 1,
                "host_observations": []
            },
            "property_seeds": {
                "bytes": [""],
                "recursion_depths": [0],
                "discovery_orders": [[]]
            },
            "suites": [{
                "id": "fixture",
                "profile": "conduit.c3",
                "requirement_ids": ["INSP-001"],
                "artifacts": [{
                    "id": "cases",
                    "path": path,
                    "sha256": digest,
                    "operation": "fixture",
                    "requirement_ids": ["INSP-001"],
                    "default_rule": "INSP-001",
                    "case_rules": {},
                    "format": "json-vectors",
                    "collection": "cases",
                    "case_fields": ["id"],
                    "expected_fields": ["expected"]
                }],
                "coverage": {
                    "positive": ["cases#ok"],
                    "negative": ["cases#bad"],
                    "boundary": ["cases#bound"],
                    "migration": ["cases#migrate"]
                },
                "reference_tests": []
            }]
        })
    };
    let manifest_path = version.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest(
            "../c3/cases.json",
            &format!("sha256:{}", "0".repeat(64)),
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        inspect_conformance_manifest_path(&manifest_path, InspectLimits::default())
            .unwrap_err()
            .code,
        "CND-INSP-006"
    );

    let outside = directory.join("outside.json");
    std::fs::write(&outside, case_bytes).unwrap();
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest("../../outside.json", &digest)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        inspect_conformance_manifest_path(&manifest_path, InspectLimits::default())
            .unwrap_err()
            .code,
        "CND-INSP-006"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_version_and_record_mutations_are_rejected() {
    let evidence = include_str!("../../../conformance/c2/execution-event-v1.ndjson");
    let mut lines = evidence.lines();
    let mut first: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    first["schema_version"] = serde_json::json!(2);
    let mutated = std::iter::once(serde_json::to_string(&first).unwrap())
        .chain(lines.map(str::to_owned))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(
        inspect_bytes(
            mutated.as_bytes(),
            RequestedKind::Evidence,
            Some("ndjson"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-EVD-001"
    );
    assert_eq!(
        inspect_bytes(
            b"{}\n",
            RequestedKind::Evidence,
            Some("ndjson"),
            InspectLimits::default()
        )
        .unwrap_err()
        .code,
        "CND-INSP-006"
    );
}

fn fixture_bytes(case: &serde_json::Value) -> Vec<u8> {
    if let Some(hex) = case.get("input_hex").and_then(serde_json::Value::as_str) {
        return hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
    }
    match case["input"].as_str().unwrap() {
        "$HELLO_PANEL" => include_bytes!("../../../examples/hello.panel").to_vec(),
        "$EVIDENCE" => {
            include_bytes!("../../../conformance/c2/execution-event-v1.ndjson").to_vec()
        }
        "$DIAGNOSTIC" => br#"{"schema_version":1,"code":"CND-TST-001","severity":"error","message":"sensitive prose","arguments":[{"name":"token","value":{"disposition":"redacted","sensitivity":"secret","value_type":"fixture/token"}}]}"#.to_vec(),
        "$CONFORMANCE_JSON" => {
            include_bytes!("../../../conformance/c3/diagnostics-v1.json").to_vec()
        }
        "$CONFORMANCE_TSV" => {
            include_bytes!("../../../conformance/c2/execution-plan-v1.tsv").to_vec()
        }
        "$NATIVE_SCRIPT" => b"#!/bin/sh\nexit 99\n".to_vec(),
        value => value.as_bytes().to_vec(),
    }
}

fn requested(value: &str) -> RequestedKind {
    match value {
        "auto" => RequestedKind::Auto,
        "panel" => RequestedKind::Panel,
        "lowered-source" => RequestedKind::LoweredSource,
        "execution-plan" => RequestedKind::ExecutionPlan,
        "evidence" => RequestedKind::Evidence,
        "diagnostic" => RequestedKind::Diagnostic,
        "conformance" => RequestedKind::Conformance,
        _ => panic!("unknown requested kind {value}"),
    }
}

#[test]
fn fixture_limits_are_the_frozen_defaults() {
    let fixture: serde_json::Value = serde_json::from_str(INSPECTION_FIXTURE).unwrap();
    let limits = InspectLimits::default();
    assert_eq!(
        fixture["limits"]["max_input_bytes"].as_u64().unwrap(),
        limits.max_input_bytes
    );
    assert_eq!(
        fixture["limits"]["max_record_bytes"].as_u64().unwrap(),
        limits.max_record_bytes as u64
    );
    assert_eq!(
        fixture["limits"]["max_records"].as_u64().unwrap(),
        limits.max_records as u64
    );
    assert_eq!(
        fixture["limits"]["max_json_depth"].as_u64().unwrap(),
        limits.max_json_depth as u64
    );
    assert_eq!(
        fixture["limits"]["max_collection_items"].as_u64().unwrap(),
        limits.max_collection_items as u64
    );
    assert_eq!(
        fixture["limits"]["max_modules"].as_u64().unwrap(),
        limits.max_modules as u64
    );
    assert_eq!(
        fixture["limits"]["max_total_module_bytes"]
            .as_u64()
            .unwrap(),
        limits.max_total_module_bytes
    );
    assert_eq!(
        fixture["limits"]["max_total_reference_bytes"]
            .as_u64()
            .unwrap(),
        limits.max_total_reference_bytes
    );
}

#[test]
fn every_serialized_inspection_conformance_vector_executes() {
    let fixture: serde_json::Value = serde_json::from_str(INSPECTION_FIXTURE).unwrap();
    let mut delegated = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let runner = case["runner"].as_str().unwrap();
        if runner == "comparison" {
            let bytes = fixture_bytes(case);
            let automatic = inspect_bytes(
                &bytes,
                RequestedKind::Auto,
                Some("panel"),
                InspectLimits::default(),
            )
            .unwrap();
            let explicit = inspect_bytes(
                &bytes,
                RequestedKind::Panel,
                Some("panel"),
                InspectLimits::default(),
            )
            .unwrap();
            assert_eq!(automatic, explicit, "{id}");
            continue;
        }
        if runner == "limit" {
            let limit = case["limit"].as_str().unwrap();
            let mut limits = InspectLimits::default();
            let (bytes, requested) = match limit {
                "max_input_bytes" => {
                    limits.max_input_bytes = 3;
                    (b"panel 1\n".to_vec(), RequestedKind::Auto)
                }
                "max_record_bytes" => {
                    limits.max_record_bytes = 16;
                    (
                        include_bytes!("../../../conformance/c2/execution-event-v1.ndjson")
                            .to_vec(),
                        RequestedKind::Evidence,
                    )
                }
                "max_records" => {
                    limits.max_records = 1;
                    (
                        include_bytes!("../../../conformance/c2/execution-event-v1.ndjson")
                            .to_vec(),
                        RequestedKind::Evidence,
                    )
                }
                "max_json_depth" => {
                    limits.max_json_depth = 2;
                    (
                        br#"{"schema_version":1,"code":"CND-TST-001","severity":"error","related":[{"label":"x"}]}"#.to_vec(),
                        RequestedKind::Diagnostic,
                    )
                }
                "max_collection_items" => {
                    limits.max_collection_items = 2;
                    (
                        br#"{"schema_version":1,"code":"CND-TST-001","severity":"error","message":"x"}"#.to_vec(),
                        RequestedKind::Diagnostic,
                    )
                }
                _ => panic!("unknown limit {limit}"),
            };
            let error = inspect_bytes(&bytes, requested, None, limits).unwrap_err();
            assert_eq!(error.code, case["expected"]["error"], "{id}");
            continue;
        }
        if runner != "bytes" {
            delegated.push((id, runner));
            continue;
        }
        let bytes = fixture_bytes(case);
        let requested = requested(case["requested"].as_str().unwrap());
        let result = inspect_bytes(
            &bytes,
            requested,
            case.get("extension").and_then(serde_json::Value::as_str),
            InspectLimits::default(),
        );
        if let Some(code) = case["expected"]
            .get("error")
            .and_then(serde_json::Value::as_str)
        {
            assert_eq!(result.unwrap_err().code, code, "{id}");
            continue;
        }
        let report = result.unwrap();
        assert_eq!(
            report.kind.as_str(),
            case["expected"]["kind"].as_str().unwrap(),
            "{id}"
        );
        if let Some(version) = case["expected"]
            .get("version")
            .and_then(serde_json::Value::as_u64)
        {
            assert_eq!(u64::from(report.artifact_version), version, "{id}");
        }
        if let Some(records) = case["expected"]
            .get("records")
            .and_then(serde_json::Value::as_u64)
        {
            assert_eq!(report.counts["records"], records, "{id}");
        }
        if let Some(redacted) = case["expected"]
            .get("redacted_fields")
            .and_then(serde_json::Value::as_u64)
        {
            assert_eq!(report.redacted_fields, redacted, "{id}");
        }
        if let Some(unresolved) = case["expected"]
            .get("unresolved_selectors")
            .and_then(serde_json::Value::as_u64)
        {
            assert_eq!(report.counts["unresolved_selectors"], unresolved, "{id}");
        }
    }

    assert_eq!(
        delegated,
        [
            ("panel-module-path", "path"),
            ("lowered-source-typed", "typed-lowering"),
            ("execution-plan-typed", "typed-plan"),
            ("conformance-manifest", "path"),
            ("module-count-limit", "path-limit"),
            ("aggregate-module-byte-limit", "path-limit"),
            ("conformance-reference-digest-mismatch", "manifest-mutation"),
            ("conformance-reference-traversal", "manifest-mutation"),
            ("stale-plan-observation", "typed-plan"),
            ("dangling-plan-endpoint", "typed-plan"),
            ("native-bytes-never-executed", "no-exec"),
            ("wasm-bytes-never-executed", "no-exec"),
            ("human-stdout", "cli"),
            ("json-result-stdout", "cli"),
            ("diagnostic-json-isolated", "cli"),
            ("quiet-preserves-result", "cli"),
            ("broken-pipe-success", "cli"),
            ("output-failure", "cli"),
            ("secondary-path-escape", "cli"),
            ("mode-secondary-conflict", "cli"),
            ("inspect-ndjson-rejected", "cli"),
            ("unsupported-evidence-version", "evidence-mutation"),
            ("malformed-evidence-record", "evidence-mutation"),
            ("identity-categories-remain-distinct", "typed-comparison"),
            ("inspect-help", "generated"),
            ("generated-inspect-completions", "generated"),
            ("generated-inspect-man", "generated")
        ]
    );
}
