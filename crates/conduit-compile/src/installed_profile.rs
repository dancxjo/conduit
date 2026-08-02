use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ArtifactDocument, ArtifactReferenceDocument, AuthorityConstraintDocument,
    AuthorityDecisionDocument, AuthorityGrantDocument, BudgetDocument, COMPILE_INPUT_SCHEMA,
    COMPILE_INPUT_SCHEMA_VERSION, CandidateDocument, CompileInput, CompileModuleDocument,
    CompileSourceLimits, EffectCommitProfileDocument, EffectRequirementDocument,
    EvidenceProviderBindingDocument, ExecutionLimitsDocument, ExecutionProfileDocument,
    ExternalLeafContractDocument, HostCapabilityDocument, HostReportDocument,
    ImplementationDocument, ImplementationInterfaceDocument, MemoryClaimDocument, PinDocument,
    ResourceLeaseDocument, WatchAdmissionDocument, builtin_catalog_document,
};
use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION, ExecutionPlan, ExecutorKind,
    IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, SemanticHash,
};
use conduit_panel::{SourceValue, parse};
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

/// Caller-supplied snapshot identity for one host observing an installed
/// registry. Installation and discovery do not manufacture this fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledHostObservationInput {
    pub id: String,
    pub host: String,
    pub boot_id: String,
    pub reporter: PinDocument,
    pub trust: PinDocument,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub current_tick: u64,
    pub available: BudgetDocument,
}

impl InstalledHostObservationInput {
    #[must_use]
    pub fn conduct_host() -> Self {
        Self {
            id: "conduit/conduct-host-observation".to_owned(),
            host: "conduit/conduct-host".to_owned(),
            boot_id: "conduit/conduct-host-boot".to_owned(),
            reporter: pin("conduit/conduct-host-reporter", 50),
            trust: pin("conduit/local-build-trust", 51),
            time_basis: "clock/conduct-host".to_owned(),
            observed_at_tick: 10,
            valid_until_tick: 20,
            current_tick: 12,
            available: BudgetDocument {
                memory_bytes: 4 * 1024 * 1024,
                storage_bytes: 16 * 1024 * 1024,
                cpu_units: 64,
                timers: 16,
                transports: 16,
                checkpoints: 16,
                evidence_bytes: 256 * 1024,
            },
        }
    }
}

/// One authority decision observed independently by the host and offered to
/// the resolver for a specific semantic service instance. Panel source cannot
/// construct this value or make its status current merely by requesting the
/// associated contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedHostServiceAuthority {
    pub contract_id: String,
    pub instance: String,
    pub decision: AuthorityDecisionDocument,
}

/// Host-policy input for one observed service authority. The caller owns the
/// policy decision; source can only be checked against the resulting exact
/// resource, grant, lease, and constraint identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostServiceAuthorityObservationInput {
    pub contract_id: String,
    pub instance: String,
    pub run_id: String,
    pub epoch: u64,
    pub constraints: Vec<AuthorityConstraintDocument>,
    pub resource_id: Option<String>,
    pub grant_id: Option<String>,
    pub sharing: Option<String>,
    pub maximum_holders: Option<u16>,
    pub lease_ticks: Option<u64>,
    pub revocation_grace_ticks: Option<u64>,
    pub cleanup_ticks: Option<u64>,
}

/// Caller-owned policy input for an optional implementation whose effect is
/// not defined by a built-in semantic contract.
///
/// This is the generic extension path for domain providers such as process,
/// FFI, WASM-host, device, or remote adapters. Provider installation cannot
/// call this implicitly: the host must independently name the action,
/// resource, requirement, grant, lease, and run audience.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalHostServiceAuthorityObservationInput {
    pub contract_id: String,
    pub instance: String,
    pub run_id: String,
    pub epoch: u64,
    pub host: String,
    pub time_basis: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub name: String,
    pub requirement: String,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub grant_id: String,
    pub constraints: Vec<AuthorityConstraintDocument>,
    pub revocation_grace_ticks: u64,
    pub cleanup_ticks: u64,
}

/// Commits host-observed opaque values to the semantic constraint identifiers
/// owned by an effect domain.
#[must_use]
pub fn observed_host_service_constraints(
    values: &[(&str, &[u8])],
) -> Vec<AuthorityConstraintDocument> {
    values
        .iter()
        .map(|(id, value)| AuthorityConstraintDocument {
            id: (*id).to_owned(),
            semantic_hash: hosted_effect_constraint_hash(id, value).to_string(),
        })
        .collect()
}

/// Converts an independently observed host-policy decision into compile input
/// without deriving authority from panel source or provider installation.
#[must_use]
pub fn observed_host_service_authority(
    input: HostServiceAuthorityObservationInput,
) -> Option<ObservedHostServiceAuthority> {
    let instance = HostedServiceInstance {
        instance: input.instance.clone(),
        constraints: input.constraints,
        resource_id: input.resource_id,
        grant_id: input.grant_id,
        sharing: input.sharing,
        maximum_holders: input.maximum_holders,
        lease_ticks: input.lease_ticks,
        revocation_grace_ticks: input.revocation_grace_ticks,
        cleanup_ticks: input.cleanup_ticks,
    };
    host_service_authority(&input.contract_id, &instance).map(|mut decision| {
        decision.effect.audience = input.run_id.clone();
        decision.grant.audience = input.run_id.clone();
        decision.resource_lease.run = input.run_id;
        decision.resource_lease.epoch = input.epoch;
        ObservedHostServiceAuthority {
            contract_id: input.contract_id,
            instance: input.instance,
            decision,
        }
    })
}

/// Admit one independently observed authority for a generically installed
/// implementation. This function creates no provider, artifact, observation,
/// or selection and is never called by descriptor discovery.
#[must_use]
pub fn observed_external_host_service_authority(
    input: ExternalHostServiceAuthorityObservationInput,
) -> ObservedHostServiceAuthority {
    let (mut resource_lease, commit_profile) = effect_contracts(
        &input.name,
        &input.instance,
        &input.action,
        &input.resource_id,
    );
    resource_lease.run = input.run_id.clone();
    resource_lease.epoch = input.epoch;
    resource_lease.time_basis = input.time_basis.clone();
    resource_lease.issued_at_tick = input.observed_at_tick;
    resource_lease.expires_at_tick = input.valid_until_tick;
    resource_lease.revocation_grace_ticks = input.revocation_grace_ticks;
    resource_lease.cleanup_ticks = input.cleanup_ticks;
    let valid_until_tick = input.valid_until_tick;
    ObservedHostServiceAuthority {
        contract_id: input.contract_id,
        instance: input.instance.clone(),
        decision: AuthorityDecisionDocument {
            requirement: input.requirement,
            effect_hash: String::new(),
            grant_hash: String::new(),
            effect: EffectRequirementDocument {
                id: format!("conduit.effect/{}", input.name),
                administrative_class: None,
                policy_budget_class: None,
                action: input.action.clone(),
                resource_kind: input.resource_kind.clone(),
                resource_id: Some(input.resource_id.clone()),
                requester: input.instance.clone(),
                audience: input.run_id.clone(),
                constraints: input.constraints.clone(),
                check_at_use: true,
            },
            capability: HostCapabilityDocument {
                id: format!("conduit.capability/{}", input.name),
                action: input.action.clone(),
                resource_kind: input.resource_kind.clone(),
                resource_id: input.resource_id.clone(),
                host: input.host.clone(),
                time_basis: input.time_basis.clone(),
                observed_at_tick: input.observed_at_tick,
                valid_until_tick,
            },
            grant: AuthorityGrantDocument {
                id: input.grant_id,
                action: input.action,
                resource_kind: input.resource_kind,
                resource_id: input.resource_id,
                scope_root: input.instance,
                scope_descendants: false,
                audience: input.run_id,
                constraints: input.constraints,
                time_basis: input.time_basis,
                not_before_tick: input.observed_at_tick,
                expires_at_tick: valid_until_tick,
                issued_for_host: input.host,
                delegation: "none".to_owned(),
                audit_id: format!("conduit.audit/{}", input.name),
                terminal_policy: "abort".to_owned(),
            },
            status: "active".to_owned(),
            administrative_subject: None,
            containment: None,
            policy_budgets: Vec::new(),
            resource_lease,
            commit_profile,
        },
    }
}

struct HostedServiceInstance {
    instance: String,
    constraints: Vec<AuthorityConstraintDocument>,
    resource_id: Option<String>,
    grant_id: Option<String>,
    sharing: Option<String>,
    maximum_holders: Option<u16>,
    lease_ticks: Option<u64>,
    revocation_grace_ticks: Option<u64>,
    cleanup_ticks: Option<u64>,
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

    /// Observe an explicitly assembled registry together with independently
    /// supplied host authority facts. This is the production entry point for
    /// effectful providers whose grants must not be inferred from panel
    /// source or provider installation.
    pub fn observe_registry_with_host_authorities(
        source: &str,
        registry: &Registry,
        authorities: &[ObservedHostServiceAuthority],
    ) -> Result<Self, RuntimeError> {
        Self::observe_registry_with_stdout_grant_and_host_authorities(
            source,
            registry,
            true,
            authorities,
            &InstalledHostObservationInput::conduct_host(),
        )
    }

    /// Observe one explicitly named host snapshot. The host identity and
    /// freshness window are independent of the implementations installed in
    /// the registry, so the same inventory can be observed on multiple hosts
    /// without changing semantic contracts.
    pub fn observe_registry_on_host(
        source: &str,
        registry: &Registry,
        observation: &InstalledHostObservationInput,
        authorities: &[ObservedHostServiceAuthority],
    ) -> Result<Self, RuntimeError> {
        Self::observe_registry_with_stdout_grant_and_host_authorities(
            source,
            registry,
            true,
            authorities,
            observation,
        )
    }

    /// Adds one independently observed host evidence-provider binding to the
    /// compile input and reseals its exact identity. The observation remains
    /// distinct from panel source and from later use-time grant/lease status.
    pub fn with_evidence_provider_observation(
        mut self,
        provider: EvidenceProviderBindingDocument,
    ) -> Result<Self, RuntimeError> {
        self.input.evidence_provider = Some(provider);
        self.input
            .seal()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        Ok(self)
    }

    /// Adds exact host-policy Watch admissions before compilation seals the
    /// plan. These controls are compiler input, never a post-compile identity
    /// rewrite or a Patchbay presentation claim.
    pub fn with_watch_admissions(
        mut self,
        admissions: Vec<WatchAdmissionDocument>,
    ) -> Result<Self, RuntimeError> {
        self.input.watch_admissions = admissions;
        self.input
            .seal()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        Ok(self)
    }

    /// Applies caller-owned provider preference without changing source or
    /// semantic contracts. Entries are implementation IDs observed in this
    /// profile; the exact resolver still rejects incompatible candidates.
    pub fn with_implementation_preference(
        mut self,
        implementation_ids: Vec<String>,
    ) -> Result<Self, RuntimeError> {
        self.input.implementation_preference = implementation_ids;
        self.input
            .seal()
            .map_err(|error| RuntimeError::new(error.code(), error.to_string()))?;
        Ok(self)
    }

    fn observe_registry_with_stdout_grant(
        source: &str,
        registry: &Registry,
        stdout_granted: bool,
    ) -> Result<Self, RuntimeError> {
        Self::observe_registry_with_stdout_grant_and_host_authorities(
            source,
            registry,
            stdout_granted,
            &[],
            &InstalledHostObservationInput::conduct_host(),
        )
    }

    fn observe_registry_with_stdout_grant_and_host_authorities(
        source: &str,
        registry: &Registry,
        stdout_granted: bool,
        observed_authorities: &[ObservedHostServiceAuthority],
        host_observation: &InstalledHostObservationInput,
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
        let providers = registry
            .installed_providers_for_panel(&panel)
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let mut implementations = BTreeMap::new();
        let mut candidates = Vec::with_capacity(required.len());
        for (contract_id, (contract_hash, instances)) in required {
            let matching = providers
                .iter()
                .filter(|provider| {
                    provider.contract.id.as_str() == contract_id
                        && provider.manifest.semantic_contract.semantic_hash == contract_hash
                })
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return Err(RuntimeError::new(
                    "CND-RUN-007",
                    format!("no installed provider implements `{contract_id}`"),
                ));
            }
            let stdout_instance = (contract_id == "io/stdout")
                .then(|| instances.first().cloned())
                .flatten();
            for installed in matching {
                let host_service_instances = if installed.implementation
                    == HostedPrimitiveImplementation::HostedService
                {
                    instances
                        .iter()
                        .map(|instance| {
                            let node = panel.nodes.iter().find(|node| {
                                node.id == *instance || instance.ends_with(&format!("/{}", node.id))
                            });
                            HostedServiceInstance {
                                instance: instance.clone(),
                                constraints: node
                                    .map_or_else(Vec::new, hosted_service_authority_constraints),
                                resource_id: node.and_then(|node| {
                                    secret_reference(node, "device_resource").map(ToOwned::to_owned)
                                }),
                                grant_id: node.and_then(|node| {
                                    secret_reference(node, "device_grant").map(ToOwned::to_owned)
                                }),
                                sharing: node
                                    .and_then(|node| node.config("sharing_mode"))
                                    .map(ToOwned::to_owned),
                                maximum_holders: node
                                    .and_then(|node| source_u64(node, "maximum_concurrent_streams"))
                                    .and_then(|value| u16::try_from(value).ok()),
                                lease_ticks: node.and_then(|node| source_u64(node, "lease_ticks")),
                                revocation_grace_ticks: node
                                    .and_then(|node| source_u64(node, "revocation_grace_ticks")),
                                cleanup_ticks: node
                                    .and_then(|node| source_u64(node, "cleanup_ticks")),
                            }
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
                    observed_authorities,
                    host_observation,
                ));
            }
        }
        share_host_observation(&mut candidates);
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
                && !catalog
                    .external_leaf_contracts
                    .iter()
                    .any(|entry| entry.id == contract.id.as_str())
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
            evidence_provider: None,
            watch_admissions: Vec::new(),
            source_semantic_hash: topology.source_semantic_hash.to_string(),
            resolver: pin("conduit/exact-compiler-resolver", 70),
            resolver_policy_hash: String::new(),
            time_basis: host_observation.time_basis.clone(),
            current_tick: host_observation.current_tick,
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
                    implementation_version: installed.manifest.implementation_version.to_owned(),
                    implementation_identity: node.implementation.semantic_hash,
                    artifact_id: installed.artifact.id.to_string(),
                    artifact_digest: installed.artifact.digest,
                    artifacts: installed
                        .artifacts
                        .iter()
                        .map(|artifact| conduit_runtime::ManagedArtifactIdentity {
                            id: artifact.id.to_string(),
                            digest: artifact.digest.to_string(),
                        })
                        .collect(),
                    implementation,
                    managed_lifecycle: installed.managed_lifecycle.cloned(),
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

/// A host observation describes the host snapshot, not one implementation
/// candidate. Every candidate resolved against that snapshot must therefore
/// carry the same report identity. Executor, target, and ABI support are the
/// canonical union of the relevant installed implementations.
fn share_host_observation(candidates: &mut [CandidateDocument]) {
    let Some(first) = candidates.first() else {
        return;
    };
    let mut report = first.host_report.clone();
    report.supported_executors = candidates
        .iter()
        .flat_map(|candidate| candidate.host_report.supported_executors.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    report.supported_targets = candidates
        .iter()
        .flat_map(|candidate| candidate.host_report.supported_targets.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    report.supported_abis = candidates
        .iter()
        .flat_map(|candidate| candidate.host_report.supported_abis.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    report.minimum_plan_version = candidates
        .iter()
        .map(|candidate| candidate.host_report.minimum_plan_version)
        .min()
        .unwrap_or(report.minimum_plan_version);
    report.maximum_plan_version = candidates
        .iter()
        .map(|candidate| candidate.host_report.maximum_plan_version)
        .max()
        .unwrap_or(report.maximum_plan_version);
    for candidate in candidates {
        candidate.host_report = report.clone();
    }
}

fn candidate(
    installed: &conduit_runtime::InstalledHostedProvider,
    stdout_instance: Option<&str>,
    host_service_instances: &[HostedServiceInstance],
    stdout_granted: bool,
    observed_authorities: &[ObservedHostServiceAuthority],
    host_observation: &InstalledHostObservationInput,
) -> CandidateDocument {
    let manifest = installed.manifest;
    let mut authorities = stdout_instance
        .map(|instance| {
            vec![authority_on_observed_host(
                stdout_authority(instance, stdout_granted),
                host_observation,
            )]
        })
        .unwrap_or_default();
    for instance in host_service_instances {
        if !matches!(
            installed.contract.id.as_str(),
            "learned/promote" | "conduit.media/audio/capture" | "conduit.media/audio/playback"
        ) {
            authorities.extend(
                host_service_authority(installed.contract.id.as_str(), instance)
                    .map(|decision| authority_on_observed_host(decision, host_observation)),
            );
        }
        authorities.extend(
            observed_authorities
                .iter()
                .filter(|observation| {
                    observation.contract_id == installed.contract.id.as_str()
                        && observation.instance == instance.instance
                })
                .map(|observation| observation.decision.clone()),
        );
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
            | HostedPrimitiveImplementation::ControlMerge
            | HostedPrimitiveImplementation::Zip
            | HostedPrimitiveImplementation::Gate
            | HostedPrimitiveImplementation::Select
    );
    let time_profile = matches!(
        installed.implementation,
        HostedPrimitiveImplementation::Ticker
            | HostedPrimitiveImplementation::TimeDelay
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
    let process_profile = installed.contract.id.as_str() == "conduit.host/process/exec"
        || manifest.executor == ExecutorKind::Process;
    let audio_device_profile = matches!(
        installed.contract.id.as_str(),
        "conduit.media/audio/capture" | "conduit.media/audio/playback"
    );
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
            implementation_version: manifest.implementation_version.to_owned(),
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
            artifacts: manifest
                .artifacts
                .iter()
                .map(|reference| ArtifactReferenceDocument {
                    id: reference.id.to_string(),
                    digest: reference.digest.to_string(),
                    role: reference.role.to_string(),
                    required: reference.required,
                })
                .collect(),
            required_interfaces: manifest
                .required_interfaces
                .iter()
                .map(|interface| ImplementationInterfaceDocument {
                    interface: PinDocument {
                        id: interface.interface.id.to_string(),
                        schema_version: interface.interface.schema_version,
                        semantic_hash: interface.interface.semantic_hash.to_string(),
                    },
                    entrypoint: interface.entrypoint.to_string(),
                })
                .collect(),
            provided_interfaces: manifest
                .provided_interfaces
                .iter()
                .map(|interface| ImplementationInterfaceDocument {
                    interface: PinDocument {
                        id: interface.interface.id.to_string(),
                        schema_version: interface.interface.schema_version,
                        semantic_hash: interface.interface.semantic_hash.to_string(),
                    },
                    entrypoint: interface.entrypoint.to_string(),
                })
                .collect(),
            required_authorities: manifest
                .required_authorities
                .iter()
                .map(ToString::to_string)
                .collect(),
            required_effects: manifest
                .required_effects
                .iter()
                .map(ToString::to_string)
                .collect(),
            minimum_plan_version: manifest.minimum_plan_version,
            maximum_plan_version: EXECUTION_PLAN_SCHEMA_VERSION,
            minimum_runtime_protocol: manifest.minimum_runtime_protocol,
            maximum_runtime_protocol: manifest.maximum_runtime_protocol,
            coexistence_memory_bytes: manifest.coexistence_memory_bytes,
        },
        execution_profile: if audio_device_profile {
            audio_device_execution_profile()
        } else if process_profile {
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
        artifacts: installed
            .artifacts
            .iter()
            .map(|artifact| ArtifactDocument {
                schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
                identity: String::new(),
                id: artifact.id.to_string(),
                digest: artifact.digest.to_string(),
                media_type: artifact.media_type.to_owned(),
                byte_size: artifact.byte_size,
                target: artifact.target.map(|value| value.to_string()),
                abi: artifact.abi.map(|value| value.to_string()),
                builder: artifact.provenance.builder.to_string(),
                source_digest: artifact.provenance.source_digest.to_string(),
                build_recipe_digest: artifact.provenance.build_recipe_digest.to_string(),
                reproducible: artifact.provenance.reproducible,
                license_expressions: artifact
                    .license_expressions
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            })
            .collect(),
        host_report: HostReportDocument {
            schema_version: conduit_core::CAPABILITY_REPORT_SCHEMA_VERSION,
            identity: String::new(),
            id: host_observation.id.clone(),
            host: host_observation.host.clone(),
            boot_id: host_observation.boot_id.clone(),
            reporter: host_observation.reporter.clone(),
            trust: host_observation.trust.clone(),
            membership: None,
            time_basis: host_observation.time_basis.clone(),
            observed_at_tick: host_observation.observed_at_tick,
            valid_until_tick: host_observation.valid_until_tick,
            available: host_observation.available,
            capabilities: Vec::new(),
            resources: Vec::new(),
            topology: Vec::new(),
            supported_executors: vec![executor_name(manifest.executor).to_owned()],
            supported_targets: installed
                .artifacts
                .iter()
                .filter_map(|artifact| artifact.target.map(|value| value.to_string()))
                .collect(),
            supported_abis: installed
                .artifacts
                .iter()
                .filter_map(|artifact| artifact.abi.map(|value| value.to_string()))
                .chain(std::iter::once(manifest.entrypoint.abi.to_string()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
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
    if let (Some(device), Some(observation), Some(backend)) = (
        secret_reference(node, "device_resource"),
        node.config("provider_observation"),
        node.config("backend_identity"),
    ) {
        return [
            ("conduit.constraint/audio-device", device),
            ("conduit.constraint/audio-observation", observation),
            ("conduit.constraint/audio-backend", backend),
        ]
        .into_iter()
        .map(|(id, value)| AuthorityConstraintDocument {
            id: id.to_owned(),
            semantic_hash: hosted_effect_constraint_hash(id, value.as_bytes()).to_string(),
        })
        .collect();
    }
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

fn secret_reference<'a>(node: &'a conduit_panel::Node, key: &str) -> Option<&'a str> {
    match node.config_value(key) {
        Some(SourceValue::SecretReference(value)) => Some(value),
        _ => None,
    }
}

fn source_u64(node: &conduit_panel::Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
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

fn authority_on_observed_host(
    mut decision: AuthorityDecisionDocument,
    observation: &InstalledHostObservationInput,
) -> AuthorityDecisionDocument {
    decision.capability.host.clone_from(&observation.host);
    decision
        .capability
        .time_basis
        .clone_from(&observation.time_basis);
    decision.capability.observed_at_tick = observation.observed_at_tick;
    decision.grant.issued_for_host.clone_from(&observation.host);
    decision
        .grant
        .time_basis
        .clone_from(&observation.time_basis);
    decision.grant.not_before_tick = observation.observed_at_tick;
    decision
        .resource_lease
        .time_basis
        .clone_from(&observation.time_basis);
    decision.resource_lease.issued_at_tick = observation.observed_at_tick;
    decision
}

/// Builds one deterministic authority fixture for conformance tests. This is
/// not a production policy observer and must never be called by default host
/// execution paths.
#[must_use]
pub fn fixture_host_service_authority_observation(
    contract_id: &str,
    instance: &str,
    run_id: &str,
    epoch: u64,
    constraints: &[AuthorityConstraintDocument],
) -> Option<ObservedHostServiceAuthority> {
    observed_host_service_authority(HostServiceAuthorityObservationInput {
        contract_id: contract_id.to_owned(),
        instance: instance.to_owned(),
        run_id: run_id.to_owned(),
        epoch,
        constraints: constraints.to_vec(),
        resource_id: None,
        grant_id: None,
        sharing: None,
        maximum_holders: None,
        lease_ticks: None,
        revocation_grace_ticks: None,
        cleanup_ticks: None,
    })
}

fn host_service_authority(
    contract_id: &str,
    instance: &HostedServiceInstance,
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
        "net/http/listen" => (
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
        "learned/promote" => (
            "learned-promotion",
            "sha256:5454545454545454545454545454545454545454545454545454545454545454",
            "conduit.action/promote",
            "conduit.resource/learned-model-slot",
            "conduit.resource/learned-reference-slot",
        ),
        "conduit.media/audio/capture" => (
            "audio-capture",
            "sha256:6565656565656565656565656565656565656565656565656565656565656565",
            "conduit.action/capture-audio",
            "conduit.resource/audio-input-device",
            "conduit.audio/device/unspecified-capture",
        ),
        "conduit.media/audio/playback" => (
            "audio-playback",
            "sha256:6666666666666666666666666666666666666666666666666666666666666666",
            "conduit.action/play-audio",
            "conduit.resource/audio-output-device",
            "conduit.audio/device/unspecified-playback",
        ),
        _ => return None,
    };
    let resource_id = instance.resource_id.as_deref().unwrap_or(resource_id);
    let (mut resource_lease, commit_profile) =
        effect_contracts(name, &instance.instance, action, resource_id);
    if let Some(sharing) = &instance.sharing {
        resource_lease.sharing.clone_from(sharing);
    }
    if let Some(maximum_holders) = instance.maximum_holders {
        resource_lease.maximum_holders = maximum_holders;
    }
    if let Some(ticks) = instance.lease_ticks {
        resource_lease.expires_at_tick = resource_lease.issued_at_tick.saturating_add(ticks);
    }
    if let Some(ticks) = instance.revocation_grace_ticks {
        resource_lease.revocation_grace_ticks = ticks;
    }
    if let Some(ticks) = instance.cleanup_ticks {
        resource_lease.cleanup_ticks = ticks;
    }
    let valid_until_tick = if contract_id == "learned/promote" {
        200
    } else {
        resource_lease.expires_at_tick
    };
    resource_lease.expires_at_tick = valid_until_tick;
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
            requester: instance.instance.clone(),
            audience: "conduit/conduct-run".to_owned(),
            constraints: instance.constraints.clone(),
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
            valid_until_tick,
        },
        grant: AuthorityGrantDocument {
            id: instance
                .grant_id
                .clone()
                .unwrap_or_else(|| format!("conduit.grant/{name}")),
            action: action.to_owned(),
            resource_kind: resource_kind.to_owned(),
            resource_id: resource_id.to_owned(),
            scope_root: instance.instance.clone(),
            scope_descendants: false,
            audience: "conduit/conduct-run".to_owned(),
            constraints: instance.constraints.clone(),
            time_basis: "clock/conduct-host".to_owned(),
            not_before_tick: 10,
            expires_at_tick: valid_until_tick,
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

fn audio_device_execution_profile() -> ExecutionProfileDocument {
    const HOST_BUFFER_BYTES: u64 = 4 * 1024;
    const MEMORY_BYTES: u64 = 32 * 1024;
    ExecutionProfileDocument {
        id: "conduit/hosted-audio-device-profile".to_owned(),
        schema_version: 0,
        semantic_hash: String::new(),
        boundedness: "observed".to_owned(),
        cancellation: "bounded".to_owned(),
        step_bound_enforced: false,
        limits: ExecutionLimitsDocument {
            max_step_work: 256,
            max_input_leases: 1,
            max_input_bytes: 256,
            max_output_reservations: 1,
            max_output_bytes: 256,
            max_transactions: 1,
            max_fragments_per_step: 1,
            max_pending_operations: 1,
            max_timers: 1,
            max_child_tasks: 1,
            max_host_buffer_bytes: HOST_BUFFER_BYTES,
            max_foreign_queue_items: 2,
            max_foreign_queue_bytes: HOST_BUFFER_BYTES,
            implementation_memory_bytes: MEMORY_BYTES,
            cancellation_ticks: 2,
            ..ExecutionLimitsDocument::default()
        },
        representations: Vec::new(),
        memory_claims: vec![
            MemoryClaimDocument {
                category: "host-services".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: HOST_BUFFER_BYTES,
            },
            MemoryClaimDocument {
                category: "foreign-runtime".to_owned(),
                accounting: "observed-only".to_owned(),
                bytes: HOST_BUFFER_BYTES,
            },
            MemoryClaimDocument {
                category: "pending-operations".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: HOST_BUFFER_BYTES,
            },
            MemoryClaimDocument {
                category: "port-transactions".to_owned(),
                accounting: "executor-allocated".to_owned(),
                bytes: MEMORY_BYTES - 3 * HOST_BUFFER_BYTES,
            },
        ],
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
