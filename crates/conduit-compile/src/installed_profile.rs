use std::collections::BTreeMap;

use crate::{
    ArtifactDocument, ArtifactReferenceDocument, AuthorityDecisionDocument, AuthorityGrantDocument,
    BudgetDocument, COMPILE_INPUT_SCHEMA, COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument,
    CompileInput, CompileModuleDocument, CompileSourceLimits, EffectRequirementDocument,
    ExecutionLimitsDocument, ExecutionProfileDocument, HostCapabilityDocument, HostReportDocument,
    ImplementationDocument, MemoryClaimDocument, PinDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION_V3, ExecutionPlan,
    ExecutorKind, IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::{
    ExactHostedBinding, ExactHostedBindings, HostedPrimitiveImplementation,
    InstalledHostedProvider, OwnedNodeSchema, Registry, RuntimeError,
};

pub struct InstalledProfile {
    pub input: CompileInput,
    implementations: BTreeMap<String, HostedPrimitiveImplementation>,
    providers: Vec<InstalledHostedProvider>,
}

impl InstalledProfile {
    pub fn observe(source: &str) -> Result<Self, RuntimeError> {
        Self::observe_with_stdout_grant(source, true)
    }

    /// Observe the compiled-in profile with an explicit caller-owned stdout
    /// authority fact. The false branch exists for fail-closed conformance.
    pub fn observe_with_stdout_grant(
        source: &str,
        stdout_granted: bool,
    ) -> Result<Self, RuntimeError> {
        Self::observe_registry_with_stdout_grant(
            source,
            &Registry::hosted_primitives(),
            stdout_granted,
        )
    }

    /// Observe an explicitly assembled host registry. This is the production
    /// extension point for linked host-service providers.
    pub fn observe_registry(source: &str, registry: &Registry) -> Result<Self, RuntimeError> {
        Self::observe_registry_with_stdout_grant(source, registry, true)
    }

    fn observe_registry_with_stdout_grant(
        source: &str,
        registry: &Registry,
        stdout_granted: bool,
    ) -> Result<Self, RuntimeError> {
        let panel =
            parse(source).map_err(|error| RuntimeError::new("CND-SRC-001", error.to_string()))?;
        let topology = registry
            .resolve(&panel)
            .and_then(|resolved| resolved.exact_topology())
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let mut required = BTreeMap::new();
        for node in &topology.nodes {
            required
                .entry(node.contract_id.clone())
                .or_insert_with(|| (node.contract_hash, Vec::new()))
                .1
                .push(node.instance.clone());
        }
        let requires_large_evidence = required.contains_key("std/text/format")
            || required.contains_key("std/format-values/literal")
            || required.contains_key("std/text/lines")
            || required.contains_key("std/text/join");
        let providers = registry.installed_providers();
        let mut implementations = BTreeMap::new();
        let mut candidates = Vec::with_capacity(required.len());
        for (contract_id, (contract_hash, instances)) in required {
            let installed = providers
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
            let stdout_instance = (contract_id == "io/stdout")
                .then(|| instances.first().cloned())
                .flatten();
            let host_service_instance = (installed.implementation
                == HostedPrimitiveImplementation::HostedService)
                .then(|| instances.first().cloned())
                .flatten();
            candidates.push(candidate(
                installed,
                stdout_instance.as_deref(),
                host_service_instance.as_deref(),
                stdout_granted,
            ));
        }
        let mut catalog = builtin_catalog_document()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        for contract_id in candidates
            .iter()
            .map(|candidate| candidate.implementation.semantic_contract.id.as_str())
        {
            let contract = registry
                .contracts()
                .find(|contract| contract.id.as_str() == contract_id)
                .expect("candidate contract came from registry");
            if catalog
                .nodes
                .iter()
                .any(|node| node.id == contract.id.as_str())
            {
                continue;
            }
            catalog.nodes.push(PinDocument {
                id: contract.id.to_string(),
                schema_version: 1,
                semantic_hash: OwnedNodeSchema::from_contract(contract)
                    .semantic_hash()
                    .to_string(),
            });
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
            catalog,
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
                evidence_bytes: if requires_large_evidence {
                    256 * 1024
                } else {
                    16 * 1024
                },
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
            let installed = providers
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
            providers,
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
            let installed = self
                .providers
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

fn candidate(
    installed: &conduit_runtime::InstalledHostedProvider,
    stdout_instance: Option<&str>,
    host_service_instance: Option<&str>,
    stdout_granted: bool,
) -> CandidateDocument {
    let manifest = installed.manifest;
    let artifact = installed.artifact;
    let mut authorities = stdout_instance
        .map(|instance| vec![stdout_authority(instance, stdout_granted)])
        .unwrap_or_default();
    if let Some(instance) = host_service_instance {
        authorities.push(host_service_authority(instance));
    }
    let format_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Format | HostedPrimitiveImplementation::FormatValuesLiteral
    );
    let buffered_text_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Lines | HostedPrimitiveImplementation::Join
    );
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
            required_authorities: manifest
                .required_authorities
                .iter()
                .map(ToString::to_string)
                .collect(),
            required_effects: Vec::new(),
            minimum_plan_version: manifest.minimum_plan_version,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION_V3,
            minimum_runtime_protocol: manifest.minimum_runtime_protocol,
            maximum_runtime_protocol: manifest.maximum_runtime_protocol,
            coexistence_memory_bytes: manifest.coexistence_memory_bytes,
        },
        execution_profile: if host_service_instance.is_some() {
            host_service_execution_profile()
        } else if format_profile {
            format_execution_profile()
        } else if buffered_text_profile {
            buffered_text_execution_profile()
        } else {
            execution_profile()
        },
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
                evidence_bytes: 64 * 1024,
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
            memory_bytes: if host_service_instance.is_some() {
                64 * 1024
            } else if format_profile || buffered_text_profile {
                128 * 1024
            } else {
                2048
            },
            cpu_units: 1,
            evidence_bytes: if format_profile || buffered_text_profile {
                8 * 1024
            } else {
                256
            },
            transports: u16::from(host_service_instance.is_some()),
            timers: u16::from(host_service_instance.is_some()),
            ..BudgetDocument::default()
        },
        lifecycle_policy: pin("conduit/finite-lifecycle", 60),
        capabilities: Vec::new(),
        resources: Vec::new(),
        topology: Vec::new(),
        granted_authorities: Vec::new(),
        authorities,
    }
}

fn stdout_authority(instance: &str, granted: bool) -> AuthorityDecisionDocument {
    let host = "conduit/conduct-host";
    AuthorityDecisionDocument {
        requirement: "sha256:8d4cf343da90c32b69b7f9037f5a687f5dd3e2afcd08cfdc3f73c7232f7e0801"
            .to_owned(),
        effect_hash: String::new(),
        grant_hash: String::new(),
        effect: EffectRequirementDocument {
            id: "conduit.effect/stdout-write".to_owned(),
            administrative_class: None,
            policy_budget_class: None,
            action: "conduit.action/write".to_owned(),
            resource_kind: "conduit.resource/output-stream".to_owned(),
            resource_id: Some("conduit.resource/stdout".to_owned()),
            requester: instance.to_owned(),
            audience: "conduit/conduct-run".to_owned(),
            constraints: Vec::new(),
            check_at_use: true,
        },
        capability: HostCapabilityDocument {
            id: "conduit.capability/stdout-write".to_owned(),
            action: "conduit.action/write".to_owned(),
            resource_kind: "conduit.resource/output-stream".to_owned(),
            resource_id: "conduit.resource/stdout".to_owned(),
            host: host.to_owned(),
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
        },
        grant: AuthorityGrantDocument {
            id: "conduit.grant/stdout-write".to_owned(),
            action: "conduit.action/write".to_owned(),
            resource_kind: "conduit.resource/output-stream".to_owned(),
            resource_id: "conduit.resource/stdout".to_owned(),
            scope_root: instance.to_owned(),
            scope_descendants: false,
            audience: "conduit/conduct-run".to_owned(),
            constraints: Vec::new(),
            time_basis: "clock/conduct-host".to_owned(),
            not_before_tick: 10,
            expires_at_tick: 20,
            issued_for_host: host.to_owned(),
            delegation: "none".to_owned(),
            audit_id: "conduit.audit/stdout-write".to_owned(),
            terminal_policy: "abort".to_owned(),
        },
        status: if granted { "active" } else { "revoked" }.to_owned(),
        administrative_subject: None,
        containment: None,
        policy_budgets: Vec::new(),
    }
}

fn host_service_authority(instance: &str) -> AuthorityDecisionDocument {
    let host = "conduit/conduct-host";
    AuthorityDecisionDocument {
        requirement: "sha256:4848484848484848484848484848484848484848484848484848484848484848"
            .to_owned(),
        effect_hash: String::new(),
        grant_hash: String::new(),
        effect: EffectRequirementDocument {
            id: "conduit.effect/http-loopback-listen".to_owned(),
            administrative_class: None,
            policy_budget_class: None,
            action: "conduit.action/listen".to_owned(),
            resource_kind: "conduit.resource/tcp-loopback".to_owned(),
            resource_id: Some("conduit.resource/ephemeral-loopback-port".to_owned()),
            requester: instance.to_owned(),
            audience: "conduit/conduct-run".to_owned(),
            constraints: Vec::new(),
            check_at_use: true,
        },
        capability: HostCapabilityDocument {
            id: "conduit.capability/http-loopback-listen".to_owned(),
            action: "conduit.action/listen".to_owned(),
            resource_kind: "conduit.resource/tcp-loopback".to_owned(),
            resource_id: "conduit.resource/ephemeral-loopback-port".to_owned(),
            host: host.to_owned(),
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
        },
        grant: AuthorityGrantDocument {
            id: "conduit.grant/http-loopback-listen".to_owned(),
            action: "conduit.action/listen".to_owned(),
            resource_kind: "conduit.resource/tcp-loopback".to_owned(),
            resource_id: "conduit.resource/ephemeral-loopback-port".to_owned(),
            scope_root: instance.to_owned(),
            scope_descendants: false,
            audience: "conduit/conduct-run".to_owned(),
            constraints: Vec::new(),
            time_basis: "clock/conduct-host".to_owned(),
            not_before_tick: 10,
            expires_at_tick: 20,
            issued_for_host: host.to_owned(),
            delegation: "none".to_owned(),
            audit_id: "conduit.audit/http-loopback-listen".to_owned(),
            terminal_policy: "abort".to_owned(),
        },
        status: "active".to_owned(),
        administrative_subject: None,
        containment: None,
        policy_budgets: Vec::new(),
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

fn format_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-format-profile-v1".to_owned(),
        schema_version: 1,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: conduit_std::FORMAT_MAX_WORK as u32,
            max_input_leases: 2,
            max_input_bytes: (conduit_std::FORMAT_MAX_TEMPLATE_BYTES
                + conduit_std::FORMAT_VALUES_MAX_ENCODED_BYTES) as u64,
            max_output_reservations: 1,
            max_output_bytes: conduit_std::FORMAT_MAX_OUTPUT_BYTES as u64,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: 3,
            max_retained_bytes: conduit_std::FORMAT_MAX_RETAINED_BYTES as u64,
            max_scratch_bytes: conduit_std::FORMAT_MAX_OUTPUT_BYTES as u32,
            implementation_memory_bytes: 128 * 1024,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: conduit_std::FORMAT_MAX_RETAINED_BYTES as u64,
            },
            MemoryClaimDocument {
                category: "step-scratch".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: conduit_std::FORMAT_MAX_OUTPUT_BYTES as u64,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: (128 * 1024
                    - conduit_std::FORMAT_MAX_RETAINED_BYTES
                    - conduit_std::FORMAT_MAX_OUTPUT_BYTES) as u64,
            },
        ],
        checkpoint: None,
    }
}

fn buffered_text_execution_profile() -> ExecutionProfileDocument {
    const RETAINED_BYTES: u64 =
        (conduit_std::JOIN_MAX_ITEMS * conduit_std::JOIN_MAX_ITEM_BYTES) as u64;
    const SCRATCH_BYTES: u64 = conduit_std::JOIN_MAX_OUTPUT_BYTES as u64;
    const MEMORY_BYTES: u64 = 128 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-buffered-text-profile-v1".to_owned(),
        schema_version: 1,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: conduit_std::JOIN_MAX_ITEMS as u32,
            max_input_leases: 1,
            max_input_bytes: RETAINED_BYTES,
            max_output_reservations: 1,
            max_output_bytes: conduit_std::JOIN_MAX_OUTPUT_BYTES as u64,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: conduit_std::JOIN_MAX_ITEMS as u16,
            max_retained_bytes: RETAINED_BYTES,
            max_scratch_bytes: conduit_std::JOIN_MAX_OUTPUT_BYTES as u32,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: RETAINED_BYTES,
            },
            MemoryClaimDocument {
                category: "step-scratch".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: SCRATCH_BYTES,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - RETAINED_BYTES - SCRATCH_BYTES,
            },
        ],
        checkpoint: None,
    }
}

fn host_service_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile-v1".to_owned(),
        schema_version: 1,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 30_000,
            max_transactions: 1,
            max_pending_operations: 1,
            max_timers: 1,
            max_host_buffer_bytes: 32 * 1024,
            implementation_memory_bytes: 64 * 1024,
            cancellation_ticks: 30_000,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "backend-bounded".to_owned(),
                bytes: 60 * 1024,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 4 * 1024,
            },
        ],
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
