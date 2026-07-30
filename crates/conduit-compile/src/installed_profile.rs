use std::collections::BTreeMap;

use crate::{
    ArtifactDocument, ArtifactReferenceDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, ExecutionLimitsDocument, ExecutionProfileDocument, HostReportDocument,
    ImplementationDocument, MemoryClaimDocument, PinDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION_V3, ExecutionPlan,
    ExecutorKind, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::{
    ExactHostedBinding, ExactHostedBindings, HostedPrimitiveImplementation, Registry, RuntimeError,
};

pub struct InstalledProfile {
    pub input: CompileInput,
    implementations: BTreeMap<String, HostedPrimitiveImplementation>,
}

impl InstalledProfile {
    pub fn observe(source: &str) -> Result<Self, RuntimeError> {
        let panel =
            parse(source).map_err(|error| RuntimeError::new("CND-SRC-001", error.to_string()))?;
        let registry = Registry::hosted_primitives();
        let topology = registry
            .resolve(&panel)
            .and_then(|resolved| resolved.exact_topology())
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let mut required = BTreeMap::new();
        for node in &topology.nodes {
            required.insert(node.contract_id.clone(), node.contract_hash);
        }
        let mut implementations = BTreeMap::new();
        let mut candidates = Vec::with_capacity(required.len());
        for (contract_id, contract_hash) in required {
            let installed = Registry::installed_hosted_providers()
                .iter()
                .find(|provider| {
                    provider.contract.id.as_str() == contract_id
                        && provider.manifest.semantic_contract.semantic_hash == contract_hash
                })
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-007",
                        format!("no installed provider implements `{contract_id}`"),
                    )
                })?;
            candidates.push(candidate(installed));
        }
        let mut input = CompileInput {
            schema: COMPILE_INPUT_SCHEMA.to_owned(),
            schema_version: COMPILE_INPUT_SCHEMA_VERSION,
            identity: String::new(),
            entry_uri: "mem://conduct/entry.panel".to_owned(),
            selected_root: panel.selected_root.clone(),
            source_limits: CompileSourceLimits::default(),
            modules: vec![CompileModuleDocument {
                canonical_uri: "mem://conduct/entry.panel".to_owned(),
                content_hash: String::new(),
                source: source.to_owned(),
            }],
            catalog: builtin_catalog_document()
                .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?,
            pool_bindings: Vec::new(),
            supervision_bindings: Vec::new(),
            hazard_closure: None,
            distribution: None,
            source_semantic_hash: topology.source_semantic_hash.to_string(),
            resolver: pin("conduit/exact-compiler-resolver", 70),
            resolver_policy_hash: String::new(),
            time_basis: "clock/conduct-host".to_owned(),
            current_tick: 12,
            plan_budget: BudgetDocument {
                memory_bytes: 2 * 1024 * 1024,
                storage_bytes: 16 * 1024 * 1024,
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
        input
            .seal()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        for candidate in &input.candidates {
            let installed = Registry::installed_hosted_providers()
                .iter()
                .find(|provider| provider.manifest.id.as_str() == candidate.implementation.id)
                .expect("candidate came from installed provider inventory");
            implementations.insert(
                candidate.implementation.identity.clone(),
                installed.implementation,
            );
        }
        Ok(Self {
            input,
            implementations,
        })
    }

    pub fn bindings(&self, plan: &ExecutionPlan<'_>) -> Result<ExactHostedBindings, RuntimeError> {
        let mut bindings = Vec::with_capacity(plan.nodes.len());
        for node in plan.nodes {
            let candidate = self
                .input
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.implementation.id == node.implementation.id.as_str()
                        && candidate.implementation.identity
                            == node.implementation.semantic_hash.to_string()
                        && candidate.host_report.id == node.host_observation.as_str()
                })
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-RUN-007",
                        format!(
                            "planned implementation `{}` is not installed on this host",
                            node.implementation.id
                        ),
                    )
                })?;
            let implementation = self
                .implementations
                .get(&candidate.implementation.identity)
                .copied()
                .ok_or_else(|| {
                    RuntimeError::new("CND-RUN-007", "installed executable authority is absent")
                })?;
            let observation = plan
                .host_observations
                .iter()
                .find(|observation| observation.id == node.host_observation)
                .ok_or_else(|| {
                    RuntimeError::new("CND-RUN-007", "planned host observation is absent")
                })?;
            if observation.semantic_hash.to_string() != candidate.host_report.identity
                || observation.host.as_str() != candidate.host_report.host
            {
                return Err(RuntimeError::new(
                    "CND-RUN-007",
                    "planned host observation does not match the installed profile",
                ));
            }
            let installed = Registry::installed_hosted_providers()
                .iter()
                .find(|provider| provider.manifest.id.as_str() == candidate.implementation.id)
                .ok_or_else(|| RuntimeError::new("CND-RUN-007", "provider is not installed"))?;
            if node.artifact != installed.artifact.id
                || candidate
                    .artifacts
                    .iter()
                    .all(|artifact| artifact.digest != installed.artifact.digest.to_string())
            {
                return Err(RuntimeError::new(
                    "CND-RUN-008",
                    "planned artifact does not match installed executable code",
                ));
            }
            bindings.push(ExactHostedBinding {
                implementation_id: node.implementation.id.to_string(),
                implementation_identity: node.implementation.semantic_hash,
                artifact_id: installed.artifact.id.to_string(),
                artifact_digest: installed.artifact.digest,
                implementation,
            });
        }
        ExactHostedBindings::new(bindings)
    }
}

fn candidate(installed: &conduit_runtime::InstalledHostedProvider) -> CandidateDocument {
    let manifest = installed.manifest;
    let artifact = installed.artifact;
    CandidateDocument {
        implementation: ImplementationDocument {
            schema_version: IMPLEMENTATION_MANIFEST_SCHEMA_VERSION,
            identity: String::new(),
            id: manifest.id.to_string(),
            implementation_version: "1.0.0".to_owned(),
            semantic_contract: PinDocument {
                id: manifest.semantic_contract.id.to_string(),
                schema_version: manifest.semantic_contract.schema_version,
                semantic_hash: manifest.semantic_contract.semantic_hash.to_string(),
            },
            executor: executor_name(manifest.executor).to_owned(),
            entrypoint_name: manifest.entrypoint.name.to_string(),
            entrypoint_adapter: manifest.entrypoint.adapter.to_string(),
            entrypoint_abi: manifest.entrypoint.abi.to_string(),
            runtime_protocol_version: manifest.entrypoint.protocol_version,
            execution_profile: pin("conduit/hosted-primitive-profile-v1", 30),
            artifacts: vec![ArtifactReferenceDocument {
                id: artifact.id.to_string(),
                digest: artifact.digest.to_string(),
                role: "implementation".to_owned(),
                required: true,
            }],
            required_authorities: Vec::new(),
            required_effects: Vec::new(),
            minimum_plan_version: manifest.minimum_plan_version,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
            minimum_runtime_protocol: manifest.minimum_runtime_protocol,
            maximum_runtime_protocol: manifest.maximum_runtime_protocol,
            coexistence_memory_bytes: manifest.coexistence_memory_bytes,
        },
        execution_profile: execution_profile(),
        artifacts: vec![ArtifactDocument {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            identity: String::new(),
            id: artifact.id.to_string(),
            digest: artifact.digest.to_string(),
            media_type: "application/octet-stream".to_owned(),
            byte_size: artifact.byte_size,
            target: None,
            abi: None,
            builder: artifact.provenance.builder.to_string(),
            source_digest: artifact.provenance.source_digest.to_string(),
            build_recipe_digest: artifact.provenance.build_recipe_digest.to_string(),
            reproducible: artifact.provenance.reproducible,
            license_expressions: artifact
                .license_expressions
                .iter()
                .map(ToString::to_string)
                .collect(),
        }],
        host_report: HostReportDocument {
            schema_version: conduit_core::CAPABILITY_REPORT_SCHEMA_VERSION,
            identity: String::new(),
            id: "conduit/conduct-host-observation-v1".to_owned(),
            host: "conduit/conduct-host".to_owned(),
            reporter: pin("conduit/conduct-host-reporter", 50),
            trust: pin("conduit/local-build-trust", 51),
            membership: None,
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
            available: BudgetDocument {
                memory_bytes: 2 * 1024 * 1024,
                storage_bytes: 16 * 1024 * 1024,
                cpu_units: 64,
                timers: 16,
                transports: 16,
                checkpoints: 16,
                evidence_bytes: 16 * 1024,
            },
            capabilities: Vec::new(),
            resources: Vec::new(),
            topology: Vec::new(),
            supported_executors: vec![executor_name(manifest.executor).to_owned()],
            supported_targets: Vec::new(),
            supported_abis: Vec::new(),
            minimum_plan_version: manifest.minimum_plan_version,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
            current_constraints: Vec::new(),
        },
        allocation: BudgetDocument {
            memory_bytes: 2048,
            cpu_units: 1,
            evidence_bytes: 256,
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

fn execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile-v1".to_owned(),
        schema_version: 1,
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

fn executor_name(executor: ExecutorKind) -> &'static str {
    match executor {
        ExecutorKind::NativeInProcess => "native-in-process",
        ExecutorKind::WasmComponent => "wasm-component",
        ExecutorKind::FfiDynamicLibrary => "ffi-dynamic-library",
        ExecutorKind::Process => "process",
        ExecutorKind::Firmware => "firmware",
        ExecutorKind::RemoteEndpoint => "remote-endpoint",
    }
}

fn pin(id: &str, byte: u8) -> PinDocument {
    PinDocument {
        id: id.to_owned(),
        schema_version: 1,
        semantic_hash: SemanticHash::from_bytes([byte; 32]).to_string(),
    }
}
