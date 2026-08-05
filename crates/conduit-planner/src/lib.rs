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
    InvalidFormIdentity(String),
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
            PlannerError::InvalidFormIdentity(value) => {
                write!(f, "invalid form identity: {value}")
            }
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

pub mod placement;
mod validation;

pub use placement::{
    default_placements, parse_placements, plan, plan_with_authority_grants,
    plan_with_connection_limits, plan_with_connection_limits_and_provider_overrides,
    plan_with_link_bindings,
};

use validation::{
    find_capability, hash_string, select_provider, startup_order,
    validate_authority_grants, validate_host_resources, validate_link_bindings,
    validate_operation_capability,
};

pub fn plan_with_options(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    form.validate_identities()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
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

#[cfg(test)]
mod tests;
