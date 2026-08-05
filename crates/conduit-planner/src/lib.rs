use conduit_core::{
    mandatory_evidence_storage_requirement, seal_plan, AuthorityBinding, AuthorityGrant,
    CancellationPolicy, CapabilityId, ConnectionId, ConnectionProvider, ExpectedEvidence,
    ExpectedTerminal, FragmentId, HostAdvertisement, HostId, LinkAvailability, LinkBinding,
    OperationId, PlacementId, Plan, PlanFragment, PlanId, PlannedConnection, PlannedOperation,
    ResourceBinding, ResourcePoolId, StartupDependency, TerminalPolicy,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedForm, CheckedOperation};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoice {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoices {
    pub by_operation: BTreeMap<OperationId, PlacementChoice>,
}

#[derive(Debug, Clone, Copy)]
pub struct PlanningOptions<'a> {
    pub connection_providers: &'a BTreeMap<(OperationId, OperationId), ConnectionProvider>,
    pub connection_item_capacity: u16,
    pub connection_byte_capacity: u32,
    pub authority_grants: &'a [AuthorityGrant],
    pub link_bindings: &'a [LinkBinding],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    UnknownOperation(String),
    MissingPlacement(String),
    DuplicatePlacement(String),
    UnknownHost(String),
    UnknownCapability(String),
    WrongSemanticKind(String),
    WrongKindContractRevision(String),
    IncompatiblePortContract(String),
    InvalidHostOperationRequirement(String),
    InvalidResourceContract(String),
    UnavailableResource(String),
    ResourceCapacityExceeded(String),
    InvalidAuthorityContract(String),
    AuthorityGrantMissing(String),
    AuthorityGrantAmbiguous(String),
    InvalidLinkBinding(String),
    LinkBindingMissing(String),
    LinkBindingUnavailable(String),
    LinkBindingAmbiguous(String),
    UnavailableConnectionProvider(String),
    QueueRequirementAboveHostLimit(String),
    CapabilityInstanceLimitExceeded(String),
    CyclicStartupDependencies(String),
    EvidenceBudgetOverflow(String),
    InvalidPlacementSyntax(String),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlannerError::UnknownOperation(value) => write!(f, "unknown operation '{value}'"),
            PlannerError::MissingPlacement(value) => write!(f, "missing placement for '{value}'"),
            PlannerError::DuplicatePlacement(value) => {
                write!(f, "duplicate placement for '{value}'")
            }
            PlannerError::UnknownHost(value) => write!(f, "unknown host '{value}'"),
            PlannerError::UnknownCapability(value) => write!(f, "unknown capability '{value}'"),
            PlannerError::WrongSemanticKind(value) => write!(f, "wrong semantic kind: {value}"),
            PlannerError::WrongKindContractRevision(value) => {
                write!(f, "wrong kind contract revision: {value}")
            }
            PlannerError::IncompatiblePortContract(value) => {
                write!(f, "incompatible port contract: {value}")
            }
            PlannerError::InvalidHostOperationRequirement(value) => {
                write!(f, "invalid host-operation requirement: {value}")
            }
            PlannerError::InvalidResourceContract(value) => {
                write!(f, "invalid resource contract: {value}")
            }
            PlannerError::UnavailableResource(value) => {
                write!(f, "unavailable resource: {value}")
            }
            PlannerError::ResourceCapacityExceeded(value) => {
                write!(f, "resource capacity exceeded: {value}")
            }
            PlannerError::InvalidAuthorityContract(value) => {
                write!(f, "invalid authority contract: {value}")
            }
            PlannerError::AuthorityGrantMissing(value) => {
                write!(f, "authority grant missing: {value}")
            }
            PlannerError::AuthorityGrantAmbiguous(value) => {
                write!(f, "authority grant ambiguous: {value}")
            }
            PlannerError::InvalidLinkBinding(value) => {
                write!(f, "invalid link binding: {value}")
            }
            PlannerError::LinkBindingMissing(value) => {
                write!(f, "link binding missing: {value}")
            }
            PlannerError::LinkBindingUnavailable(value) => {
                write!(f, "link binding unavailable: {value}")
            }
            PlannerError::LinkBindingAmbiguous(value) => {
                write!(f, "link binding ambiguous: {value}")
            }
            PlannerError::UnavailableConnectionProvider(value) => {
                write!(f, "unavailable connection provider: {value}")
            }
            PlannerError::QueueRequirementAboveHostLimit(value) => {
                write!(f, "queue requirement above host limit: {value}")
            }
            PlannerError::CapabilityInstanceLimitExceeded(value) => {
                write!(f, "capability instance limit exceeded: {value}")
            }
            PlannerError::CyclicStartupDependencies(value) => {
                write!(f, "cyclic startup dependencies: {value}")
            }
            PlannerError::EvidenceBudgetOverflow(value) => {
                write!(f, "mandatory evidence budget overflow: {value}")
            }
            PlannerError::InvalidPlacementSyntax(value) => {
                write!(f, "invalid placement syntax: {value}")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

pub fn parse_placements(source: &str) -> Result<PlacementChoices, PlannerError> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    if lines.first().copied().unwrap_or("") != "placements 0" {
        return Err(PlannerError::InvalidPlacementSyntax(
            "expected first non-comment line to be 'placements 0'".to_string(),
        ));
    }

    let mut index = 1usize;
    let mut by_operation = BTreeMap::new();
    while index < lines.len() {
        let header = lines[index];
        let operation_name = header
            .strip_suffix(':')
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(header.to_string()))?;
        if by_operation.contains_key(&OperationId::from(operation_name)) {
            return Err(PlannerError::DuplicatePlacement(operation_name.to_string()));
        }
        let host_line = lines
            .get(index + 1)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let capability_line = lines
            .get(index + 2)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let host_id = parse_assignment(host_line, "host")?;
        let capability_id = parse_assignment(capability_line, "capability")?;
        by_operation.insert(
            OperationId::from(operation_name),
            PlacementChoice {
                host_id: HostId::from(host_id),
                capability_id: CapabilityId::from(capability_id),
            },
        );
        index += 3;
    }

    Ok(PlacementChoices { by_operation })
}

fn parse_assignment<'a>(line: &'a str, key: &str) -> Result<&'a str, PlannerError> {
    let (lhs, rhs) = line
        .split_once('=')
        .ok_or_else(|| PlannerError::InvalidPlacementSyntax(line.to_string()))?;
    if lhs.trim() != key {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    let value = rhs.trim().trim_matches('"');
    if value.is_empty() {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    Ok(value)
}

pub fn default_placements(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    let host = realm
        .first()
        .ok_or_else(|| PlannerError::UnknownHost("realm is empty".to_string()))?;
    let mut by_operation = BTreeMap::new();
    for operation in &form.operations {
        let offer = host
            .capabilities
            .iter()
            .find(|offer| {
                offer.kind_id == operation.kind_id
                    && offer.kind_contract_revision == operation.kind_contract_revision
                    && offer.inputs == operation.inputs
                    && offer.outputs == operation.outputs
            })
            .ok_or_else(|| {
                PlannerError::UnknownCapability(operation.kind_id.as_str().to_string())
            })?;
        by_operation.insert(
            operation.operation_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: offer.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_operation })
}

pub fn plan(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits(
        form,
        realm,
        placements,
        providers,
        DEFAULT_CONNECTION_ITEM_CAPACITY,
        DEFAULT_CONNECTION_BYTE_CAPACITY,
    )
}

pub fn plan_with_authority_grants(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    authority_grants: &[AuthorityGrant],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants,
            link_bindings: &[],
        },
    )
}

pub fn plan_with_link_bindings(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
    link_bindings: &[LinkBinding],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            link_bindings,
        },
    )
}

pub fn plan_with_connection_limits(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits_and_provider_overrides(
        form,
        realm,
        placements,
        providers,
        &BTreeMap::new(),
        connection_item_capacity,
        connection_byte_capacity,
    )
}

pub fn plan_with_connection_limits_and_provider_overrides(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_providers: &BTreeMap<(OperationId, OperationId), ConnectionProvider>,
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers,
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            link_bindings: &[],
        },
    )
}

pub fn plan_with_options(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    let PlanningOptions {
        connection_providers,
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
        link_bindings,
    } = options;
    let realm_index = realm
        .iter()
        .map(|host| (host.host_id.clone(), host))
        .collect::<BTreeMap<_, _>>();

    for host in realm {
        validate_host_resources(host)?;
    }
    validate_authority_grants(authority_grants)?;
    validate_link_bindings(link_bindings)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut planned_operations = Vec::<PlannedOperation>::new();
    let mut placement_lookup = BTreeMap::<OperationId, PlacementId>::new();

    for operation in &form.operations {
        let choice = placements
            .by_operation
            .get(&operation.operation_id)
            .ok_or_else(|| {
                PlannerError::MissingPlacement(operation.operation_id.as_str().to_string())
            })?;
        let host = realm_index
            .get(&choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
            })?;
        validate_operation_capability(operation, capability)?;

        let count = placement_count
            .entry((host.host_id.clone(), capability.capability_id.clone()))
            .or_insert(0);
        *count += 1;
        if *count > capability.limits.max_active_instances {
            return Err(PlannerError::CapabilityInstanceLimitExceeded(format!(
                "capability '{}' exceeds max {}",
                capability.capability_id.as_str(),
                capability.limits.max_active_instances
            )));
        }

        let mut resource_bindings = Vec::with_capacity(capability.resource_requirements.len());
        for requirement in &capability.resource_requirements {
            let mut matches = host
                .resources
                .iter()
                .filter(|resource| resource.class_id == requirement.class_id);
            let Some(resource) = matches.next() else {
                return Err(PlannerError::UnavailableResource(format!(
                    "host '{}' has no pool for class '{}'",
                    host.host_id.as_str(),
                    requirement.class_id.as_str()
                )));
            };
            if matches.next().is_some() {
                return Err(PlannerError::InvalidResourceContract(format!(
                    "host '{}' has multiple pools for class '{}' in the first planning profile",
                    host.host_id.as_str(),
                    requirement.class_id.as_str()
                )));
            }
            let used = resource_usage
                .entry((host.host_id.clone(), resource.pool_id.clone()))
                .or_insert(0);
            *used = used.checked_add(requirement.units).ok_or_else(|| {
                PlannerError::ResourceCapacityExceeded(resource.pool_id.as_str().to_string())
            })?;
            if *used > resource.capacity_units {
                return Err(PlannerError::ResourceCapacityExceeded(format!(
                    "pool '{}' requires {} units above capacity {}",
                    resource.pool_id.as_str(),
                    *used,
                    resource.capacity_units
                )));
            }
            resource_bindings.push(ResourceBinding {
                pool_id: resource.pool_id.clone(),
                class_id: resource.class_id.clone(),
                units: requirement.units,
            });
        }
        resource_bindings.sort();

        let mut authority_bindings = Vec::with_capacity(capability.authority_requirements.len());
        for requirement in &capability.authority_requirements {
            let mut matches = authority_grants.iter().filter(|grant| {
                grant.contract_id == requirement.contract_id
                    && grant.host_operation_contract_id == requirement.host_operation_contract_id
                    && grant.subject_kind == requirement.subject_kind
                    && grant.host_id == host.host_id
                    && grant.boot_id == host.boot_id
                    && grant.capability_id == capability.capability_id
            });
            let Some(grant) = matches.next() else {
                return Err(PlannerError::AuthorityGrantMissing(format!(
                    "capability '{}' requires '{}' for subject '{}' on host '{}' boot '{}'",
                    capability.capability_id.as_str(),
                    requirement.contract_id.as_str(),
                    requirement.subject_kind.as_str(),
                    host.host_id.as_str(),
                    host.boot_id.as_str()
                )));
            };
            if matches.next().is_some() {
                return Err(PlannerError::AuthorityGrantAmbiguous(format!(
                    "multiple grants satisfy capability '{}' requirement '{}'",
                    capability.capability_id.as_str(),
                    requirement.contract_id.as_str()
                )));
            }
            authority_bindings.push(AuthorityBinding {
                grant_id: grant.grant_id.clone(),
                contract_id: grant.contract_id.clone(),
                host_operation_contract_id: grant.host_operation_contract_id.clone(),
                subject_kind: grant.subject_kind.clone(),
                host_id: grant.host_id.clone(),
                boot_id: grant.boot_id.clone(),
                capability_id: grant.capability_id.clone(),
            });
        }
        authority_bindings.sort();

        let placement_id = PlacementId::from(hash_string(&format!(
            "placement:{}:{}:{}:{}",
            form.checked_form_id.as_str(),
            operation.operation_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(operation.operation_id.clone(), placement_id.clone());
        planned_operations.push(PlannedOperation {
            placement_id,
            operation_id: operation.operation_id.clone(),
            kind_id: operation.kind_id.clone(),
            kind_contract_revision: operation.kind_contract_revision.clone(),
            execution_profile_id: capability.execution_profile_id.clone(),
            configuration: operation.configuration.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation_id.clone(),
            artifact_id: capability.artifact_id.clone(),
            inputs: operation.inputs.clone(),
            outputs: operation.outputs.clone(),
            host_operations: capability.host_operations.clone(),
            resources: resource_bindings,
            authority: authority_bindings,
        });
    }

    for operation in placements.by_operation.keys() {
        if !form
            .operations
            .iter()
            .any(|item| &item.operation_id == operation)
        {
            return Err(PlannerError::UnknownOperation(
                operation.as_str().to_string(),
            ));
        }
    }

    let mut planned_connections = Vec::<PlannedConnection>::new();
    for connection in &form.connections {
        let source_placement = placement_lookup
            .get(&connection.source_operation_id)
            .ok_or_else(|| {
                PlannerError::UnknownOperation(connection.source_operation_id.as_str().to_string())
            })?;
        let sink_placement = placement_lookup
            .get(&connection.sink_operation_id)
            .ok_or_else(|| {
                PlannerError::UnknownOperation(connection.sink_operation_id.as_str().to_string())
            })?;
        let source_plan = planned_operations
            .iter()
            .find(|item| &item.placement_id == source_placement)
            .expect("source placement must exist");
        let sink_plan = planned_operations
            .iter()
            .find(|item| &item.placement_id == sink_placement)
            .expect("sink placement must exist");
        let (provider, link_binding) = select_provider(
            source_plan,
            sink_plan,
            providers,
            connection_providers
                .get(&(
                    connection.source_operation_id.clone(),
                    connection.sink_operation_id.clone(),
                ))
                .copied(),
            link_bindings,
            connection_item_capacity,
            connection_byte_capacity,
        )?;
        let source_capability =
            find_capability(realm, &source_plan.host_id, &source_plan.capability_id)?;
        let sink_capability = find_capability(realm, &sink_plan.host_id, &sink_plan.capability_id)?;
        if connection_item_capacity > source_capability.limits.max_queue_items
            || connection_item_capacity > sink_capability.limits.max_queue_items
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires item capacity {}",
                source_plan.operation_id.as_str(),
                sink_plan.operation_id.as_str(),
                connection_item_capacity
            )));
        }
        if connection_byte_capacity > source_capability.limits.max_queue_bytes
            || connection_byte_capacity > sink_capability.limits.max_queue_bytes
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires byte capacity {}",
                source_plan.operation_id.as_str(),
                sink_plan.operation_id.as_str(),
                connection_byte_capacity
            )));
        }
        planned_connections.push(PlannedConnection {
            connection_id: ConnectionId::from(hash_string(&format!(
                "connection:{}:{}:{}:{}:{}:{}",
                form.checked_form_id.as_str(),
                connection.source_operation_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_operation_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str()
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            provider,
            link_binding,
            item_capacity: connection_item_capacity,
            byte_capacity: connection_byte_capacity,
        });
    }

    let global_startup_order = startup_order(&planned_operations, &planned_connections)
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(form.name.clone()))?;

    let fragments = realm
        .iter()
        .map(|host| -> Result<Option<PlanFragment>, PlannerError> {
            let placements = planned_operations
                .iter()
                .filter(|item| item.host_id == host.host_id)
                .cloned()
                .collect::<Vec<_>>();
            if placements.is_empty() {
                return Ok(None);
            }
            let connections = planned_connections
                .iter()
                .filter(|connection| {
                    placements
                        .iter()
                        .any(|item| item.placement_id == connection.source_placement_id)
                        || placements
                            .iter()
                            .any(|item| item.placement_id == connection.sink_placement_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            let startup_order = global_startup_order
                .iter()
                .filter(|placement_id| {
                    placements
                        .iter()
                        .any(|placement| &placement.placement_id == *placement_id)
                })
                .cloned()
                .collect();
            let startup_dependencies = connections
                .iter()
                .map(|connection| StartupDependency {
                    prerequisite_placement_id: connection.sink_placement_id.clone(),
                    dependent_placement_id: connection.source_placement_id.clone(),
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let expected_terminals = placements
                .iter()
                .map(|placement| {
                    ExpectedTerminal::PlacementCompleted(placement.placement_id.clone())
                })
                .chain(connections.iter().map(|connection| {
                    ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
                .collect();
            let expected_evidence = core::iter::once(ExpectedEvidence::PlanFragmentReceived)
                .chain(placements.iter().map(|placement| {
                    ExpectedEvidence::PlacementPrepared(placement.placement_id.clone())
                }))
                .chain(placements.iter().map(|placement| {
                    ExpectedEvidence::PlacementTerminal(placement.placement_id.clone())
                }))
                .chain(connections.iter().map(|connection| {
                    ExpectedEvidence::ConnectionTerminal(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedEvidence::PlanTerminal))
                .collect::<Vec<_>>();
            let evidence_storage_budget =
                mandatory_evidence_storage_requirement(&expected_evidence).ok_or_else(|| {
                    PlannerError::EvidenceBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            Ok(Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                connections,
                startup_dependencies,
                startup_order,
                cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals,
                expected_evidence,
                evidence_storage_budget,
                plan_fragments: Vec::new(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(seal_plan(form.identity(), fragments))
}

fn startup_order(
    placements: &[PlannedOperation],
    connections: &[PlannedConnection],
) -> Option<Vec<PlacementId>> {
    let mut remaining = placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .find(|candidate| {
                connections.iter().all(|connection| {
                    &connection.source_placement_id != *candidate
                        || !remaining.contains(&connection.sink_placement_id)
                })
            })
            .cloned()?;
        remaining.remove(&next);
        ordered.push(next);
    }
    Some(ordered)
}

fn validate_operation_capability(
    operation: &CheckedOperation,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    if capability.kind_id != operation.kind_id {
        return Err(PlannerError::WrongSemanticKind(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_id.as_str(),
            capability.capability_id.as_str(),
            capability.kind_id.as_str()
        )));
    }
    if capability.kind_contract_revision != operation.kind_contract_revision {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_contract_revision.as_str(),
            capability.capability_id.as_str(),
            capability.kind_contract_revision.as_str()
        )));
    }
    if capability.inputs != operation.inputs || capability.outputs != operation.outputs {
        return Err(PlannerError::IncompatiblePortContract(format!(
            "operation '{}' ports differ from capability '{}'",
            operation.operation_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_operations.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_operations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostOperationRequirement(format!(
            "capability '{}' requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds",
            capability.capability_id.as_str()
        )));
    }
    if capability
        .resource_requirements
        .iter()
        .any(|requirement| requirement.class_id.as_str().is_empty() || requirement.units == 0)
        || capability
            .resource_requirements
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' requirements must have non-empty classes, positive units, and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    if capability.authority_requirements.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement.host_operation_contract_id.as_str().is_empty()
            || requirement.subject_kind.as_str().is_empty()
            || !capability.host_operations.iter().any(|host_operation| {
                host_operation.contract_id == requirement.host_operation_contract_id
                    && host_operation.target_kind.as_ref() == Some(&requirement.subject_kind)
            })
    }) || capability
        .authority_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidAuthorityContract(format!(
            "capability '{}' authority requirements must bind a declared targeted host operation with non-empty identities and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    Ok(())
}

fn validate_authority_grants(grants: &[AuthorityGrant]) -> Result<(), PlannerError> {
    if grants.iter().any(|grant| {
        grant.grant_id.as_str().is_empty()
            || grant.contract_id.as_str().is_empty()
            || grant.host_operation_contract_id.as_str().is_empty()
            || grant.subject_kind.as_str().is_empty()
            || grant.host_id.as_str().is_empty()
            || grant.boot_id.as_str().is_empty()
            || grant.capability_id.as_str().is_empty()
    }) {
        return Err(PlannerError::InvalidAuthorityContract(
            "grants must have non-empty immutable scope identities".to_string(),
        ));
    }
    let unique_ids = grants
        .iter()
        .map(|grant| &grant.grant_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != grants.len() {
        return Err(PlannerError::InvalidAuthorityContract(
            "grant identities must be unique".to_string(),
        ));
    }
    Ok(())
}

fn validate_host_resources(host: &HostAdvertisement) -> Result<(), PlannerError> {
    if host.resources.iter().any(|resource| {
        resource.pool_id.as_str().is_empty()
            || resource.class_id.as_str().is_empty()
            || resource.capacity_units == 0
    }) || host
        .resources
        .windows(2)
        .any(|pair| pair[0].pool_id >= pair[1].pool_id)
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "host '{}' pools must have non-empty identities, positive capacity, and unique pool-id ordering",
            host.host_id.as_str()
        )));
    }
    Ok(())
}

fn find_capability<'a>(
    realm: &'a [HostAdvertisement],
    host_id: &HostId,
    capability_id: &CapabilityId,
) -> Result<&'a conduit_core::CapabilityOffer, PlannerError> {
    realm
        .iter()
        .find(|host| &host.host_id == host_id)
        .and_then(|host| {
            host.capabilities
                .iter()
                .find(|item| &item.capability_id == capability_id)
        })
        .ok_or_else(|| PlannerError::UnknownCapability(capability_id.as_str().to_string()))
}

fn select_provider(
    source: &PlannedOperation,
    sink: &PlannedOperation,
    providers: &[ConnectionProvider],
    requested: Option<ConnectionProvider>,
    link_bindings: &[LinkBinding],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<(ConnectionProvider, Option<LinkBinding>), PlannerError> {
    if source.host_id == sink.host_id {
        if requested.is_some_and(|provider| provider != ConnectionProvider::Local)
            || !providers.contains(&ConnectionProvider::Local)
        {
            return Err(PlannerError::UnavailableConnectionProvider(format!(
                "local provider unavailable for '{}' -> '{}'",
                source.operation_id.as_str(),
                sink.operation_id.as_str()
            )));
        }
        return Ok((ConnectionProvider::Local, None));
    }

    if requested == Some(ConnectionProvider::Local) {
        return Err(PlannerError::UnavailableConnectionProvider(format!(
            "local provider cannot connect '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let endpoint_matches = |binding: &&LinkBinding| {
        binding.source.host_id == source.host_id
            && binding.source.boot_id == source.boot_id
            && binding.sink.host_id == sink.host_id
            && binding.sink.boot_id == sink.boot_id
            && requested.is_none_or(|provider| binding.provider == provider)
    };
    let exact = link_bindings
        .iter()
        .filter(endpoint_matches)
        .collect::<Vec<_>>();
    if exact.is_empty() {
        return Err(PlannerError::LinkBindingMissing(format!(
            "no observed boot-scoped link for '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let ready = exact
        .into_iter()
        .filter(|binding| {
            binding.availability == LinkAvailability::Ready
                && binding.limits.maximum_in_flight_items >= connection_item_capacity
                && binding.limits.maximum_buffered_bytes >= connection_byte_capacity
        })
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Err(PlannerError::LinkBindingUnavailable(format!(
            "observed link for '{}' -> '{}' is unavailable or below item/byte limits",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    if ready.len() != 1 {
        return Err(PlannerError::LinkBindingAmbiguous(format!(
            "multiple observed links satisfy '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let binding = ready[0].clone();
    Ok((binding.provider, Some(binding)))
}

fn validate_link_bindings(bindings: &[LinkBinding]) -> Result<(), PlannerError> {
    if bindings.iter().any(|binding| {
        binding.binding_id.as_str().is_empty()
            || binding.source.host_id.as_str().is_empty()
            || binding.source.boot_id.as_str().is_empty()
            || binding.source.endpoint_id.as_str().is_empty()
            || binding.sink.host_id.as_str().is_empty()
            || binding.sink.boot_id.as_str().is_empty()
            || binding.sink.endpoint_id.as_str().is_empty()
            || binding.source.endpoint_id == binding.sink.endpoint_id
            || binding.source.host_id == binding.sink.host_id
            || binding.provider == ConnectionProvider::Local
            || binding.provider_instance_id.as_str().is_empty()
            || binding.limits.maximum_in_flight_items == 0
            || binding.limits.maximum_buffered_bytes == 0
            || matches!(
                &binding.credential,
                conduit_core::LinkCredentialReference::Opaque(reference)
                    if reference.as_str().is_empty()
            )
            || matches!(
                &binding.authority,
                conduit_core::LinkAuthorityReference::Grant(grant_id)
                    if grant_id.as_str().is_empty()
            )
    }) {
        return Err(PlannerError::InvalidLinkBinding(
            "remote link bindings require non-empty distinct boot-scoped endpoints, one initialized non-local provider, and positive limits".to_string(),
        ));
    }
    let unique_ids = bindings
        .iter()
        .map(|binding| &binding.binding_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != bindings.len() {
        return Err(PlannerError::InvalidLinkBinding(
            "link binding identities must be unique".to_string(),
        ));
    }
    Ok(())
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_placements, parse_placements, plan, plan_with_authority_grants,
        plan_with_link_bindings, startup_order, PlacementChoice, PlacementChoices, PlannerError,
    };
    use conduit_core::{
        authority_grant, kind_id, mandatory_evidence_storage_requirement,
        present_authority_requirement, process_owned_link_binding, verify_plan,
        verify_plan_fragment, ArtifactId, CancellationPolicy, CapabilityLimits, CapabilityOffer,
        ConnectionProvider, ExpandedFormId, HostAdvertisement, HostId, HostProfileId,
        ImplementationId, OfferGeneration, SourceDocumentId, StartupDependency, TerminalPolicy,
        PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_signal::{
        pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
        pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
        show_host_operation_requirements, show_inputs, show_resource_requirements,
        signal_profile_catalog, signal_resource_offers, PULSE_KIND, SHOW_KIND,
        SIGNAL_PRESENTATION_KIND,
    };
    use std::collections::BTreeMap;

    fn form() -> conduit_form::CheckedForm {
        parse(
            "form 0\n\nsignal-demo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("form must parse")
    }

    fn host() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("std-host-1"),
            boot_id: conduit_core::BootId::from("boot-1"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("rust-std"),
            resources: signal_resource_offers("test/timer", "test/presentation", 4),
            capabilities: vec![
                CapabilityOffer {
                    capability_id: conduit_core::CapabilityId::from("pulse-1"),
                    kind_id: kind_id(PULSE_KIND),
                    kind_contract_revision: pulse_contract_revision(),
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                    inputs: vec![],
                    outputs: pulse_outputs(),
                    host_operations: pulse_host_operation_requirements(),
                    resource_requirements: pulse_resource_requirements(),
                    authority_requirements: vec![],
                    limits: CapabilityLimits {
                        max_active_instances: 4,
                        max_queue_items: 4,
                        max_queue_bytes: 64,
                    },
                },
                CapabilityOffer {
                    capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    kind_id: kind_id(SHOW_KIND),
                    kind_contract_revision: show_contract_revision(),
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    artifact_id: ArtifactId::from("test/show-artifact-v1"),
                    inputs: show_inputs(),
                    outputs: vec![],
                    host_operations: show_host_operation_requirements(),
                    resource_requirements: show_resource_requirements(),
                    authority_requirements: vec![],
                    limits: CapabilityLimits {
                        max_active_instances: 4,
                        max_queue_items: 4,
                        max_queue_bytes: 64,
                    },
                },
            ],
        }
    }

    #[test]
    fn parses_block_placement_file() {
        let placements = parse_placements(
            "placements 0\npulse:\n    host = \"std-host-1\"\n    capability = \"pulse-1\"\nshow:\n    host = \"std-host-1\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
        assert_eq!(placements.by_operation.len(), 2);
    }

    #[test]
    fn default_placement_uses_realm() {
        let placements = default_placements(&form(), &[host()]).expect("placements must work");
        assert_eq!(placements.by_operation.len(), 2);
    }

    #[test]
    fn planning_binds_exact_contract_profile_and_every_port() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let plan = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("exact plan resolves");
        assert_eq!(plan.source_document_id, form.source_document_id);
        assert_eq!(plan.checked_form_id, form.checked_form_id);
        assert_eq!(plan.expanded_form_id, form.expanded_form_id);
        assert!(plan.fragments.iter().all(|fragment| {
            fragment.source_document_id == form.source_document_id
                && fragment.checked_form_id == form.checked_form_id
                && fragment.expanded_form_id == form.expanded_form_id
        }));
        for placement in &plan.fragments[0].placements {
            let operation = form
                .operations
                .iter()
                .find(|operation| operation.operation_id == placement.operation_id)
                .expect("checked operation exists");
            let capability = host
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == placement.capability_id)
                .expect("capability exists");
            assert_eq!(
                placement.kind_contract_revision,
                operation.kind_contract_revision
            );
            assert_eq!(
                placement.kind_contract_revision,
                capability.kind_contract_revision
            );
            assert_eq!(
                placement.execution_profile_id,
                capability.execution_profile_id
            );
            assert_eq!(placement.inputs, operation.inputs);
            assert_eq!(placement.outputs, operation.outputs);
            assert_eq!(placement.host_operations, capability.host_operations);
            assert_eq!(
                placement.resources.len(),
                capability.resource_requirements.len()
            );
            for binding in &placement.resources {
                assert!(capability.resource_requirements.iter().any(|requirement| {
                    requirement.class_id == binding.class_id && requirement.units == binding.units
                }));
                assert!(host.resources.iter().any(|resource| {
                    resource.pool_id == binding.pool_id && resource.class_id == binding.class_id
                }));
            }
        }
        assert!(plan.fragments[0]
            .placements
            .iter()
            .all(|placement| !placement.host_operations.is_empty()));
        let fragment = &plan.fragments[0];
        assert_eq!(
            fragment.startup_dependencies,
            vec![StartupDependency {
                prerequisite_placement_id: fragment.connections[0].sink_placement_id.clone(),
                dependent_placement_id: fragment.connections[0].source_placement_id.clone(),
            }]
        );
        assert_eq!(
            fragment.startup_order,
            vec![
                fragment.connections[0].sink_placement_id.clone(),
                fragment.connections[0].source_placement_id.clone(),
            ]
        );
        assert_eq!(
            fragment.cancellation_policy,
            CancellationPolicy::CancelAllAndRejectLateCompletion
        );
        assert_eq!(
            fragment.terminal_policy,
            TerminalPolicy::RequireAllPlacementsAndConnections
        );
        assert_eq!(
            fragment.evidence_storage_budget,
            mandatory_evidence_storage_requirement(&fragment.expected_evidence)
                .expect("focused evidence fits public budget types")
        );
    }

    #[test]
    fn planning_rejects_cyclic_startup_dependencies() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let plan = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("acyclic plan resolves");
        let fragment = &plan.fragments[0];
        let mut connections = fragment.connections.clone();
        let mut reverse = connections[0].clone();
        core::mem::swap(
            &mut reverse.source_placement_id,
            &mut reverse.sink_placement_id,
        );
        connections.push(reverse);
        assert_eq!(startup_order(&fragment.placements, &connections), None);
    }

    #[test]
    fn planning_rejects_invalid_host_operation_requirements() {
        let form = form();
        let mut host = host();
        host.capabilities[0].host_operations[0].maximum_in_flight = 0;
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&host),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::InvalidHostOperationRequirement(_))
        ));
    }

    #[test]
    fn planning_rejects_invalid_unavailable_ambiguous_and_exhausted_resources() {
        let form = form();

        let mut advertised = host();
        advertised.capabilities[0].resource_requirements[0].units = 0;
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::InvalidResourceContract(_))
        ));

        let mut advertised = host();
        advertised.resources.retain(|resource| {
            resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS
        });
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::UnavailableResource(_))
        ));

        let mut advertised = host();
        advertised.resources.push(conduit_core::resource_offer(
            "zz-test/presentation",
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            4,
        ));
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::InvalidResourceContract(_))
        ));

        let mut advertised = host();
        advertised.capabilities[1].resource_requirements[0].units = 5;
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::ResourceCapacityExceeded(_))
        ));

        let mut advertised = host();
        advertised.resources[1].capacity_units = u32::MAX;
        advertised.capabilities[0].resource_requirements[0].units = u32::MAX;
        advertised.capabilities[1].resource_requirements[0] =
            conduit_core::resource_requirement(conduit_core::TIMER_RESOURCE_CLASS, 1);
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::ResourceCapacityExceeded(_))
        ));
    }

    #[test]
    fn planning_binds_exact_authority_and_rejects_missing_stale_or_ambiguous_grants() {
        let form = form();
        let mut invalid = host();
        invalid.capabilities[1].authority_requirements = vec![conduit_core::AuthorityRequirement {
            contract_id: conduit_core::AuthorityContractId::from(
                conduit_core::PRESENT_AUTHORITY_CONTRACT,
            ),
            host_operation_contract_id: conduit_core::HostOperationContractId::from(
                conduit_core::WAIT_HOST_OPERATION_CONTRACT,
            ),
            subject_kind: kind_id(SIGNAL_PRESENTATION_KIND),
        }];
        let invalid_placements = default_placements(&form, std::slice::from_ref(&invalid))
            .expect("placements resolve before authority validation");
        assert!(matches!(
            plan_with_authority_grants(
                &form,
                std::slice::from_ref(&invalid),
                &invalid_placements,
                &[ConnectionProvider::Local],
                &[],
            ),
            Err(PlannerError::InvalidAuthorityContract(_))
        ));

        let mut advertised = host();
        let requirement = present_authority_requirement(kind_id(SIGNAL_PRESENTATION_KIND));
        advertised.capabilities[1].authority_requirements = vec![requirement.clone()];
        let placements = default_placements(&form, std::slice::from_ref(&advertised))
            .expect("placements resolve without implying authority");

        assert!(matches!(
            plan_with_authority_grants(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
                &[],
            ),
            Err(PlannerError::AuthorityGrantMissing(_))
        ));

        let stale = authority_grant(
            "grant/stale",
            &requirement,
            advertised.host_id.clone(),
            conduit_core::BootId::from("stale-boot"),
            advertised.capabilities[1].capability_id.clone(),
        );
        assert!(matches!(
            plan_with_authority_grants(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
                &[stale],
            ),
            Err(PlannerError::AuthorityGrantMissing(_))
        ));

        let grant = authority_grant(
            "grant/show",
            &requirement,
            advertised.host_id.clone(),
            advertised.boot_id.clone(),
            advertised.capabilities[1].capability_id.clone(),
        );
        let mut duplicate_scope = grant.clone();
        duplicate_scope.grant_id = conduit_core::AuthorityGrantId::from("grant/show-alternate");
        assert!(matches!(
            plan_with_authority_grants(
                &form,
                std::slice::from_ref(&advertised),
                &placements,
                &[ConnectionProvider::Local],
                &[grant.clone(), duplicate_scope],
            ),
            Err(PlannerError::AuthorityGrantAmbiguous(_))
        ));

        let plan = plan_with_authority_grants(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionProvider::Local],
            std::slice::from_ref(&grant),
        )
        .expect("exact grant resolves");
        let show = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
            .expect("show placement exists");
        assert_eq!(show.authority.len(), 1);
        assert_eq!(show.authority[0].grant_id, grant.grant_id);
        assert_eq!(show.authority[0].host_id, advertised.host_id);
        assert_eq!(show.authority[0].boot_id, advertised.boot_id);
    }

    #[test]
    fn planning_binds_one_exact_observed_link_and_rejects_unproven_remote_providers() {
        let form = form();
        let source = host();
        let mut sink = host();
        sink.host_id = HostId::from("remote-host");
        sink.boot_id = conduit_core::BootId::from("remote-boot");
        let realm = [source.clone(), sink.clone()];
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    conduit_core::OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: source.host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    conduit_core::OperationId::from("show"),
                    PlacementChoice {
                        host_id: sink.host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    },
                ),
            ]),
        };
        assert!(matches!(
            plan_with_link_bindings(
                &form,
                &realm,
                &placements,
                &[ConnectionProvider::FixtureFrame],
                4,
                64,
                &[],
            ),
            Err(PlannerError::LinkBindingMissing(_))
        ));

        let exact = process_owned_link_binding(
            "link/source-remote",
            ConnectionProvider::FixtureFrame,
            "fixture/frame/source-remote",
            &source,
            &sink,
            4,
            64,
        );
        let mut stale = exact.clone();
        stale.sink.boot_id = conduit_core::BootId::from("stale-boot");
        assert!(matches!(
            plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[stale]),
            Err(PlannerError::LinkBindingMissing(_))
        ));

        let mut unavailable = exact.clone();
        unavailable.availability = conduit_core::LinkAvailability::Unavailable;
        assert!(matches!(
            plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[unavailable],),
            Err(PlannerError::LinkBindingUnavailable(_))
        ));

        let mut underbounded = exact.clone();
        underbounded.limits.maximum_buffered_bytes = 63;
        assert!(matches!(
            plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[underbounded],),
            Err(PlannerError::LinkBindingUnavailable(_))
        ));

        let mut alternate = exact.clone();
        alternate.binding_id = conduit_core::LinkBindingId::from("link/source-remote-alternate");
        assert!(matches!(
            plan_with_link_bindings(
                &form,
                &realm,
                &placements,
                &[],
                4,
                64,
                &[exact.clone(), alternate],
            ),
            Err(PlannerError::LinkBindingAmbiguous(_))
        ));

        let mut invalid = exact.clone();
        invalid.provider_instance_id = conduit_core::ConnectionProviderInstanceId::from("");
        assert!(matches!(
            plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[invalid]),
            Err(PlannerError::InvalidLinkBinding(_))
        ));

        let mut invalid_credential = exact.clone();
        invalid_credential.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from(""),
        );
        assert!(matches!(
            plan_with_link_bindings(
                &form,
                &realm,
                &placements,
                &[],
                4,
                64,
                &[invalid_credential],
            ),
            Err(PlannerError::InvalidLinkBinding(_))
        ));

        let mut invalid_authority = exact.clone();
        invalid_authority.authority =
            conduit_core::LinkAuthorityReference::Grant(conduit_core::AuthorityGrantId::from(""));
        assert!(matches!(
            plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[invalid_authority],),
            Err(PlannerError::InvalidLinkBinding(_))
        ));

        let mut secured = exact;
        secured.credential = conduit_core::LinkCredentialReference::Opaque(
            conduit_core::CredentialReferenceId::from("credential/source-remote"),
        );
        secured.authority = conduit_core::LinkAuthorityReference::Grant(
            conduit_core::AuthorityGrantId::from("grant/source-remote"),
        );
        let plan = plan_with_link_bindings(
            &form,
            &realm,
            &placements,
            &[],
            4,
            64,
            std::slice::from_ref(&secured),
        )
        .expect("an observed link, not a global provider enum, resolves the remote cord");
        assert!(verify_plan(&plan));
        let connection = plan.fragments[0]
            .connections
            .first()
            .expect("remote connection exists");
        assert_eq!(connection.provider, secured.provider);
        assert_eq!(connection.link_binding.as_ref(), Some(&secured));
    }

    #[test]
    fn planning_link_binding_mutations_change_fragment_identity() {
        let form = form();
        let source = host();
        let mut sink = host();
        sink.host_id = HostId::from("remote-host");
        sink.boot_id = conduit_core::BootId::from("remote-boot");
        let realm = [source.clone(), sink.clone()];
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    conduit_core::OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: source.host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    conduit_core::OperationId::from("show"),
                    PlacementChoice {
                        host_id: sink.host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    },
                ),
            ]),
        };
        let link = process_owned_link_binding(
            "link/mutation",
            ConnectionProvider::FixtureFrame,
            "fixture/frame/mutation",
            &source,
            &sink,
            4,
            64,
        );
        let original = plan_with_link_bindings(&form, &realm, &placements, &[], 4, 64, &[link])
            .expect("remote plan resolves")
            .fragments[0]
            .clone();

        for field in 0..14 {
            let mut mutated = original.clone();
            let binding = mutated.connections[0]
                .link_binding
                .as_mut()
                .expect("remote binding exists");
            match field {
                0 => binding.binding_id = conduit_core::LinkBindingId::from("mutated/link"),
                1 => binding.source.host_id = HostId::from("mutated-source"),
                2 => binding.source.boot_id = conduit_core::BootId::from("mutated-source-boot"),
                3 => {
                    binding.source.endpoint_id =
                        conduit_core::LinkEndpointId::from("mutated-source-endpoint")
                }
                4 => binding.sink.host_id = HostId::from("mutated-sink"),
                5 => binding.sink.boot_id = conduit_core::BootId::from("mutated-sink-boot"),
                6 => {
                    binding.sink.endpoint_id =
                        conduit_core::LinkEndpointId::from("mutated-sink-endpoint")
                }
                7 => binding.provider = ConnectionProvider::FixtureDatagram,
                8 => {
                    binding.provider_instance_id =
                        conduit_core::ConnectionProviderInstanceId::from("mutated/provider")
                }
                9 => binding.availability = conduit_core::LinkAvailability::Unavailable,
                10 => {
                    binding.credential = conduit_core::LinkCredentialReference::Opaque(
                        conduit_core::CredentialReferenceId::from("mutated/credential"),
                    )
                }
                11 => {
                    binding.authority = conduit_core::LinkAuthorityReference::Grant(
                        conduit_core::AuthorityGrantId::from("mutated/grant"),
                    )
                }
                12 => binding.limits.maximum_in_flight_items += 1,
                13 => binding.limits.maximum_buffered_bytes += 1,
                _ => unreachable!(),
            }
            assert!(!verify_plan_fragment(&mutated));
        }
    }

    #[test]
    fn planning_verification_rejects_each_top_level_form_identity_mutation() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let original = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("exact plan resolves");

        let mut source_changed = form.clone();
        source_changed.source_document_id = SourceDocumentId::from("changed-source");
        let source_plan = plan(
            &source_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("source-identity plan resolves");
        assert_ne!(original.plan_id, source_plan.plan_id);

        let mut checked_changed = form.clone();
        checked_changed.checked_form_id = conduit_core::CheckedFormId::from("changed-checked");
        let checked_plan = plan(
            &checked_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("checked-identity plan resolves");
        assert_ne!(original.plan_id, checked_plan.plan_id);

        let mut expanded_changed = form.clone();
        expanded_changed.expanded_form_id = ExpandedFormId::from("changed-expanded");
        let expanded_plan = plan(
            &expanded_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("expanded-identity plan resolves");
        assert_ne!(original.plan_id, expanded_plan.plan_id);

        let mut mutated = original.clone();
        mutated.source_document_id = SourceDocumentId::from("mutated-source");
        assert!(!verify_plan(&mutated));

        let mut mutated = original.clone();
        mutated.checked_form_id = conduit_core::CheckedFormId::from("mutated-checked");
        assert!(!verify_plan(&mutated));

        let mut mutated = original;
        mutated.expanded_form_id = ExpandedFormId::from("mutated-expanded");
        assert!(!verify_plan(&mutated));
    }

    #[test]
    fn planning_rejects_contract_revision_and_nonfirst_port_mismatch() {
        let form = form();
        let original_host = host();
        let placements = default_placements(&form, std::slice::from_ref(&original_host))
            .expect("placements must resolve");

        let mut mismatched_revision = original_host.clone();
        mismatched_revision.capabilities[0].kind_contract_revision =
            conduit_core::KindContractRevision::from("mutated/flow-pulse@1");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&mismatched_revision),
                &placements,
                &[ConnectionProvider::Local]
            ),
            Err(PlannerError::WrongKindContractRevision(_))
        ));

        let mut mismatched_ports = original_host;
        mismatched_ports.capabilities[0]
            .outputs
            .push(conduit_core::PortDescriptor {
                port_id: conduit_core::PortId::from("unexpected"),
                value_kind: kind_id("value/unexpected"),
                direction: conduit_core::PortDirection::Output,
            });
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&mismatched_ports),
                &placements,
                &[ConnectionProvider::Local]
            ),
            Err(PlannerError::IncompatiblePortContract(_))
        ));
    }

    #[test]
    fn planning_rejects_unknown_host() {
        let form = form();
        let placements = parse_placements(
            "placements 0\npulse:\n    host = \"missing\"\n    capability = \"pulse-1\"\nshow:\n    host = \"missing\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
        let error = plan(&form, &[host()], &placements, &[ConnectionProvider::Local])
            .expect_err("planning should fail");
        assert!(matches!(error, PlannerError::UnknownHost(_)));
    }
}
