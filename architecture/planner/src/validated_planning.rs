//! Assemble one Plan from already validated plot identities and native offers.
use super::*;
use crate::planning_input::PlanningInput;

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
    plan_borrowed_plot_with_connection_limits(
        PlanningInput::from(plot),
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )
}

pub(crate) fn plan_borrowed_plot_with_connection_limits(
    plot: PlanningInput<'_>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let line_policy = contract::LineMechanismPolicy::new(bases);
    let PlanningOptions {
        connection_bases,
        line_candidates,
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
        protected_resource_grants,
        line_offers,
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

    for host in hosts {
        validate_host_resources(host)?;
    }
    validate_authority_grants(authority_grants)?;
    validate_protected_resource_grants(protected_resource_grants)?;
    validate_line_offers(line_offers)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut remaining_compute_minimum =
        compute_admission::admit_minima(plot.gears, &host_index, placements)?;
    let mut consumed_protected_handles = BTreeSet::new();
    let mut resource_writers = BTreeSet::new();
    let mut planned_gears = Vec::<PlannedGear>::new();
    let mut placement_lookup = BTreeMap::<GearId, PlacementId>::new();

    for gear in plot.gears {
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
        validate_operation_capability(gear, capability)?;
        validate_keep_retention(gear, capability)?;

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
            gear,
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
        planned_gears.push(conduit_core::planned_gear_from_parts! {
            placement_id,
            gear_id: gear.gear_id.clone(),
            kind_id: capability.kind_id.clone(),
            kind_contract_revision: capability.kind_contract_revision.clone(),
            execution_profile_id: capability.implementation.execution_profile_id.clone(),
            configuration: gear.configuration.clone(),
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
        });
    }

    if consumed_protected_handles.len() != protected_resource_grants.len() {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "every supplied protected-resource grant must be consumed by one exact planned role"
                .to_string(),
        ));
    }

    for gear in placements.by_gear.keys() {
        if !plot.gears.iter().any(|item| &item.gear_id == gear) {
            return Err(PlannerError::UnknownGear(gear.as_str().to_string()));
        }
    }

    let mut planned_connections = Vec::<PlannedConnection>::new();
    for connection in plot.connections {
        let limits = connection_limits
            .get(&connection_endpoints(connection))
            .copied()
            .unwrap_or(ConnectionQueueLimits {
                item_capacity: connection_item_capacity,
                byte_capacity: connection_byte_capacity,
            });
        let source_placement = placement_lookup
            .get(&connection.source_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.source_gear_id.as_str().to_string())
            })?;
        let sink_placement = placement_lookup
            .get(&connection.sink_gear_id)
            .ok_or_else(|| {
                PlannerError::UnknownGear(connection.sink_gear_id.as_str().to_string())
            })?;
        let source_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == source_placement)
            .expect("source placement must exist");
        let sink_plan = planned_gears
            .iter()
            .find(|item| &item.placement_id == sink_placement)
            .expect("sink placement must exist");
        let source_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.source_gear_id)
            .expect("checked source gear must exist");
        let sink_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.sink_gear_id)
            .expect("checked sink gear must exist");
        let resource = resource_port::plan_resource_connection(
            connection,
            source_gear,
            sink_gear,
            source_plan,
            sink_plan,
        )?;
        let (selected_line, admitted_lines) = select_line(LineSelection {
            source: source_plan,
            sink: sink_plan,
            policy: line_policy,
            requested: connection_bases
                .get(&(
                    connection.source_gear_id.clone(),
                    connection.sink_gear_id.clone(),
                ))
                .cloned(),
            requested_candidates: line_candidates.get(&(
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            )),
            line_offers,
            connection_item_capacity: limits.item_capacity,
            connection_byte_capacity: limits.byte_capacity,
        })?;
        let source_capability =
            find_capability(hosts, &source_plan.host_id, &source_plan.capability_id)?;
        let sink_capability = find_capability(hosts, &sink_plan.host_id, &sink_plan.capability_id)?;
        if limits.item_capacity > source_capability.limits.max_queue_items
            || limits.item_capacity > sink_capability.limits.max_queue_items
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires item capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.item_capacity
            )));
        }
        if limits.byte_capacity > source_capability.limits.max_queue_bytes
            || limits.byte_capacity > sink_capability.limits.max_queue_bytes
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires byte capacity {}",
                source_plan.gear_id.as_str(),
                sink_plan.gear_id.as_str(),
                limits.byte_capacity
            )));
        }
        planned_connections.push(PlannedConnection {
            connection_id: ConnectionId::from(hash_string(&format!(
                "connection:{}:{}:{}:{}:{}:{}:{}:{}",
                plot.checked_plot_id.as_str(),
                connection.source_gear_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_gear_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str(),
                connection.track.as_str(),
                connection.temporal.as_str(),
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            resource,
            abnormal_kind: source_capability
                .outputs
                .iter()
                .find(|port| port.port_id == connection.source_port_id)
                .and_then(|port| port.abnormal_kind.clone()),
            track: connection.track,
            temporal: connection.temporal,
            pressure_policy: if source_plan.kind_id.as_str() == "flow/coalesce-latest" {
                DeliveryPressurePolicy::CoalesceLatest
            } else {
                DeliveryPressurePolicy::PreserveOrder
            },
            selected_line,
            admitted_lines,
            item_capacity: limits.item_capacity,
            byte_capacity: limits.byte_capacity,
        });
    }

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
