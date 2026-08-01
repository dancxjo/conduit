use std::collections::BTreeMap;

use conduit_compile::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, ExecutionLimitsDocument, ExecutionProfileDocument, HostReportDocument,
    ImplementationDocument, MemoryClaimDocument, PinDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactDigest, CAPABILITY_REPORT_SCHEMA_VERSION,
    EXECUTION_PLAN_SCHEMA_VERSION, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::Registry;

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
        id: format!("fixture/browser-execution-profile-{ordinal}"),
        schema_version: 0,
        semantic_hash: String::new(),
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
            implementation_memory_bytes: 2048,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![MemoryClaimDocument {
            category: "port-transactions".to_owned(),
            accounting: "executor-allocated".to_owned(),
            bytes: 2048,
        }],
        checkpoint: None,
    }
}

fn candidate(ordinal: u8, contract_id: &str, contract_hash: SemanticHash) -> CandidateDocument {
    let name = contract_id
        .rsplit('/')
        .next()
        .expect("reference contract has a final path segment");
    let artifact_id = format!("fixture/browser-artifact-{ordinal}");
    let artifact_digest = ArtifactDigest::from_bytes([ordinal; 32]).to_string();
    CandidateDocument {
        implementation: ImplementationDocument {
            schema_version: IMPLEMENTATION_MANIFEST_SCHEMA_VERSION,
            identity: String::new(),
            id: format!("fixture/browser-implementation-{ordinal}"),
            implementation_version: "1.0.0".to_owned(),
            semantic_contract: PinDocument {
                id: contract_id.to_owned(),
                schema_version: 0,
                semantic_hash: contract_hash.to_string(),
            },
            executor: "wasm-component".to_owned(),
            entrypoint_name: name.to_owned(),
            entrypoint_adapter: "conduit/hosted-primitive-step".to_owned(),
            entrypoint_abi: "conduit/hosted-primitive".to_owned(),
            runtime_protocol_version: 0,
            execution_profile: pin("fixture/browser-execution-profile", 30),
            artifacts: vec![ArtifactReferenceDocument {
                id: artifact_id.clone(),
                digest: artifact_digest.clone(),
                role: "implementation".to_owned(),
                required: true,
            }],
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
            media_type: "application/wasm".to_owned(),
            byte_size: 1,
            target: Some("wasm32-unknown-unknown".to_owned()),
            abi: Some("wasm-bindgen".to_owned()),
            builder: "fixture/browser-builder".to_owned(),
            source_digest: ArtifactDigest::from_bytes([40; 32]).to_string(),
            build_recipe_digest: ArtifactDigest::from_bytes([41; 32]).to_string(),
            reproducible: true,
            license_expressions: vec!["MIT".to_owned()],
        }],
        host_report: HostReportDocument {
            schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
            identity: String::new(),
            id: format!("fixture/browser-report-{ordinal}"),
            host: "fixture/browser-worker".to_owned(),
            reporter: pin("fixture/browser-reporter", 50),
            trust: pin("fixture/browser-report-trust", 51),
            membership: None,
            time_basis: "clock/browser".to_owned(),
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
            supported_executors: vec!["wasm-component".to_owned()],
            supported_targets: vec!["wasm32-unknown-unknown".to_owned()],
            supported_abis: vec!["wasm-bindgen".to_owned()],
            minimum_plan_version: 0,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            current_constraints: Vec::new(),
        },
        allocation: BudgetDocument {
            memory_bytes: 2048,
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

pub fn browser_compile_input(source: &str) -> CompileInput {
    let panel = parse(source).expect("reference source parses");
    let topology = Registry::compatibility_demo()
        .resolve(&panel)
        .expect("reference source resolves")
        .exact_topology()
        .expect("reference topology lowers");
    let mut contracts = BTreeMap::new();
    for node in &topology.nodes {
        contracts
            .entry(node.contract_id.clone())
            .or_insert(node.contract_hash);
    }
    let candidates = contracts
        .into_iter()
        .enumerate()
        .map(|(index, (id, semantic_hash))| candidate(index as u8 + 1, &id, semantic_hash))
        .collect();
    let mut input = CompileInput {
        schema: COMPILE_INPUT_SCHEMA.to_owned(),
        schema_version: COMPILE_INPUT_SCHEMA_VERSION,
        identity: String::new(),
        entry_uri: "mem://browser/reference.panel".to_owned(),
        selected_root: panel.selected_root.clone(),
        source_limits: CompileSourceLimits::default(),
        modules: vec![CompileModuleDocument {
            canonical_uri: "mem://browser/reference.panel".to_owned(),
            content_hash: String::new(),
            source: source.to_owned(),
        }],
        catalog: builtin_catalog_document().expect("builtin catalog"),
        pool_bindings: Vec::new(),
        supervision_bindings: Vec::new(),
        hazard_closure: None,
        distribution: None,
        evidence_provider: None,
        watch_admissions: Vec::new(),
        source_semantic_hash: topology.source_semantic_hash.to_string(),
        resolver: pin("conduit/exact-compiler-resolver", 70),
        resolver_policy_hash: String::new(),
        time_basis: "clock/browser".to_owned(),
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
    input.seal().expect("reference input seals");
    input
}
