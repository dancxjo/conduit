use conduit_core::{
    mandatory_evidence_storage_requirement, seal_plan, AuthorityBinding, AuthorityGrant, BoundLink,
    CancellationPolicy, CapabilityId, ConnectionId, ConnectionProvider, ExpectedEvidence,
    ExpectedTerminal, FragmentId, HostAdvertisement, HostId, LinkAvailability, LinkBinding,
    LinkBindingId, OperationId, PlacementId, Plan, PlanFragment, PlanId, PlannedConnection,
    PlannedOperation, ResourceBinding, ResourcePoolId, StartupDependency, TerminalPolicy,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedForm, CheckedOperation};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

mod canonical;
mod characteristic_policy;
mod characteristics;
mod compute_admission;
mod contract;
mod diagnostic;
mod functional_compatibility;
mod observations;
mod policy;
mod profile;
mod protected_resources;
mod realization;
mod replanning;
mod requirements;

use functional_compatibility::default_placements_unvalidated;
use protected_resources::{bind_protected_resource, validate_protected_resource_grants};

pub use canonical::{
    default_expanded_placements, plan_expanded_canonical, plan_expanded_canonical_with_options,
    plan_expanded_canonical_with_shared_pools, SharedPoolPlanningRequirement,
};
pub use characteristics::{
    plan_selected_realizations_with_characteristics, select_realization_with_characteristics,
    select_realization_with_characteristics_and_evidence, RealizationDecisionDisposition,
    RealizationDecisionRecord, RealizationRejection, RealizationSelection,
    MAXIMUM_REALIZATION_DECISION_RECORDS,
};
pub use contract::{
    parse_placements, PlacementChoice, PlacementChoices, PlannerError, PlanningOptions,
};
pub use diagnostic::structured_planner_diagnostic;
pub use observations::select_realization_with_observations;
pub use policy::{select_realization_with_policy, RealizationPolicy, RealizationPreference};
pub use profile::{
    plan_with_advertised_profile, BROWSER_PLANNER_PROFILE, FULL_PLANNER_LIMITS,
    FULL_PLANNER_PROFILE,
};
pub use realization::plan_selected_realizations;
pub use replanning::{replan_selected_realizations_with_characteristics, RealizationReplanOutcome};
pub use requirements::{plan_with_hard_requirements, HardRealizationRequirements};

pub fn default_placements(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    default_placements_unvalidated(&form.operations, realm)
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
            route_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants,
            protected_resource_grants: &[],
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
            route_candidates: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
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
            route_candidates: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
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
    form.validate_identities()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    plan_validated_form(form, realm, placements, providers, options)
}

pub(crate) fn plan_validated_form(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    let PlanningOptions {
        connection_providers,
        route_candidates,
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
        protected_resource_grants,
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
    validate_protected_resource_grants(protected_resource_grants)?;
    validate_link_bindings(link_bindings)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut remaining_compute_minimum =
        compute_admission::admit_minima(form, &realm_index, placements)?;
    let mut consumed_protected_handles = BTreeSet::new();
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
            let key = (host.host_id.clone(), resource.pool_id.clone());
            let reserved_for_later = if requirement.compute.is_some() {
                let remaining = remaining_compute_minimum
                    .get_mut(&key)
                    .expect("compute minimum was pre-admitted");
                *remaining -= requirement.units;
                *remaining
            } else {
                0
            };
            let available = resource
                .capacity_units
                .saturating_sub(*used)
                .saturating_sub(reserved_for_later);
            let compute = match &requirement.compute {
                Some(_) => Some(
                    conduit_core::compute_reservation(requirement, resource, available)
                        .ok_or_else(|| {
                            PlannerError::UnavailableResource(format!(
                                "pool '{}' cannot satisfy the compute range, service, or topology contract",
                                resource.pool_id.as_str()
                            ))
                        })?,
                ),
                None => None,
            };
            let selected_units = compute
                .as_ref()
                .map_or(requirement.units, |reservation| reservation.selected_lanes);
            *used = used.checked_add(selected_units).ok_or_else(|| {
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
            let protected = bind_protected_resource(
                requirement,
                protected_resource_grants,
                operation,
                host,
                capability,
                &mut consumed_protected_handles,
            )?;
            resource_bindings.push(ResourceBinding {
                pool_id: resource.pool_id.clone(),
                class_id: resource.class_id.clone(),
                units: selected_units,
                protected,
                compute,
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
            kind_id: capability.kind_id.clone(),
            kind_contract_revision: capability.kind_contract_revision.clone(),
            execution_profile_id: capability.implementation.execution_profile_id.clone(),
            configuration: operation.configuration.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation.implementation_id.clone(),
            artifact_id: capability.implementation.artifact_id.clone(),
            realization_characteristics: Vec::new(),
            limits: capability.limits.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            host_operations: capability.host_operations.clone(),
            resources: resource_bindings,
            authority: authority_bindings,
            pool_references: operation.pool_references.clone(),
        });
    }

    if consumed_protected_handles.len() != protected_resource_grants.len() {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "every supplied protected-resource grant must be consumed by one exact planned role"
                .to_string(),
        ));
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
        let (provider, link_binding, sealed_candidates) = select_provider(ProviderSelection {
            source: source_plan,
            sink: sink_plan,
            providers,
            requested: connection_providers
                .get(&(
                    connection.source_operation_id.clone(),
                    connection.sink_operation_id.clone(),
                ))
                .copied(),
            requested_candidates: route_candidates.get(&(
                connection.source_operation_id.clone(),
                connection.sink_operation_id.clone(),
            )),
            link_bindings,
            connection_item_capacity,
            connection_byte_capacity,
        })?;
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
                "connection:{}:{}:{}:{}:{}:{}:{}",
                form.checked_form_id.as_str(),
                connection.source_operation_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_operation_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str(),
                connection.temporal.as_str(),
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            temporal: connection.temporal,
            provider,
            link_binding,
            route_candidates: sealed_candidates,
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
                .filter(|connection| connection.source_placement_id != connection.sink_placement_id)
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
                shared_pools: Vec::new(),
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
                    connection.source_placement_id == connection.sink_placement_id
                        || &connection.source_placement_id != *candidate
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
    if capability.checked_face() != operation.checked_face() {
        return Err(PlannerError::IncompatibleCheckedFace(format!(
            "operation '{}' face differs from capability '{}' face",
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
    if capability.resource_requirements.iter().any(|requirement| {
        requirement.class_id.as_str().is_empty()
            || requirement.units == 0
            || requirement
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_units(requirement.units))
            || requirement
                .protected_role
                .as_ref()
                .is_some_and(|role| role.as_str().is_empty())
    }) || capability
        .resource_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' requirements must have non-empty classes and protected roles, positive units, and unique canonical ordering",
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
            || resource
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_capacity(resource.capacity_units))
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

struct ProviderSelection<'a> {
    source: &'a PlannedOperation,
    sink: &'a PlannedOperation,
    providers: &'a [ConnectionProvider],
    requested: Option<ConnectionProvider>,
    requested_candidates: Option<&'a Vec<LinkBindingId>>,
    link_bindings: &'a [LinkBinding],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
}

fn select_provider(
    selection: ProviderSelection<'_>,
) -> Result<(ConnectionProvider, Option<LinkBinding>, Vec<BoundLink>), PlannerError> {
    let ProviderSelection {
        source,
        sink,
        providers,
        requested,
        requested_candidates,
        link_bindings,
        connection_item_capacity,
        connection_byte_capacity,
    } = selection;
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
        if requested_candidates.is_some_and(|candidates| !candidates.is_empty()) {
            return Err(PlannerError::InvalidLinkBinding(
                "local connections cannot seal remote route candidates".to_string(),
            ));
        }
        return Ok((ConnectionProvider::Local, None, Vec::new()));
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
                && binding.limits.maximum_payload_bytes >= connection_byte_capacity
                && binding.limits.maximum_buffered_bytes >= connection_byte_capacity
                && binding.limits.maximum_frame_bytes >= binding.limits.maximum_payload_bytes
        })
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Err(PlannerError::LinkBindingUnavailable(format!(
            "observed link for '{}' -> '{}' is unavailable or below item/byte limits",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    if let Some(requested_candidates) = requested_candidates {
        let unique_candidates = requested_candidates.iter().collect::<BTreeSet<_>>();
        if requested_candidates.is_empty() || unique_candidates.len() != requested_candidates.len()
        {
            return Err(PlannerError::InvalidLinkBinding(
                "route candidate policy must be non-empty and contain no duplicates".to_string(),
            ));
        }
        let mut selected = Vec::with_capacity(requested_candidates.len());
        for binding_id in requested_candidates {
            let matches = ready
                .iter()
                .filter(|binding| &binding.binding_id == binding_id)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(PlannerError::LinkBindingMissing(format!(
                    "requested route '{}' is not one exact ready bounded link",
                    binding_id.as_str()
                )));
            }
            selected.push((*matches[0]).clone());
        }
        let first = selected[0].clone();
        return Ok((
            first.provider,
            Some(first),
            selected
                .iter()
                .map(|binding| binding.bound_link())
                .collect(),
        ));
    }
    if ready.len() != 1 {
        return Err(PlannerError::LinkBindingAmbiguous(format!(
            "multiple observed links satisfy '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let binding = ready[0].clone();
    Ok((
        binding.provider,
        Some(binding.clone()),
        vec![binding.bound_link()],
    ))
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
            || binding.limits.maximum_payload_bytes == 0
            || binding.limits.maximum_buffered_bytes == 0
            || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
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
mod tests;
