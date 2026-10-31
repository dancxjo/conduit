//! Assemble one Plan from already validated plot identities and native offers.
use super::*;
use crate::planning_input::PlanningInput;
mod authority;
mod connections;
mod hosts;
pub(crate) use hosts::PlanningHosts;
mod quantity_wire;
pub(crate) mod retention;

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
    plot: PlanningInput<'_>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let identity = (
        plot.source_document_id,
        plot.checked_plot_id,
        plot.expanded_plot_id,
    );
    let completion = plan_completion_policy(plot.completion);
    let fragments = plan_fragments_with_connection_limits(
        plot,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
        &mut retention::ConfigurationRetention::new(false),
    )?;
    let identity = conduit_core::PlotIdentity {
        source_document_id: identity.0.clone(),
        checked_plot_id: identity.1.clone(),
        expanded_plot_id: identity.2.clone(),
    };
    Ok(seal_plan_with_completion(identity, completion, fragments))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_fragments_with_connection_limits(
    plot: PlanningInput<'_>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    retention: &mut retention::ConfigurationRetention,
) -> Result<Vec<PlanFragment>, PlannerError> {
    plan_fragments_with_host_inputs(
        plot,
        PlanningHosts::Borrowed(hosts),
        placements,
        bases,
        options,
        connection_limits,
        retention,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_fragments_with_host_inputs(
    mut plot: PlanningInput<'_>,
    mut hosts: PlanningHosts<'_>,
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
    retention: &mut retention::ConfigurationRetention,
) -> Result<Vec<PlanFragment>, PlannerError> {
    let retiring = hosts.retiring();
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

    hosts.validate_placements(&plot.gears, placements)?;
    hosts.validate_resources()?;
    validate_authority_grants(authority_grants)?;
    validate_protected_resource_grants(protected_resource_grants)?;
    validate_line_offers(line_offers)?;

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut resource_usage = BTreeMap::<(HostId, ResourcePoolId), u32>::new();
    let mut remaining_compute_minimum = match &hosts {
        PlanningHosts::Borrowed(_) | PlanningHosts::Owned(_) => {
            compute_admission::admit_minima(&plot.gears, &host_index, placements)?
        }
        #[cfg(feature = "indexed-source")]
        PlanningHosts::Indexed(_) => {
            compute_admission::admit_access_minima(&plot.gears, &hosts.access(), placements)?
        }
    };
    // Global host/language/grant/minimum checks above still inspect the complete
    // snapshot. Ordinary placement and Cord checks below only use selected offers.
    drop(host_index);
    hosts.retain_selected(placements);
    // These final fragment IDs are needed regardless of placement count.
    // Allocate them while the arena has contiguous space, then move them.
    struct FragmentHost {
        identity: conduit_core::PlotIdentity,
        host_id: HostId,
        boot_id: conduit_core::BootId,
        offer_generation: conduit_core::OfferGeneration,
    }
    let fragment_hosts = hosts
        .iter()
        .filter(|host| {
            placements
                .by_gear
                .values()
                .any(|choice| choice.host_id == host.host_id)
        })
        .map(|host| FragmentHost {
            identity: conduit_core::PlotIdentity {
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
            },
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
        })
        .collect::<Vec<_>>();
    // Reserve the one large contiguous Cord array before smaller descriptions
    // fragment the same finite arena. Retiring inputs transfer without cloning.
    let connection_storage = retiring.then(|| Vec::with_capacity(plot.connections.len()));
    let mut consumed_protected_handles = BTreeSet::new();
    let mut resource_writers = BTreeSet::new();
    let mut planned_gears = Vec::<PlannedGear>::with_capacity(plot.gears.len());
    let mut placement_lookup = BTreeMap::<GearId, PlacementId>::new();

    for gear_index in 0..plot.gears.len() {
        let gear = plot.gears.get(gear_index)?;
        let choice = placements
            .by_gear
            .get(&gear.gear_id)
            .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().to_string()))?;
        let host = hosts
            .iter()
            .rev()
            .find(|host| host.host_id == choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let selected_offer = hosts.capability(&choice.host_id, &choice.capability_id)?;
        let capability = selected_offer.as_ref();
        validate_operation_capability(&gear, capability)?;
        validate_keep_retention(&gear, capability)?;

        let count = placement_count
            .entry((host.host_id.clone(), capability.capability_id.clone()))
            .or_insert(0);
        *count += 1;
        validate_instance_count_ref(capability.into(), *count)?;

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

        let authority_bindings = authority::bind(host, capability, authority_grants)?;

        let placement_id = placement_identity_ref(
            plot.checked_plot_id.as_str(),
            gear.gear_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str(),
        );
        placement_lookup.insert(gear.gear_id.clone(), placement_id.clone());
        let scope = (
            host.host_id.clone(),
            host.boot_id.clone(),
            host.offer_generation,
        );
        let capability_id = capability.capability_id.clone();
        let limits = capability.limits.clone();
        let last_use = retiring
            && usize::from(*count)
                == placements
                    .by_gear
                    .values()
                    .filter(|choice| {
                        choice.host_id == scope.0 && choice.capability_id == capability_id
                    })
                    .count();
        let decoded = match selected_offer {
            alloc::borrow::Cow::Owned(offer) => Some(offer),
            alloc::borrow::Cow::Borrowed(_) => None,
        };
        let description = if let Some(offer) = decoded {
            hosts::OfferDescription::owned(offer)
        } else {
            hosts.description(&scope.0, &capability_id, last_use)
        };
        let mut planned = conduit_core::planned_gear_from_parts! {
            realization_properties: description.realization_properties,
            placement_id,
            gear_id: gear.gear_id.clone(),
            kind_id: description.kind_id,
            kind_contract_revision: description.kind_contract_revision,
            execution_profile_id: description.execution_profile_id,
            configuration: Vec::new(),
            host_id: scope.0,
            boot_id: scope.1,
            offer_generation: scope.2,
            capability_id,
            implementation_id: description.implementation_id,
            artifact_id: description.artifact_id,
            base,
            realization_characteristics: Vec::new(),
            limits,
            inputs: description.inputs,
            outputs: description.outputs,
            semantic_contract: description.semantic_contract,
            terminal_transductions: gear.terminal_transductions.clone(),
            host_calls: description.host_calls,
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
        retention.placement(gear_index, &mut planned);
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

    // Placement admission is complete. These ledgers are not Plan material and
    // need not coexist with Cord admission or fragment construction.
    // Placements own the exact admitted limits and descriptions. Cord planning
    // reads those same limits; the retiring catalogue need not remain live.
    drop(hosts);

    drop((
        placement_count,
        resource_usage,
        remaining_compute_minimum,
        consumed_protected_handles,
        resource_writers,
    ));

    let mut planned_connections = connections::plan(
        &plot,
        &planned_gears,
        &placement_lookup,
        bases,
        options,
        connection_limits,
        connection_storage,
    )?;

    // Indexed input retained only the checked Front/resource facts needed by
    // Cord admission. Retire them and the lookup before startup/Sign material.
    drop((plot.gears, plot.connections, placement_lookup));

    let mut global_startup_order = startup::startup_order(&planned_gears, &planned_connections)?
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(plot.name.into()))?;

    // A placement belongs to exactly one Host. Move its retained program into
    // that fragment instead of copying every program while both lists are live.
    let mut placements_by_host = BTreeMap::<HostId, Vec<PlannedGear>>::new();
    let sole_host = planned_gears
        .first()
        .map(|p| p.host_id.clone())
        .filter(|host| planned_gears.iter().all(|p| p.host_id == *host));
    let single_host = sole_host.is_some();
    if let Some(host) = sole_host {
        // Preserve the admitted placement allocation as the fragment's own
        // storage. Rebuilding the same array doubles its live footprint.
        placements_by_host.insert(host, planned_gears);
    } else {
        for placement in planned_gears {
            placements_by_host
                .entry(placement.host_id.clone())
                .or_default()
                .push(placement);
        }
    }
    let mut fragments = Vec::with_capacity(placements_by_host.len());
    for host in fragment_hosts {
        let placements = placements_by_host.remove(&host.host_id).unwrap_or_default();
        if placements.is_empty() {
            continue;
        }
        let connections = if single_host {
            // Every Cord belongs to this fragment. Keep its original large
            // allocation instead of cloning the same admitted graph.
            core::mem::take(&mut planned_connections)
        } else {
            planned_connections
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
                .collect::<Vec<_>>()
        };
        let startup_order = if single_host {
            core::mem::take(&mut global_startup_order)
        } else {
            global_startup_order
                .iter()
                .filter(|placement_id| {
                    placements
                        .iter()
                        .any(|placement| &placement.placement_id == *placement_id)
                })
                .cloned()
                .collect()
        };
        let startup_dependencies = startup::startup_dependencies(&placements, &connections)?;
        let expected_terminals = placements
            .iter()
            .map(|placement| ExpectedTerminal::PlacementCompleted(placement.placement_id.clone()))
            .chain(connections.iter().map(|connection| {
                ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
            }))
            .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
            .collect();
        // Keep only actual state boundaries. A Vec<Option<large State>>
        // allocates one full slot even for every stateless placement.
        let mut states = Vec::new();
        for placement in &placements {
            if let Some(state) = retention.state(placement)? {
                states.push(state);
            }
        }
        let expected_sign =
            core::iter::once(ExpectedSign::PlanFragmentReceived)
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
            .ok_or_else(|| PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string()))?;
        let state_budget = conduit_core::state_resource_budget(&states).map_err(|error| {
            PlannerError::InvalidStateContract(format!(
                "host '{}' retained State admission: {error:?}",
                host.host_id.as_str()
            ))
        })?;
        sign_storage_budget.item_capacity = sign_storage_budget
            .item_capacity
            .checked_add(state_budget.sign_storage.item_capacity)
            .ok_or_else(|| PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string()))?;
        sign_storage_budget.byte_capacity = sign_storage_budget
            .byte_capacity
            .checked_add(state_budget.sign_storage.byte_capacity)
            .ok_or_else(|| PlannerError::SignBudgetOverflow(host.host_id.as_str().to_string()))?;
        fragments.push(PlanFragment {
            plan_id: PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: host.identity.source_document_id,
            checked_plot_id: host.identity.checked_plot_id,
            expanded_plot_id: host.identity.expanded_plot_id,
            completion_policy: plan_completion_policy(plot.completion),
            realization_backs: Vec::new(),
            host_id: host.host_id,
            boot_id: host.boot_id,
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
        });
    }

    Ok(fragments)
}
