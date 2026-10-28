//! Ordinary single-Host planning into borrowed records. No effect authority.
use crate::*;
use conduit_core as c;
use conduit_plot::LinkedPlanningFactsRef;

#[derive(Debug)]
pub enum BorrowedLocalPlanningRefusal {
    UnsupportedProfile,
    WorkspaceCapacity,
    Planning(PlannerError),
    Verification(c::FlatPlanRefusal),
}
impl From<PlannerError> for BorrowedLocalPlanningRefusal {
    fn from(e: PlannerError) -> Self {
        Self::Planning(e)
    }
}
/// Source-derived ordinary choices plus Core structural verification. This is
/// not readiness, Root possession, Body membership, or permission to execute.
/// Construction is private; no alleged-ID or digest constructor is exposed.
pub struct SourcePlannedLocalScope<'a> {
    source: &'a conduit_plot::flat_image::indexed::IndexedSourceImage<'a>,
    verified: c::VerifiedFlatPlanRef<'a>,
}
impl<'a> SourcePlannedLocalScope<'a> {
    pub fn verified(&self) -> &c::VerifiedFlatPlanRef<'a> {
        &self.verified
    }
    pub fn source(&self) -> &conduit_plot::flat_image::indexed::IndexedSourceImage<'a> {
        self.source
    }
}

#[allow(clippy::too_many_arguments)]
pub fn with_linked_local_plan<R>(
    image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
    host: c::HostAdvertisementRef<'_>,
    bases: &[c::BaseImplementationId],
    grants: c::AuthorityGrantRefs<'_>,
    item_capacity: u16,
    byte_capacity: u32,
    consume: impl for<'a> FnOnce(&SourcePlannedLocalScope<'a>) -> R,
) -> Result<R, BorrowedLocalPlanningRefusal> {
    use BorrowedLocalPlanningRefusal as E;
    let facts = image
        .linked_planning_facts()
        .map_err(|_| E::UnsupportedProfile)?;
    // No meaningful field may be projected away. Rich entrances remain available
    // for configuration, realizations, retained State, resources and remote Lines.
    if facts.gears.is_empty() || !plain_linked_profile(facts, host) {
        return Err(E::UnsupportedProfile);
    }
    // Selection precedes ordinary options/Host/grant validation, just as the rich
    // default-placements + compact-planning entrance does.
    let choices = plain_default_choices(facts, host)?;
    if item_capacity == 0 || byte_capacity == 0 {
        return Err(PlannerError::InvalidConnectionBudget(
            "item and byte capacity must both be nonzero".into(),
        )
        .into());
    }
    validate_host_resources_ref(host)?;
    validate_authority_grants_ref(grants)?;
    let mut usage = vec![0u32; host.resources.len()];
    let mut counts: Vec<(&str, u16)> = Vec::new();
    let mut resources: Vec<Vec<c::ResourceBindingRef<'_>>> = Vec::with_capacity(facts.gears.len());
    let mut authority: Vec<Vec<c::AuthorityBindingRef<'_>>> = Vec::with_capacity(facts.gears.len());
    let mut selected_bases = Vec::with_capacity(facts.gears.len());
    let mut ids = Vec::with_capacity(facts.gears.len());
    for (gear, choice) in facts.gears.iter().zip(&choices) {
        let offer = host
            .capabilities
            .iter()
            .nth(*choice)
            .ok_or_else(|| PlannerError::UnknownCapability(gear.kind_id.into()))?;
        validate_operation_capability_ref(*gear, offer)?;
        validate_keep_retention_ref(*gear, offer)?;
        let count = if let Some((_, count)) =
            counts.iter_mut().find(|(id, _)| *id == offer.capability_id)
        {
            count
        } else {
            counts.push((offer.capability_id, 0));
            &mut counts.last_mut().unwrap().1
        };
        *count += 1;
        validate_instance_count_ref(offer, *count)?;
        let mut bound = Vec::with_capacity(offer.resource_requirements.len());
        for requirement in offer.resource_requirements.iter() {
            let index = resource_binding::pool_selection::resolve_resource_pool_index(
                host,
                offer,
                requirement.class_id,
            )?;
            let pool =
                host.resources.iter().nth(index).ok_or_else(|| {
                    PlannerError::UnavailableResource(requirement.class_id.into())
                })?;
            resource_binding::consume_units_ref(
                pool.pool_id,
                pool.capacity_units,
                &mut usage[index],
                requirement.units,
            )?;
            bound.push(c::ResourceBindingRef {
                pool_id: pool.pool_id,
                class_id: pool.class_id,
                units: requirement.units,
                protected: None,
                compute: None,
                content: None,
            });
        }
        bound.sort_by(|a, b| {
            a.pool_id
                .cmp(b.pool_id)
                .then(a.class_id.cmp(b.class_id))
                .then(a.units.cmp(&b.units))
        });
        selected_bases.push(selected_base_provider_index_ref(host, offer)?);
        resources.push(bound);
        let mut bound = Vec::with_capacity(offer.authority_requirements.len());
        for requirement in offer.authority_requirements.iter() {
            let index = resolve_authority_grant_index(host, offer, requirement, grants)?;
            bound.push(
                grants
                    .iter()
                    .nth(index)
                    .ok_or_else(|| {
                        PlannerError::InvalidAuthorityContract("selected grant disappeared".into())
                    })?
                    .into(),
            );
        }
        bound.sort_by(|a, b| compare_authority_bindings_ref(*a, *b));
        authority.push(bound);
        ids.push(placement_identity_ref(
            facts.checked_plot_id,
            gear.gear_id,
            host.host_id,
            offer.capability_id,
        ));
    }
    // This complete temporary placement table is the largest single allocation
    // on the native network preparation path. Aggregate free bytes do not prove
    // that a fragmented Host can back it. Refuse before invoking the consumer
    // (and therefore before Root issuance), rather than aborting local Play.
    let mut records = Vec::new();
    records
        .try_reserve_exact(facts.gears.len())
        .map_err(|_| E::WorkspaceCapacity)?;
    for (i, gear) in facts.gears.iter().enumerate() {
        let offer = host.capabilities.iter().nth(choices[i]).unwrap();
        let base = selected_bases[i].map(|index| host.bases.iter().nth(index).unwrap());
        records.push(c::PlannedGearRef {
            placement_id: ids[i].as_str(),
            gear_id: gear.gear_id,
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.execution_profile_id,
            host_id: host.host_id,
            boot_id: host.boot_id,
            capability_id: offer.capability_id,
            implementation_id: offer.implementation_id,
            artifact_id: offer.artifact_id,
            source_span: None,
            offer_generation: host.offer_generation,
            configuration: gear.configuration,
            base: base.map(|b| c::BaseProviderBindingRef {
                base_id: b.base_id,
                provider_instance_id: b.provider_instance_id,
                provider_generation: b.provider_generation,
                implementation_id: b.implementation_id,
                mechanism_family: b.mechanism_family,
                enforcement_class: b.enforcement_class,
            }),
            realization_characteristics: &[],
            realization_properties: offer.realization_properties,
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            semantic_contract: offer.semantic_contract,
            terminal_transductions: gear.terminal_transductions,
            host_calls: offer.host_calls,
            resources: c::ResourceBindingRefs::Borrowed(&resources[i]),
            authority: c::AuthorityBindingRefs::Borrowed(&authority[i]),
            pool_references: gear.pool_references,
        });
    }
    let connection_ids = facts
        .connections
        .iter()
        .map(|v| connection_identity_ref(facts.checked_plot_id, *v))
        .collect::<Vec<_>>();
    let mut connections = Vec::with_capacity(facts.connections.len());
    for (i, connection) in facts.connections.iter().enumerate() {
        let source_index = facts
            .gears
            .iter()
            .position(|g| g.gear_id == connection.source_gear_id)
            .ok_or_else(|| PlannerError::UnknownGear(connection.source_gear_id.into()))?;
        let sink_index = facts
            .gears
            .iter()
            .position(|g| g.gear_id == connection.sink_gear_id)
            .ok_or_else(|| PlannerError::UnknownGear(connection.sink_gear_id.into()))?;
        let source = records[source_index];
        let sink = records[sink_index];
        validate_plain_local_cord_ref(
            *connection,
            source,
            sink,
            bases,
            item_capacity,
            byte_capacity,
        )?;
        connections.push(c::PlannedConnectionRef {
            connection_id: connection_ids[i].as_str(),
            source_placement_id: source.placement_id,
            source_port_id: connection.source_port_id,
            sink_placement_id: sink.placement_id,
            sink_port_id: connection.sink_port_id,
            value_kind: connection.value_kind,
            resource: None,
            abnormal_kind: source
                .outputs
                .iter()
                .find(|p| p.port_id == connection.source_port_id)
                .and_then(|p| p.abnormal_kind),
            track: connection.track,
            temporal: connection.temporal,
            pressure_policy: if source.kind_id == "flow/coalesce-latest" {
                c::DeliveryPressurePolicy::CoalesceLatest
            } else {
                c::DeliveryPressurePolicy::PreserveOrder
            },
            selected_line: None,
            admitted_lines: &[],
            item_capacity,
            byte_capacity,
        });
    }
    let startup_order = startup::plain_startup_order_ref(&records, &connections)?
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(facts.name.into()))?;
    let dependencies = startup::plain_startup_dependencies_ref(&connections);
    let terminals = records
        .iter()
        .map(|p| c::ExpectedTerminalRef::PlacementCompleted(p.placement_id))
        .chain(
            connections
                .iter()
                .map(|v| c::ExpectedTerminalRef::ConnectionCompleted(v.connection_id)),
        )
        .chain(core::iter::once(c::ExpectedTerminalRef::PlanCompleted))
        .collect::<Vec<_>>();
    let signs = core::iter::once(c::ExpectedSignRef::PlanFragmentReceived)
        .chain(
            records
                .iter()
                .map(|p| c::ExpectedSignRef::PlacementPrepared(p.placement_id)),
        )
        .chain(
            records
                .iter()
                .map(|p| c::ExpectedSignRef::PlacementTerminal(p.placement_id)),
        )
        .chain(
            connections
                .iter()
                .map(|p| c::ExpectedSignRef::ConnectionTerminal(p.connection_id)),
        )
        .chain(core::iter::once(c::ExpectedSignRef::PlanTerminal))
        .collect::<Vec<_>>();
    let sign_budget =
        c::mandatory_sign_storage_requirement_ref(c::ExpectedSignRefs::Borrowed(&signs))
            .ok_or_else(|| PlannerError::SignBudgetOverflow(host.host_id.into()))?;
    for record in &mut records {
        record.source_span = Some(crate::canonical::checked_source_span_ref(
            record.gear_id,
            facts
                .provenance
                .iter()
                .find(|p| p.gear_id == record.gear_id)
                .map(|p| p.source_span),
        )?);
    }
    let completion = crate::plan_completion_policy(facts.completion);
    let header = c::FragmentFingerprintHeaderRef {
        source_document_id: facts.source_document_id,
        checked_plot_id: facts.checked_plot_id,
        expanded_plot_id: facts.expanded_plot_id,
        completion_policy: completion,
        realization_backs: &[],
        host_id: host.host_id,
        boot_id: host.boot_id,
        offer_generation: host.offer_generation,
        execution_regions: &[],
        execution_fusions: &[],
    };
    let tail = c::FragmentFingerprintTailRef {
        connections: c::PlannedConnectionRefs::Borrowed(&connections),
        fore_ports: &[],
        shared_pools: &[],
        startup_dependencies: c::StartupDependencyRefs::Borrowed(&dependencies),
        startup_order: c::PlacementIdRefs::Borrowed(&startup_order),
        cancellation_policy: c::CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: c::TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: c::ExpectedTerminalRefs::Borrowed(&terminals),
        expected_sign: c::ExpectedSignRefs::Borrowed(&signs),
        sign_storage_budget: &sign_budget,
        states: &[],
    };
    let mut fingerprint = c::FragmentFingerprintBuilder::new_ref(header, records.len());
    for record in &records {
        fingerprint
            .push_placement_ref(*record)
            .map_err(|_| E::UnsupportedProfile)?;
    }
    let fragment = fingerprint
        .finish_ref(tail)
        .map_err(|_| E::UnsupportedProfile)?;
    let commitments = c::FragmentCommitmentRefs::Single(c::FragmentCommitmentRef {
        host_id: host.host_id,
        fragment_id: fragment.as_str(),
    });
    let root = c::PlanFingerprintInputRef {
        source_document_id: facts.source_document_id,
        checked_plot_id: facts.checked_plot_id,
        expanded_plot_id: facts.expanded_plot_id,
        realization_backs: &[],
        activations: &[],
        commitments,
        activation_preparations: &[],
    };
    let plan = c::compute_plan_fingerprint_ref(root);
    let verified = c::FlatPlanRef {
        plan_id: plan.as_str(),
        root,
        completion_policy: completion,
        fragment_plan_id: plan.as_str(),
        fragment_id: fragment.as_str(),
        header,
        placements: c::PlannedGearRefs::Borrowed(&records),
        tail,
        fragment_commitments: commitments,
    }
    .verify()
    .map_err(E::Verification)?;
    Ok(consume(&SourcePlannedLocalScope {
        source: image,
        verified,
    }))
}

fn plain_linked_profile(
    facts: LinkedPlanningFactsRef<'_>,
    host: c::HostAdvertisementRef<'_>,
) -> bool {
    facts.gears.iter().all(|g| {
        g.startup_parameters.is_empty()
            && g.semantic_contract.configuration.is_empty()
            && g.semantic_contract.laws.is_empty()
            && g.terminal_transductions.is_empty()
            && g.resource_ports.is_empty()
            && g.configuration.is_empty()
            && g.pool_references.is_empty()
    }) && host.capabilities.iter().all(|o| {
        o.realization_properties.is_empty()
            && o.resource_requirements
                .iter()
                .all(|r| r.compute.is_none() && r.content.is_none() && r.protected_role.is_none())
    }) && host
        .resources
        .iter()
        .all(|r| r.compute.is_none() && r.content.is_none())
}
fn plain_default_choices(
    facts: LinkedPlanningFactsRef<'_>,
    host: c::HostAdvertisementRef<'_>,
) -> Result<Vec<usize>, PlannerError> {
    let mut choices = Vec::with_capacity(facts.gears.len());
    for gear in facts.gears {
        let mut best: Option<(usize, c::CapabilityOfferRef<'_>)> = None;
        for (index, offer) in host.capabilities.iter().enumerate() {
            // This calls the same callable/semantic/revision predicate as the rich path.
            if !gear.accepts_realization_ref(offer) {
                continue;
            }
            let used = choices
                .iter()
                .filter(|previous| {
                    host.capabilities
                        .iter()
                        .nth(**previous)
                        .is_some_and(|p| p.capability_id == offer.capability_id)
                })
                .count();
            if used >= usize::from(offer.limits.max_active_instances) {
                continue;
            }
            if best.is_none_or(|(_, prior)| {
                crate::functional_compatibility::compare_default_candidates_ref(
                    *gear, 0, offer, 0, prior,
                )
                .is_lt()
            }) {
                best = Some((index, offer));
            }
        }
        let (_, offer) =
            best.ok_or_else(|| PlannerError::UnknownCapability(gear.kind_id.into()))?;
        // Ordinary default choices retain an ID, and placement admission then
        // resolves its first advertised occurrence. Preserve that behavior even
        // for duplicate alleged capability IDs with contradictory descriptions.
        let index = host
            .capabilities
            .iter()
            .position(|candidate| candidate.capability_id == offer.capability_id)
            .expect("selected advertised capability remains immutable");
        choices.push(index);
    }
    Ok(choices)
}

// Shared ordinary predicate extractions. Rich callers delegate without changing
// ownership transfer or their BTreeSet/BTreeMap orchestration.

fn validate_authority_grants_ref(
    grants: conduit_core::AuthorityGrantRefs<'_>,
) -> Result<(), PlannerError> {
    if grants.iter().any(invalid_grant_scope_ref) {
        return Err(PlannerError::InvalidAuthorityContract(
            "grants must have non-empty immutable scope identities".into(),
        ));
    }
    if grants.iter().enumerate().any(|(index, g)| {
        grants
            .iter()
            .take(index)
            .any(|previous| previous.grant_id == g.grant_id)
    }) {
        return Err(PlannerError::InvalidAuthorityContract(
            "grant identities must be unique".into(),
        ));
    }
    Ok(())
}

// Shared decision only: actual Root key/table possession remains independent.

fn validate_plain_local_cord_ref(
    _connection: conduit_plot::CheckedConnectionRef<'_>,
    source: c::PlannedGearRef<'_>,
    sink: c::PlannedGearRef<'_>,
    bases: &[c::BaseImplementationId],
    items: u16,
    bytes: u32,
) -> Result<(), PlannerError> {
    if !crate::contract::LineMechanismPolicy::new(bases).permits_local() {
        return Err(PlannerError::UnavailableBaseImplementationId(format!(
            "local base unavailable for '{}' > '{}'",
            source.gear_id, sink.gear_id
        )));
    }
    crate::validate_queue_limits_ref(source, sink, items, bytes)
}
