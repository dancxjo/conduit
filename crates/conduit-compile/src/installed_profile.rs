use std::collections::BTreeMap;

use crate::{
    ArtifactDocument, ArtifactReferenceDocument, AuthorityConstraintDocument,
    AuthorityDecisionDocument, AuthorityGrantDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, EffectCommitProfileDocument, EffectRequirementDocument,
    ExecutionLimitsDocument, ExecutionProfileDocument, ExternalLeafContractDocument,
    HostCapabilityDocument, HostReportDocument, ImplementationDocument, MemoryClaimDocument,
    PinDocument, ResourceLeaseDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION, ExecutionPlan, ExecutorKind,
    IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::parse;
use conduit_runtime::{
    ExactGrantObservation, ExactHostedBinding, ExactHostedBindings, HostedPrimitiveImplementation,
    InstalledHostedProvider, OwnedNodeSchema, Registry, RuntimeError, SourceContractCatalog,
    hosted_effect_constraint_hash,
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
        let requires_large_evidence = topology.nodes.len() > 3
            || required.contains_key("std/text/format")
            || required.contains_key("std/format-values/literal")
            || required.contains_key("std/text/lines")
            || required.contains_key("std/text/join")
            || required.keys().any(|id| {
                id.starts_with("std/data/")
                    || id == "std/record/literal"
                    || id == "std/testing/assert-validation-decision"
            })
            || required.keys().any(|id| id.starts_with("time/"))
            || required.keys().any(|id| id.starts_with("state/"))
            || required
                .keys()
                .any(|id| id.starts_with("conduit.host/net/"))
            || required.keys().any(|id| id.starts_with("conduit.media/"))
            || required.keys().any(|id| {
                matches!(
                    id.as_str(),
                    "conduit.std/tee"
                        | "conduit.std/merge"
                        | "conduit.std/zip"
                        | "conduit.std/gate"
                        | "conduit.std/select"
                )
            });
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
            let host_service_instances = if installed.implementation
                == HostedPrimitiveImplementation::HostedService
            {
                instances
                    .iter()
                    .map(|instance| {
                        let constraints = panel
                            .nodes
                            .iter()
                            .find(|node| {
                                node.id == *instance || instance.ends_with(&format!("/{}", node.id))
                            })
                            .map_or_else(Vec::new, hosted_service_authority_constraints);
                        (instance.clone(), constraints)
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            candidates.push(candidate(
                installed,
                stdout_instance.as_deref(),
                &host_service_instances,
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
            let node_missing = !catalog
                .nodes
                .iter()
                .any(|node| node.id == contract.id.as_str());
            if node_missing {
                catalog.nodes.push(PinDocument {
                    id: contract.id.to_string(),
                    schema_version: 0,
                    semantic_hash: OwnedNodeSchema::from_contract(contract)
                        .semantic_hash()
                        .to_string(),
                });
            }
            if Registry::default()
                .node_schema(contract.id.as_str())
                .is_none()
            {
                catalog.external_leaf_contracts.push(
                    ExternalLeafContractDocument::from_contract(contract).ok_or_else(|| {
                        RuntimeError::new(
                            "CND-CMP-002",
                            format!(
                                "external contract `{}` has configuration that cannot be sealed",
                                contract.id.as_str()
                            ),
                        )
                    })?,
                );
            }
            for value_type in contract
                .config
                .fields
                .iter()
                .map(|field| field.value_type)
                .chain(contract.inputs.iter().map(|port| port.value_type))
                .chain(contract.outputs.iter().map(|port| port.value_type))
            {
                if !catalog
                    .types
                    .iter()
                    .any(|entry| entry.id == value_type.contract_id.as_str())
                {
                    catalog.types.push(PinDocument {
                        id: value_type.contract_id.to_string(),
                        schema_version: value_type.schema_version,
                        semantic_hash: value_type.semantic_hash.to_string(),
                    });
                }
            }
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
                memory_bytes: 4 * 1024 * 1024,
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
            if !bindings.iter().any(|binding: &ExactHostedBinding| {
                binding.implementation_id == node.implementation.id.as_str()
                    && binding.implementation_identity == node.implementation.semantic_hash
            }) {
                bindings.push(ExactHostedBinding {
                    implementation_id: node.implementation.id.to_string(),
                    implementation_identity: node.implementation.semantic_hash,
                    artifact_id: installed.artifact.id.to_string(),
                    artifact_digest: installed.artifact.digest,
                    implementation,
                });
            }
        }
        ExactHostedBindings::new(bindings)
    }

    /// Projects fresh grant status from this host observation snapshot. The
    /// exact plan supplies immutable grant identity and scope, never status.
    pub fn grant_observations<'a>(
        &'a self,
        plan: &'a ExecutionPlan<'a>,
    ) -> Result<Vec<ExactGrantObservation<'a>>, RuntimeError> {
        plan.authorities
            .iter()
            .map(|authority| {
                let planned = plan
                    .nodes
                    .iter()
                    .find(|node| node.instance == authority.node)
                    .ok_or_else(|| {
                        RuntimeError::new(
                            "CND-RUN-010",
                            "authority refers to an absent exact-plan node",
                        )
                    })?;
                let observed = self
                    .input
                    .candidates
                    .iter()
                    .find(|candidate| {
                        candidate.implementation.id == planned.implementation.id.as_str()
                            && candidate.host_report.id == planned.host_observation.as_str()
                    })
                    .and_then(|candidate| {
                        candidate.authorities.iter().find(|observed| {
                            observed.grant.id == authority.grant.id.as_str()
                                && observed.effect_hash == authority.effect_hash.to_string()
                        })
                    })
                    .ok_or_else(|| {
                        RuntimeError::new("CND-RUN-010", "fresh host grant observation is absent")
                    })?;
                let status = match observed.status.as_str() {
                    "active" => conduit_core::GrantStatus::Active,
                    "revoked" => conduit_core::GrantStatus::Revoked {
                        at_tick: plan.created_at.tick,
                        reason: conduit_core::Id("host/revoked-at-use"),
                    },
                    _ => {
                        return Err(RuntimeError::new(
                            "CND-RUN-010",
                            "fresh host grant observation has an unknown status",
                        ));
                    }
                };
                let resource = plan
                    .resources
                    .iter()
                    .find(|resource| {
                        resource.node == authority.node
                            && resource.resource == authority.binding.resource
                    })
                    .ok_or_else(|| {
                        RuntimeError::new(
                            "CND-RUN-010",
                            "fresh host resource observation is absent",
                        )
                    })?;
                Ok(ExactGrantObservation {
                    grant: authority.grant.id,
                    status,
                    resource_binding: conduit_core::Id(&observed.resource_lease.resource_binding),
                    resource_lease: conduit_core::Id(&observed.resource_lease.id),
                    lease_valid_until_tick: observed.resource_lease.expires_at_tick,
                    lease_available: resource.lease.is_some(),
                })
            })
            .collect()
    }
}

fn candidate(
    installed: &conduit_runtime::InstalledHostedProvider,
    stdout_instance: Option<&str>,
    host_service_instances: &[(String, Vec<AuthorityConstraintDocument>)],
    stdout_granted: bool,
) -> CandidateDocument {
    let manifest = installed.manifest;
    let artifact = installed.artifact;
    let mut authorities = stdout_instance
        .map(|instance| vec![stdout_authority(instance, stdout_granted)])
        .unwrap_or_default();
    for (instance, constraints) in host_service_instances {
        authorities.extend(host_service_authority(
            installed.contract.id.as_str(),
            instance,
            constraints,
        ));
    }
    let has_host_service = !host_service_instances.is_empty();
    let format_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Format | HostedPrimitiveImplementation::FormatValuesLiteral
    );
    let buffered_text_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Lines | HostedPrimitiveImplementation::Join
    );
    let data_boundary_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::DataEncodeUtf8
            | HostedPrimitiveImplementation::DataDecodeUtf8
            | HostedPrimitiveImplementation::FrameLengthU32Be
            | HostedPrimitiveImplementation::DeframeLengthU32Be
    );
    let structural_validation_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::RecordLiteral
            | HostedPrimitiveImplementation::ValidateClosedRecord
    );
    let structural_flow_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Tee
            | HostedPrimitiveImplementation::Merge
            | HostedPrimitiveImplementation::Zip
            | HostedPrimitiveImplementation::Gate
            | HostedPrimitiveImplementation::Select
    );
    let time_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::TimeDelay
            | HostedPrimitiveImplementation::TimeTimeout
            | HostedPrimitiveImplementation::TimeDebounce
            | HostedPrimitiveImplementation::TimeThrottle
    );
    let state_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::StateCell
            | HostedPrimitiveImplementation::StateDeduplicate
            | HostedPrimitiveImplementation::StateCache
    );
    let supervision_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::SupervisionRetry
            | HostedPrimitiveImplementation::SupervisionCircuitBreaker
    );
    let host_io_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Stdin
            | HostedPrimitiveImplementation::StdinStream
            | HostedPrimitiveImplementation::Stdout
            | HostedPrimitiveImplementation::Stderr
            | HostedPrimitiveImplementation::StdoutStream
            | HostedPrimitiveImplementation::StderrStream
            | HostedPrimitiveImplementation::DisplayText
    );
    let process_profile = installed.contract.id.as_str() == "conduit.host/process/exec";
    let socket_profile = installed
        .contract
        .id
        .as_str()
        .starts_with("conduit.host/net/");
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
            execution_profile: pin("conduit/hosted-primitive-profile", 30),
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
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            minimum_runtime_protocol: manifest.minimum_runtime_protocol,
            maximum_runtime_protocol: manifest.maximum_runtime_protocol,
            coexistence_memory_bytes: manifest.coexistence_memory_bytes,
        },
        execution_profile: if process_profile {
            process_execution_profile()
        } else if socket_profile {
            socket_execution_profile()
        } else if has_host_service {
            host_service_execution_profile()
        } else if host_io_profile {
            host_io_execution_profile()
        } else if format_profile {
            format_execution_profile()
        } else if buffered_text_profile {
            buffered_text_execution_profile()
        } else if data_boundary_profile {
            data_boundary_execution_profile()
        } else if structural_validation_profile {
            structural_validation_execution_profile()
        } else if structural_flow_profile {
            structural_flow_execution_profile()
        } else if time_profile {
            time_execution_profile()
        } else if state_profile {
            state_execution_profile()
        } else if supervision_profile {
            supervision_execution_profile()
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
            id: "conduit/conduct-host-observation".to_owned(),
            host: "conduit/conduct-host".to_owned(),
            reporter: pin("conduit/conduct-host-reporter", 50),
            trust: pin("conduit/local-build-trust", 51),
            membership: None,
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
            available: BudgetDocument {
                memory_bytes: 4 * 1024 * 1024,
                storage_bytes: 16 * 1024 * 1024,
                cpu_units: 64,
                timers: 16,
                transports: 16,
                checkpoints: 16,
                evidence_bytes: 256 * 1024,
            },
            capabilities: Vec::new(),
            resources: Vec::new(),
            topology: Vec::new(),
            supported_executors: vec![executor_name(manifest.executor).to_owned()],
            supported_targets: Vec::new(),
            supported_abis: Vec::new(),
            minimum_plan_version: manifest.minimum_plan_version,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            current_constraints: Vec::new(),
        },
        allocation: BudgetDocument {
            memory_bytes: if process_profile {
                448 * 1024
            } else if socket_profile {
                256 * 1024
            } else if has_host_service {
                192 * 1024
            } else if host_io_profile {
                3 * 1024
            } else if structural_validation_profile {
                576 * 1024
            } else if format_profile || buffered_text_profile || data_boundary_profile {
                128 * 1024
            } else if structural_flow_profile {
                8 * 1024
            } else if time_profile || state_profile || supervision_profile {
                256 * 1024
            } else {
                2048
            },
            cpu_units: 1,
            evidence_bytes: if process_profile || socket_profile {
                16 * 1024
            } else if format_profile
                || buffered_text_profile
                || data_boundary_profile
                || structural_validation_profile
            {
                8 * 1024
            } else if structural_flow_profile {
                4 * 1024
            } else if time_profile || state_profile || supervision_profile {
                8 * 1024
            } else {
                256
            },
            transports: u16::from(has_host_service && !process_profile && !socket_profile)
                + u16::from(socket_profile),
            timers: if process_profile || socket_profile || supervision_profile {
                2
            } else {
                u16::from(has_host_service || time_profile)
            },
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

fn hosted_service_authority_constraints(
    node: &conduit_panel::Node,
) -> Vec<AuthorityConstraintDocument> {
    if node.config("address").is_none()
        || node.config("authority").is_none()
        || node.config("transport").is_none()
    {
        return Vec::new();
    }
    [
        (
            "conduit.constraint/http-authority",
            node.config("authority").unwrap_or_default(),
        ),
        (
            "conduit.constraint/http-endpoint",
            node.config("address").unwrap_or_default(),
        ),
        (
            "conduit.constraint/http-transport",
            node.config("transport").unwrap_or("http"),
        ),
    ]
    .into_iter()
    .map(|(id, value)| AuthorityConstraintDocument {
        id: id.to_owned(),
        semantic_hash: hosted_effect_constraint_hash(id, value.as_bytes()).to_string(),
    })
    .collect()
}

fn stdout_authority(instance: &str, granted: bool) -> AuthorityDecisionDocument {
    let host = "conduit/conduct-host";
    let (resource_lease, commit_profile) = effect_contracts(
        "stdout-write",
        instance,
        "conduit.action/write",
        "conduit.resource/stdout",
    );
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
        resource_lease,
        commit_profile,
    }
}

fn host_service_authority(
    contract_id: &str,
    instance: &str,
    constraints: &[AuthorityConstraintDocument],
) -> Option<AuthorityDecisionDocument> {
    let host = "conduit/conduct-host";
    let (name, requirement, action, resource_kind, resource_id) = match contract_id {
        "fs/read" => (
            "filesystem-read",
            "sha256:3131313131313131313131313131313131313131313131313131313131313131",
            "conduit.action/read",
            "conduit.resource/filesystem-file",
            "conduit.resource/filesystem-example-read",
        ),
        "fs/write" => (
            "filesystem-write",
            "sha256:3232323232323232323232323232323232323232323232323232323232323232",
            "conduit.action/write",
            "conduit.resource/filesystem-file",
            "conduit.resource/filesystem-example-write",
        ),
        "fs/watch" => (
            "filesystem-watch",
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            "conduit.action/watch",
            "conduit.resource/filesystem-file",
            "conduit.resource/filesystem-example-watch",
        ),
        "storage/cache/put" => (
            "storage-cache-put",
            "sha256:4141414141414141414141414141414141414141414141414141414141414141",
            "conduit.action/cache-put",
            "conduit.resource/evictable-blob-cache",
            "conduit.resource/storage-cache-example-put",
        ),
        "storage/cache/get" => (
            "storage-cache-get",
            "sha256:4242424242424242424242424242424242424242424242424242424242424242",
            "conduit.action/cache-get",
            "conduit.resource/evictable-blob-cache",
            "conduit.resource/storage-cache-example-get",
        ),
        "storage/cache/remove" => (
            "storage-cache-remove",
            "sha256:4343434343434343434343434343434343434343434343434343434343434343",
            "conduit.action/cache-remove",
            "conduit.resource/evictable-blob-cache",
            "conduit.resource/storage-cache-example-remove",
        ),
        "conduit.host/process/exec" => (
            "process-exec",
            "sha256:5050505050505050505050505050505050505050505050505050505050505050",
            "conduit.action/execute",
            "conduit.resource/executable",
            "conduit.executable/process-fixture",
        ),
        "conduit.host/net/tcp/connect" => (
            "socket-tcp-connect",
            "sha256:6161616161616161616161616161616161616161616161616161616161616161",
            "conduit.action/connect",
            "conduit.resource/tcp-loopback",
            "conduit.resource/socket-loopback",
        ),
        "conduit.host/net/tcp/listen" => (
            "socket-tcp-listen",
            "sha256:6262626262626262626262626262626262626262626262626262626262626262",
            "conduit.action/listen",
            "conduit.resource/tcp-loopback",
            "conduit.resource/socket-loopback",
        ),
        "conduit.host/net/udp/connected" => (
            "socket-udp-connected",
            "sha256:6363636363636363636363636363636363636363636363636363636363636363",
            "conduit.action/connect",
            "conduit.resource/udp-loopback",
            "conduit.resource/socket-loopback",
        ),
        "conduit.host/net/udp/datagram" => (
            "socket-udp-datagram",
            "sha256:6464646464646464646464646464646464646464646464646464646464646464",
            "conduit.action/bind",
            "conduit.resource/udp-loopback",
            "conduit.resource/socket-loopback",
        ),
        "net/http/serve-once" => (
            "http-loopback-listen",
            "sha256:4848484848484848484848484848484848484848484848484848484848484848",
            "conduit.action/listen",
            "conduit.resource/tcp-loopback",
            "conduit.resource/ephemeral-loopback-port",
        ),
        "net/http/fetch" => (
            "http-loopback-request",
            "sha256:4949494949494949494949494949494949494949494949494949494949494949",
            "conduit.action/request",
            "conduit.resource/http-loopback",
            "conduit.resource/http-loopback",
        ),
        _ => return None,
    };
    let (resource_lease, commit_profile) = effect_contracts(name, instance, action, resource_id);
    Some(AuthorityDecisionDocument {
        requirement: requirement.to_owned(),
        effect_hash: String::new(),
        grant_hash: String::new(),
        effect: EffectRequirementDocument {
            id: format!("conduit.effect/{name}"),
            administrative_class: None,
            policy_budget_class: None,
            action: action.to_owned(),
            resource_kind: resource_kind.to_owned(),
            resource_id: Some(resource_id.to_owned()),
            requester: instance.to_owned(),
            audience: "conduit/conduct-run".to_owned(),
            constraints: constraints.to_vec(),
            check_at_use: true,
        },
        capability: HostCapabilityDocument {
            id: format!("conduit.capability/{name}"),
            action: action.to_owned(),
            resource_kind: resource_kind.to_owned(),
            resource_id: resource_id.to_owned(),
            host: host.to_owned(),
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
        },
        grant: AuthorityGrantDocument {
            id: format!("conduit.grant/{name}"),
            action: action.to_owned(),
            resource_kind: resource_kind.to_owned(),
            resource_id: resource_id.to_owned(),
            scope_root: instance.to_owned(),
            scope_descendants: false,
            audience: "conduit/conduct-run".to_owned(),
            constraints: constraints.to_vec(),
            time_basis: "clock/conduct-host".to_owned(),
            not_before_tick: 10,
            expires_at_tick: 20,
            issued_for_host: host.to_owned(),
            delegation: "none".to_owned(),
            audit_id: format!("conduit.audit/{name}"),
            terminal_policy: "abort".to_owned(),
        },
        status: "active".to_owned(),
        administrative_subject: None,
        containment: None,
        policy_budgets: Vec::new(),
        resource_lease,
        commit_profile,
    })
}

fn effect_contracts(
    name: &str,
    holder: &str,
    operation: &str,
    resource_binding: &str,
) -> (ResourceLeaseDocument, EffectCommitProfileDocument) {
    let lease_id = format!("conduit.lease/{name}");
    (
        ResourceLeaseDocument {
            schema_version: conduit_core::RESOURCE_LEASE_SCHEMA_VERSION,
            id: lease_id.clone(),
            resource_binding: resource_binding.to_owned(),
            holder: holder.to_owned(),
            run: "conduit/conduct-run".to_owned(),
            epoch: 0,
            scope: format!("conduit.scope/{name}"),
            sharing: "exclusive".to_owned(),
            maximum_holders: 1,
            reservation: BudgetDocument {
                memory_bytes: 1,
                ..BudgetDocument::default()
            },
            time_basis: "clock/conduct-host".to_owned(),
            issued_at_tick: 10,
            expires_at_tick: 20,
            revocation_grace_ticks: 1,
            cleanup_ticks: 2,
            maximum_operations: 1024,
            maximum_evidence_events: 2048,
            cleanup_escalation: pin(&format!("conduit.cleanup/{name}"), 71),
            foreign_retention: "unsupported".to_owned(),
            foreign_maximum_bytes: 0,
            foreign_release_ticks: 0,
        },
        EffectCommitProfileDocument {
            schema_version: conduit_core::EFFECT_COMMIT_PROFILE_SCHEMA_VERSION,
            id: format!("conduit.effect-profile/{name}"),
            operation: operation.to_owned(),
            resource_lease: lease_id,
            commit_boundary: pin(&format!("conduit.commit/{name}"), 72),
            idempotency: "reconcile-before-retry".to_owned(),
            unknown_commit: "reconcile".to_owned(),
            discontinuity: "reconcile-required".to_owned(),
            cleanup: pin(&format!("conduit.cleanup-profile/{name}"), 73),
            maximum_attempts: 1,
            evidence_events_per_attempt: 2,
        },
    )
}

fn execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile".to_owned(),
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

fn host_io_execution_profile() -> ExecutionProfileDocument {
    const BUFFER_BYTES: u64 = 1024;
    const MEMORY_BYTES: u64 = 3 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-io-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 4,
            max_input_leases: 1,
            max_input_bytes: BUFFER_BYTES,
            max_output_reservations: 1,
            max_output_bytes: BUFFER_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_host_buffer_bytes: BUFFER_BYTES,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: BUFFER_BYTES,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - BUFFER_BYTES,
            },
        ],
        checkpoint: None,
    }
}

fn structural_flow_execution_profile() -> ExecutionProfileDocument {
    const RETAINED_BYTES: u64 = 2 * 1024;
    const MEMORY_BYTES: u64 = 8 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-structural-flow-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 4,
            max_input_leases: 2,
            max_input_bytes: RETAINED_BYTES,
            max_output_reservations: 2,
            max_output_bytes: RETAINED_BYTES,
            max_transactions: 2,
            max_fragments_per_step: 2,
            max_retained_values: 2,
            max_retained_bytes: RETAINED_BYTES,
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
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - RETAINED_BYTES,
            },
        ],
        checkpoint: None,
    }
}

fn format_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-format-profile".to_owned(),
        schema_version: 0,
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
        id: "conduit/hosted-buffered-text-profile".to_owned(),
        schema_version: 0,
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

fn data_boundary_execution_profile() -> ExecutionProfileDocument {
    const VALUE_BYTES: u64 =
        conduit_std::DATA_MAX_FRAME_BYTES as u64 + conduit_std::LENGTH_U32BE_PREFIX_BYTES as u64;
    const MEMORY_BYTES: u64 = 32 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-data-boundary-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: VALUE_BYTES as u32,
            max_input_leases: 1,
            max_input_bytes: VALUE_BYTES,
            max_output_reservations: 1,
            max_output_bytes: VALUE_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: 1,
            max_retained_bytes: VALUE_BYTES,
            max_scratch_bytes: VALUE_BYTES as u32,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "step-scratch".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - (2 * VALUE_BYTES),
            },
        ],
        checkpoint: None,
    }
}

fn time_execution_profile() -> ExecutionProfileDocument {
    const VALUE_BYTES: u64 = 65_536;
    const MEMORY_BYTES: u64 = 256 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-time-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 4,
            max_input_leases: 1,
            max_input_bytes: VALUE_BYTES,
            max_output_reservations: 1,
            max_output_bytes: VALUE_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: 1,
            max_retained_bytes: VALUE_BYTES,
            max_pending_operations: 1,
            max_timers: 1,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 256,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - VALUE_BYTES - 256,
            },
        ],
        checkpoint: None,
    }
}

fn state_execution_profile() -> ExecutionProfileDocument {
    const VALUE_BYTES: u64 = conduit_std::STATE_MAX_VALUE_BYTES;
    const MEMORY_BYTES: u64 = 256 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-state-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: conduit_std::STATE_MAX_ENTRIES as u32,
            max_input_leases: 1,
            max_input_bytes: VALUE_BYTES,
            max_output_reservations: 1,
            max_output_bytes: VALUE_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: conduit_std::STATE_MAX_ENTRIES as u16,
            max_retained_bytes: VALUE_BYTES,
            max_pending_operations: 1,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 256,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - VALUE_BYTES - 256,
            },
        ],
        checkpoint: None,
    }
}

fn supervision_execution_profile() -> ExecutionProfileDocument {
    const VALUE_BYTES: u64 = 65_536;
    const MEMORY_BYTES: u64 = 256 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-supervision-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 8,
            max_input_leases: 1,
            max_input_bytes: VALUE_BYTES,
            max_output_reservations: 1,
            max_output_bytes: VALUE_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_retained_values: 4,
            max_retained_bytes: VALUE_BYTES,
            max_pending_operations: 1,
            max_timers: 2,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 256,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - VALUE_BYTES - 256,
            },
        ],
        checkpoint: None,
    }
}

fn structural_validation_execution_profile() -> ExecutionProfileDocument {
    const VALUE_BYTES: u64 = conduit_std::DATA_MAX_RECORD_BYTES as u64;
    const DECISION_BYTES: u64 = 5;
    const MEMORY_BYTES: u64 = 576 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-structural-validation-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: VALUE_BYTES as u32,
            max_input_leases: 1,
            max_input_bytes: VALUE_BYTES,
            max_output_reservations: 2,
            max_output_bytes: VALUE_BYTES + DECISION_BYTES,
            max_transactions: 1,
            max_fragments_per_step: 2,
            max_retained_values: 2,
            max_retained_bytes: VALUE_BYTES + DECISION_BYTES,
            max_scratch_bytes: VALUE_BYTES as u32,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 1,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "retained".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES + DECISION_BYTES,
            },
            MemoryClaimDocument {
                category: "step-scratch".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: VALUE_BYTES,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - (2 * VALUE_BYTES) - DECISION_BYTES,
            },
        ],
        checkpoint: None,
    }
}

fn host_service_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 30_000,
            max_transactions: 1,
            max_input_leases: 8,
            max_input_bytes: 64 * 1024,
            max_output_reservations: 8,
            max_output_bytes: 64 * 1024,
            max_pending_operations: 1,
            max_timers: 1,
            max_host_buffer_bytes: 32 * 1024,
            implementation_memory_bytes: 192 * 1024,
            cancellation_ticks: 30_000,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "backend-bounded".to_owned(),
                bytes: 124 * 1024,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 4 * 1024,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 64 * 1024,
            },
        ],
        checkpoint: None,
    }
}

fn process_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: 64 * 1024,
            max_transactions: 3,
            max_input_leases: 4,
            max_input_bytes: conduit_std::PROCESS_MAX_STREAM_BYTES as u64,
            max_output_reservations: 8,
            max_output_bytes: (conduit_std::PROCESS_MAX_STREAM_BYTES * 2) as u64,
            max_pending_operations: 3,
            max_timers: 2,
            max_host_buffer_bytes: (conduit_std::PROCESS_MAX_STREAM_BYTES * 3) as u64,
            implementation_memory_bytes: 448 * 1024,
            cancellation_ticks: 10_000,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "backend-bounded".to_owned(),
                bytes: 192 * 1024,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 16 * 1024,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 240 * 1024,
            },
        ],
        checkpoint: None,
    }
}

fn socket_execution_profile() -> ExecutionProfileDocument {
    ExecutionProfileDocument {
        id: "conduit/hosted-primitive-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "hard".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: true,
        limits: ExecutionLimitsDocument {
            max_step_work: conduit_std::SOCKET_MAX_MESSAGE_BYTES as u32,
            max_transactions: conduit_std::SOCKET_MAX_SESSIONS as u16,
            max_input_leases: conduit_std::SOCKET_MAX_SESSIONS as u16,
            max_input_bytes: conduit_std::SOCKET_MAX_STREAM_BYTES as u64,
            max_output_reservations: conduit_std::SOCKET_MAX_SESSIONS as u16,
            max_output_bytes: conduit_std::SOCKET_MAX_STREAM_BYTES as u64,
            max_pending_operations: 4,
            max_timers: 2,
            max_host_buffer_bytes: (conduit_std::SOCKET_MAX_STREAM_BYTES * 2) as u64,
            implementation_memory_bytes: 256 * 1024,
            cancellation_ticks: 10_000,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "backend-bounded".to_owned(),
                bytes: 96 * 1024,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 32 * 1024,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: 128 * 1024,
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
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]).to_string(),
    }
}
