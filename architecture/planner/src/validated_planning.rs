//! Assemble one Plan from already validated plot identities and native offers.
use super::*;
use crate::planning_input::PlanningInput;
mod connections;
mod quantity_wire;

pub(crate) fn plan_validated_plot(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    plan_validated_plot_with_connection_limits(
        plot,
        hosts,
        placements,
        bases,
        options,
        &BTreeMap::new(),
    )
}

pub(crate) fn plan_validated_plot_with_connection_limits(
    plot: &CheckedPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    plan_input_with_connection_limits(
        PlanningInput::from(plot),
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )
}

pub(crate) fn plan_input_with_connection_limits(
    mut plot: PlanningInput<'_>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let PlanningOptions {
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
        protected_resource_grants,
        line_offers,
        ..
    } = options;
    if connection_item_capacity == 0 || connection_byte_capacity == 0 {
        return Err(PlannerError::InvalidConnectionBudget(
            "item and byte capacity must both be nonzero".to_string(),
        ));
    }
    for (endpoints, limits) in connection_limits {
        if limits.item_capacity == 0 || limits.byte_capacity == 0 {
            return Err(PlannerError::InvalidConnectionBudget(
                "per-connection item and byte capacity must both be nonzero".to_string(),
            ));
        }
        if !plot
            .connections
            .iter()
            .any(|connection| connection_endpoints(connection) == *endpoints)
        {
            return Err(PlannerError::InvalidConnectionBudget(
                "per-connection capacity names a Cord absent from the checked plot".to_string(),
            ));
        }
    }
    let host_index = hosts
        .iter()
        .map(|host| (host.host_id.clone(), host))
        .collect::<BTreeMap<_, _>>();

    crate::language_coverage::validate_placements(&plot.gears, hosts, placements)?;
    for host in hosts {
        validate_host_resources(host)?;
    }
    validate_authority_grants(authority_grants)?;
    validate_protected_resource_grants(protected_resource_grants)?;
    validate_line_offers(line_offers)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut remaining_compute_minimum =
        compute_admission::admit_minima(&plot.gears, &host_index, placements)?;
    let mut consumed_protected_handles = BTreeSet::new();
    let mut resource_writers = BTreeSet::new();
    let mut planned_gears = Vec::<PlannedGear>::new();
    let mut placement_lookup = BTreeMap::<GearId, PlacementId>::new();

    for gear_index in 0..plot.gears.len() {
        let gear = plot.gears.get(gear_index)?;
        let choice = placements
            .by_gear
            .get(&gear.gear_id)
            .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().to_string()))?;
        let host = host_index
            .get(&choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
            })?;
        validate_operation_capability(&gear, capability)?;
        validate_keep_retention(&gear, capability)?;

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

        let resource_bindings = resource_binding::bind_resources(
            host,
            capability,
            &gear,
            protected_resource_grants,
            resource_binding::ResourcePlanningState {
                writers: &mut resource_writers,
                usage: &mut resource_usage,
                compute_minimum: &mut remaining_compute_minimum,
                protected_handles: &mut consumed_protected_handles,
            },
        )?;
        let base = selected_base_provider(host, capability, &resource_bindings)?;

        let mut authority_bindings = Vec::with_capacity(capability.authority_requirements.len());
        for requirement in &capability.authority_requirements {
            let mut matches = authority_grants.iter().filter(|grant| {
                grant.contract_id == requirement.contract_id
                    && grant.host_call_contract_id == requirement.host_call_contract_id
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
                host_call_contract_id: grant.host_call_contract_id.clone(),
                subject_kind: grant.subject_kind.clone(),
                host_id: grant.host_id.clone(),
                boot_id: grant.boot_id.clone(),
                capability_id: grant.capability_id.clone(),
            });
        }
        authority_bindings.sort();

        let placement_id = PlacementId::from(hash_string(&format!(
            "placement:{}:{}:{}:{}",
            plot.checked_plot_id.as_str(),
            gear.gear_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(gear.gear_id.clone(), placement_id.clone());
        let mut planned = conduit_core::planned_gear_from_parts! {
            realization_properties: capability.realization_properties.clone(),
            placement_id,
            gear_id: gear.gear_id.clone(),
            kind_id: capability.kind_id.clone(),
            kind_contract_revision: capability.kind_contract_revision.clone(),
            execution_profile_id: capability.implementation.execution_profile_id.clone(),
            configuration: Vec::new(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation.implementation_id.clone(),
            artifact_id: capability.implementation.artifact_id.clone(),
            base,
            realization_characteristics: Vec::new(),
            limits: capability.limits.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            semantic_contract: capability.semantic_contract.clone(),
            terminal_transductions: gear.terminal_transductions.clone(),
            host_calls: capability.host_calls.clone(),
            resources: resource_bindings,
            authority: authority_bindings,
            pool_references: gear.pool_references.clone(),
        };
        planned.configuration = match gear {
            alloc::borrow::Cow::Owned(gear) => {
                plot.gears.retain_connection_facts(&gear);
                gear.configuration
            }
            alloc::borrow::Cow::Borrowed(_) => plot.gears.take_memory_configuration(gear_index),
        };
        planned_gears.push(planned);
    }

    if consumed_protected_handles.len() != protected_resource_grants.len() {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "every supplied protected-resource grant must be consumed by one exact planned role"
                .to_string(),
        ));
    }

    for gear in placements.by_gear.keys() {
        if plot.gears.connection_gear(gear).is_none() {
            return Err(PlannerError::UnknownGear(gear.as_str().to_string()));
        }
    }

    let planned_connections = connections::plan(
        &plot,
        hosts,
        &planned_gears,
        &placement_lookup,
        bases,
        options,
        connection_limits,
    )?;

    let global_startup_order = startup::startup_order(&planned_gears, &planned_connections)?
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(plot.name.into()))?;

    // A placement belongs to exactly one Host. Move its retained program into
    // that fragment instead of copying every program while both lists are live.
    let mut placements_by_host = BTreeMap::<HostId, Vec<PlannedGear>>::new();
    for placement in planned_gears {
        placements_by_host
            .entry(placement.host_id.clone())
            .or_default()
            .push(placement);
    }
    let fragments = hosts
        .iter()
        .map(|host| -> Result<Option<PlanFragment>, PlannerError> {
            let placements = placements_by_host.remove(&host.host_id).unwrap_or_default();
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
            let startup_dependencies = startup::startup_dependencies(&placements, &connections)?;
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
            let states = placements
                .iter()
                .map(planned_keep_state)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let expected_sign = core::iter::once(ExpectedSign::PlanFragmentReceived)
                .chain(placements.iter().map(|placement| {
                    ExpectedSign::PlacementPrepared(placement.placement_id.clone())
                }))
                .chain(placements.iter().map(|placement| {
                    ExpectedSign::PlacementTerminal(placement.placement_id.clone())
                }))
                .chain(connections.iter().map(|connection| {
                    ExpectedSign::ConnectionTerminal(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedSign::PlanTerminal))
                .collect::<Vec<_>>();
            let mut sign_storage_budget = mandatory_sign_storage_requirement(&expected_sign)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            let state_budget = conduit_core::state_resource_budget(&states).map_err(|error| {
                PlannerError::InvalidStateContract(format!(
                    "host '{}' retained State admission: {error:?}",
                    host.host_id.as_str()
                ))
            })?;
            sign_storage_budget.item_capacity = sign_storage_budget
                .item_capacity
                .checked_add(state_budget.sign_storage.item_capacity)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            sign_storage_budget.byte_capacity = sign_storage_budget
                .byte_capacity
                .checked_add(state_budget.sign_storage.byte_capacity)
                .ok_or_else(|| {
                    PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            Ok(Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
                completion_policy: plan_completion_policy(plot.completion),
                realization_backs: Vec::new(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                states,
                connections,
                fore_ports: Vec::new(),
                shared_pools: Vec::new(),
                startup_dependencies,
                startup_order,
                cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals,
                expected_sign,
                sign_storage_budget,
                plan_fragments: Vec::new(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(seal_plan_with_completion(
        plot.identity(),
        plan_completion_policy(plot.completion),
        fragments,
    ))
}
