use crate::prelude::*;
use crate::{
    default_placements_unvalidated, plan_validated_form,
    plan_validated_form_with_connection_limits, ConnectionEndpoints, ConnectionQueueLimits,
    PlacementChoices, PlannerError, PlanningOptions,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    AdmittedLine, AuthorityGrant, BaseImplementationId, FormIdentity, HostAdvertisement,
    LineAvailability, Plan, PlannedForePort, PlannedGear, PlannedSharedPool, PoolMemberLimits,
    PoolRealizationEnvelope, ResourceBinding, SharedPoolId, SharedPoolSelectionPolicy,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_CALL_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    expand_canonical_form, expand_canonical_form_with_backs, CanonicalBackCatalog, CheckedForm,
    CheckedSyntaxDocument, ExpandedAuthoringForm, ExpandedCanonicalForm, ProfileCatalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeBoundaryKey {
    pub direction: conduit_core::PortDirection,
    pub front_port_id: conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
}

impl PartialOrd for ForeBoundaryKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ForeBoundaryKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.direction as u8, &self.front_port_id, self.track).cmp(&(
            other.direction as u8,
            &other.front_port_id,
            other.track,
        ))
    }
}

/// Plans an open ordinary Form and seals every external Fore binding into the
/// immutable Plan. Every binding needs an exact finite queue budget.
pub fn plan_expanded_authoring_with_options(
    form: &ExpandedAuthoringForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    boundary_limits: &BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let mut plan =
        plan_expanded_canonical_with_options(&form.expanded, hosts, placements, bases, options)?;
    let expected = form
        .input_bindings
        .iter()
        .map(|binding| ForeBoundaryKey {
            direction: conduit_core::PortDirection::Input,
            front_port_id: binding.front_port_id.clone(),
            track: binding.track,
        })
        .chain(form.output_bindings.iter().map(|binding| ForeBoundaryKey {
            direction: conduit_core::PortDirection::Output,
            front_port_id: binding.front_port_id.clone(),
            track: binding.track,
        }))
        .collect::<BTreeSet<_>>()
        .len();
    if expected != boundary_limits.len() {
        return Err(PlannerError::InvalidFormIdentity(
            "every external Fore binding requires one exact queue limit".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for (direction, bindings, descriptors) in [
        (
            conduit_core::PortDirection::Input,
            form.input_bindings.as_slice(),
            form.front.inputs(),
        ),
        (
            conduit_core::PortDirection::Output,
            form.output_bindings.as_slice(),
            form.front.outputs(),
        ),
    ] {
        for binding in bindings {
            let key = ForeBoundaryKey {
                direction,
                front_port_id: binding.front_port_id.clone(),
                track: binding.track,
            };
            if !seen.insert(key.clone())
                && (direction != conduit_core::PortDirection::Input
                    || binding.track != conduit_core::ConnectionTrack::Payload)
            {
                return Err(PlannerError::InvalidFormIdentity(format!(
                    "external Fore port '{}' track '{}' has unsupported multiple internal bindings",
                    binding.front_port_id.as_str(),
                    binding.track.as_str(),
                )));
            }
            let limits = boundary_limits.get(&key).ok_or_else(|| {
                PlannerError::InvalidFormIdentity(format!(
                    "external Fore port '{}' track '{}' has no queue limit",
                    binding.front_port_id.as_str(),
                    binding.track.as_str(),
                ))
            })?;
            if limits.item_capacity == 0 || limits.byte_capacity == 0 {
                return Err(PlannerError::InvalidFormIdentity(format!(
                    "external Fore port '{}' has a zero queue limit",
                    binding.front_port_id.as_str(),
                )));
            }
            let descriptor = descriptors
                .iter()
                .find(|port| port.port_id == binding.front_port_id)
                .ok_or_else(|| {
                    PlannerError::InvalidFormIdentity("external Fore descriptor is missing".into())
                })?;
            let value_contract = (!matches!(
                binding.track,
                conduit_core::ConnectionTrack::NormalClose
                    | conduit_core::ConnectionTrack::Quiescence
            ))
            .then(|| {
                let location = match direction {
                    conduit_core::PortDirection::Input => {
                        conduit_core::FrontValueLocation::Input(binding.front_port_id.clone())
                    }
                    conduit_core::PortDirection::Output => {
                        conduit_core::FrontValueLocation::Output(binding.front_port_id.clone())
                    }
                };
                form.front
                    .value_contracts()
                    .iter()
                    .find(|contract| contract.location == location)
                    .map(|contract| contract.contract.clone())
            })
            .flatten();
            let fragment = plan
                .fragments
                .iter_mut()
                .find(|fragment| {
                    fragment
                        .placements
                        .iter()
                        .any(|placement| placement.gear_id == binding.gear_id)
                })
                .ok_or_else(|| {
                    PlannerError::InvalidFormIdentity("external Fore placement is missing".into())
                })?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.gear_id == binding.gear_id)
                .expect("fragment selected by this placement");
            let internal = match direction {
                conduit_core::PortDirection::Input => &placement.inputs,
                conduit_core::PortDirection::Output => &placement.outputs,
            }
            .iter()
            .find(|port| port.port_id == binding.gear_port_id)
            .ok_or_else(|| {
                PlannerError::InvalidFormIdentity("external Fore internal port is missing".into())
            })?;
            let internal_matches = match binding.track {
                conduit_core::ConnectionTrack::Payload => {
                    internal.value_kind == descriptor.value_kind
                        && internal.temporal == descriptor.temporal
                        && internal.abnormal_kind == descriptor.abnormal_kind
                }
                conduit_core::ConnectionTrack::AbnormalTerminal => {
                    internal.abnormal_kind.as_ref() == Some(&descriptor.value_kind)
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::NormalClose => {
                    matches!(
                        internal.temporal,
                        conduit_core::PortTemporal::Flow { closes: true }
                    ) && descriptor.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::Quiescence => {
                    matches!(internal.temporal, conduit_core::PortTemporal::Flow { .. })
                        && descriptor.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && descriptor.temporal == conduit_core::PortTemporal::Value
                }
            };
            if !internal_matches {
                return Err(PlannerError::InvalidFormIdentity(format!(
                    "external Fore port '{}' does not match its selected Back",
                    binding.front_port_id.as_str(),
                )));
            }
            if !matches!(
                binding.track,
                conduit_core::ConnectionTrack::NormalClose
                    | conduit_core::ConnectionTrack::Quiescence
            ) {
                let internal_front = placement.checked_port_front();
                let internal_location = match (direction, binding.track) {
                    (
                        conduit_core::PortDirection::Input,
                        conduit_core::ConnectionTrack::AbnormalTerminal,
                    ) => conduit_core::FrontValueLocation::InputAbnormal(
                        binding.gear_port_id.clone(),
                    ),
                    (
                        conduit_core::PortDirection::Output,
                        conduit_core::ConnectionTrack::AbnormalTerminal,
                    ) => conduit_core::FrontValueLocation::OutputAbnormal(
                        binding.gear_port_id.clone(),
                    ),
                    (conduit_core::PortDirection::Input, _) => {
                        conduit_core::FrontValueLocation::Input(binding.gear_port_id.clone())
                    }
                    (conduit_core::PortDirection::Output, _) => {
                        conduit_core::FrontValueLocation::Output(binding.gear_port_id.clone())
                    }
                };
                let internal_contract = internal_front
                    .value_contracts()
                    .iter()
                    .find(|contract| contract.location == internal_location)
                    .map(|contract| contract.contract.clone());
                if internal_contract != value_contract {
                    return Err(PlannerError::InvalidFormIdentity(format!(
                        "external Fore port '{}' value contract does not match its selected Back",
                        binding.front_port_id.as_str(),
                    )));
                }
            }
            if limits.item_capacity > placement.limits.max_queue_items
                || limits.byte_capacity > placement.limits.max_queue_bytes
            {
                return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                    "external Fore port '{}' exceeds selected Back queue limits",
                    binding.front_port_id.as_str(),
                )));
            }
            fragment.fore_ports.push(PlannedForePort {
                front_port_id: binding.front_port_id.clone(),
                direction,
                placement_id: placement.placement_id.clone(),
                gear_port_id: binding.gear_port_id.clone(),
                value_kind: descriptor.value_kind.clone(),
                value_contract,
                abnormal_kind: descriptor.abnormal_kind.clone(),
                track: binding.track,
                temporal: descriptor.temporal,
                pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
                item_capacity: limits.item_capacity,
                byte_capacity: limits.byte_capacity,
            });
        }
    }
    for fragment in &mut plan.fragments {
        fragment.fore_ports.sort_by(|left, right| {
            (left.direction as u8, &left.front_port_id, left.track).cmp(&(
                right.direction as u8,
                &right.front_port_id,
                right.track,
            ))
        });
    }
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            FormIdentity {
                source_document_id: plan.source_document_id,
                checked_form_id: plan.checked_form_id,
                expanded_form_id: plan.expanded_form_id,
            },
            plan.completion_policy,
            plan.realization_backs,
            plan.fragments,
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalRealizationMode {
    Direct,
    RecursiveBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCanonicalRealization {
    pub mode: CanonicalRealizationMode,
    pub expanded: ExpandedCanonicalForm,
    pub placements: PlacementChoices,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRealizationSelectionError {
    InvalidDirectExpansion(String),
    InvalidRecursiveExpansion {
        direct: PlannerError,
        recursive: String,
    },
    NoRealizablePath {
        direct: PlannerError,
        recursive: PlannerError,
    },
}

/// Plans between peer realizations of one checked caller without changing its
/// source. A fully admitted direct Plan wins; otherwise the same checked
/// caller is expanded through the admitted exact Back catalog and planned
/// normally. Mere capability availability cannot suppress a valid Back.
#[allow(clippy::too_many_arguments)]
pub fn plan_canonical_realization_with_options(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<PlannedCanonicalRealization, CanonicalRealizationSelectionError> {
    let direct = expand_canonical_form(document, form_name, catalog).map_err(|error| {
        CanonicalRealizationSelectionError::InvalidDirectExpansion(error.to_string())
    })?;
    match plan_default_candidate(direct, hosts, bases, options) {
        Ok((expanded, placements, plan)) => Ok(PlannedCanonicalRealization {
            mode: CanonicalRealizationMode::Direct,
            expanded,
            placements,
            plan,
        }),
        Err(direct_error) => {
            let recursive = expand_canonical_form_with_backs(document, form_name, catalog, backs)
                .map_err(|error| {
                CanonicalRealizationSelectionError::InvalidRecursiveExpansion {
                    direct: direct_error.clone(),
                    recursive: error.to_string(),
                }
            })?;
            let (expanded, placements, plan) =
                plan_default_candidate(recursive, hosts, bases, options).map_err(
                    |recursive_error| CanonicalRealizationSelectionError::NoRealizablePath {
                        direct: direct_error,
                        recursive: recursive_error,
                    },
                )?;
            Ok(PlannedCanonicalRealization {
                mode: CanonicalRealizationMode::RecursiveBack,
                expanded,
                placements,
                plan,
            })
        }
    }
}

fn plan_default_candidate(
    expanded: ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<(ExpandedCanonicalForm, PlacementChoices, Plan), PlannerError> {
    let placements = default_expanded_placements(&expanded, hosts)?;
    let plan = plan_expanded_canonical_with_options(&expanded, hosts, &placements, bases, options)?;
    Ok((expanded, placements, plan))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPoolPlanningRequirement {
    pub member_limits: PoolMemberLimits,
    pub admission_authority: AuthorityGrant,
    pub member_sessions_required: bool,
}

pub fn default_expanded_placements(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    default_placements_unvalidated(&form.gears, hosts)
}

mod default_queues;
pub use default_queues::plan_expanded_canonical;

pub fn plan_expanded_canonical_with_options(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let planning_form = CheckedForm {
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        name: form.name.clone(),
        completion: form.completion,
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let mut plan = plan_validated_form(&planning_form, hosts, placements, bases, options)?;
    attach_source_spans(form, &mut plan)?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::FormIdentity {
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
            },
            crate::plan_completion_policy(form.completion),
            form.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

pub fn plan_expanded_canonical_with_connection_limits(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let planning_form = CheckedForm {
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        name: form.name.clone(),
        completion: form.completion,
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let mut plan = plan_validated_form_with_connection_limits(
        &planning_form,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    attach_source_spans(form, &mut plan)?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::FormIdentity {
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
            },
            crate::plan_completion_policy(form.completion),
            form.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

fn attach_source_spans(form: &ExpandedCanonicalForm, plan: &mut Plan) -> Result<(), PlannerError> {
    for placement in plan
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
    {
        let provenance = form
            .provenance
            .iter()
            .find(|entry| entry.gear_id == placement.gear_id.as_str())
            .ok_or_else(|| {
                PlannerError::InvalidFormIdentity(format!(
                    "expanded Gear '{}' has no exact source provenance",
                    placement.gear_id.as_str()
                ))
            })?;
        let span = provenance.source_span;
        let span = conduit_core::SourceSpan {
            start: span.start.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span start exceeds u64".into())
            })?,
            end: span.end.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span end exceeds u64".into())
            })?,
            line: span.line.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span line exceeds u64".into())
            })?,
            column: span.column.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span column exceeds u64".into())
            })?,
            end_line: span.end_line.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span end line exceeds u64".into())
            })?,
            end_column: span.end_column.try_into().map_err(|_| {
                PlannerError::InvalidFormIdentity("source span end column exceeds u64".into())
            })?,
        };
        if !span.is_valid() {
            return Err(PlannerError::InvalidFormIdentity(
                "expanded Gear source span is invalid".into(),
            ));
        }
        placement.source_span = Some(span);
    }
    Ok(())
}

pub fn plan_expanded_canonical_with_shared_pools(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirements: &BTreeMap<SharedPoolId, SharedPoolPlanningRequirement>,
) -> Result<Plan, PlannerError> {
    let mut plan = plan_expanded_canonical_with_options(form, hosts, placements, bases, options)?;
    if form.shared_pools.len() != requirements.len() {
        return Err(PlannerError::InvalidSharedPool(
            "every expanded shared pool requires one exact planning requirement".into(),
        ));
    }

    let mut remaining_resources = hosts
        .iter()
        .flat_map(|host| {
            host.resources.iter().map(|resource| {
                (
                    (host.host_id.clone(), resource.pool_id.clone()),
                    resource.capacity_units,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for placement in plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
    {
        for resource in &placement.resources {
            let available = remaining_resources
                .get_mut(&(placement.host_id.clone(), resource.pool_id.clone()))
                .ok_or_else(|| {
                    PlannerError::InvalidSharedPool(format!(
                        "planned resource '{}' is absent from pool accounting",
                        resource.pool_id.as_str()
                    ))
                })?;
            *available = available.checked_sub(resource.units).ok_or_else(|| {
                PlannerError::InvalidSharedPool(format!(
                    "planned resource '{}' exceeds its advertised capacity",
                    resource.pool_id.as_str()
                ))
            })?;
        }
    }

    let placement_lookup = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| (placement.gear_id.clone(), placement.placement_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let planned_gears = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    let mut planned_pools = Vec::with_capacity(form.shared_pools.len());
    for pool in &form.shared_pools {
        let requirement = requirements.get(&pool.pool_id).ok_or_else(|| {
            PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' has no planning requirement",
                pool.pool_id.as_str()
            ))
        })?;
        validate_pool_authority(&requirement.admission_authority, hosts)?;
        if !requirement.member_limits.is_finite_and_nonzero() {
            return Err(PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' has invalid per-member bounds",
                pool.pool_id.as_str()
            )));
        }
        let mut candidates = hosts
            .iter()
            .flat_map(|host| {
                host.capabilities
                    .iter()
                    .filter(|capability| capability.checked_front() == pool.member_front)
                    .map(move |capability| (host, capability))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|(left_host, left), (right_host, right)| {
            (&left_host.host_id, &left.capability_id)
                .cmp(&(&right_host.host_id, &right.capability_id))
        });

        let mut realization_envelope = Vec::new();
        let mut needed = pool.maximum_members;
        for (host, capability) in candidates {
            if needed == 0 {
                break;
            }
            if capability.limits.max_queue_items < requirement.member_limits.queue_item_capacity
                || capability.limits.max_queue_bytes < requirement.member_limits.queue_byte_capacity
            {
                continue;
            }
            let mut member_capacity = capability.limits.max_active_instances.min(needed);
            let mut matched_resources = Vec::new();
            for resource in &capability.resource_requirements {
                if resource.units == 0
                    || resource.protected_role.is_some()
                    || resource.content.is_some()
                {
                    return Err(PlannerError::InvalidSharedPool(format!(
                        "dynamic member capability '{}' has an unsupported resource requirement",
                        capability.capability_id.as_str()
                    )));
                }
                let matching = host
                    .resources
                    .iter()
                    .filter(|offer| offer.class_id == resource.class_id)
                    .collect::<Vec<_>>();
                if matching.len() != 1 || matching[0].content.is_some() {
                    return Err(PlannerError::InvalidSharedPool(format!(
                        "dynamic member capability '{}' requires one unambiguous non-content resource pool",
                        capability.capability_id.as_str()
                    )));
                }
                let offer = matching[0];
                let available = remaining_resources
                    .get(&(host.host_id.clone(), offer.pool_id.clone()))
                    .copied()
                    .unwrap_or(0);
                member_capacity = member_capacity.min((available / resource.units) as u16);
                matched_resources.push((resource, offer, available));
            }
            if member_capacity == 0 {
                continue;
            }
            let mut resources = Vec::with_capacity(matched_resources.len());
            for (requirement, offer, available) in matched_resources {
                let compute = requirement
                    .compute
                    .as_ref()
                    .map(|_| {
                        conduit_core::compute_reservation(
                            requirement,
                            offer,
                            available / u32::from(member_capacity),
                        )
                        .ok_or_else(|| {
                            PlannerError::InvalidSharedPool(format!(
                            "dynamic member capability '{}' has an unsatisfied compute contract",
                            capability.capability_id.as_str()
                        ))
                        })
                    })
                    .transpose()?;
                resources.push(ResourceBinding {
                    content: None,
                    pool_id: offer.pool_id.clone(),
                    class_id: offer.class_id.clone(),
                    units: compute
                        .as_ref()
                        .map_or(requirement.units, |reservation| reservation.selected_lanes),
                    protected: None,
                    compute,
                });
            }
            for resource in &resources {
                let reserved = resource
                    .units
                    .checked_mul(u32::from(member_capacity))
                    .ok_or_else(|| {
                        PlannerError::InvalidSharedPool(
                            "dynamic member resource reservation overflowed".into(),
                        )
                    })?;
                let available = remaining_resources
                    .get_mut(&(host.host_id.clone(), resource.pool_id.clone()))
                    .expect("resolved pool remains in accounting");
                *available -= reserved;
            }
            let consumers = pool
                .consumers
                .iter()
                .map(|gear| {
                    let placement_id = placement_lookup.get(gear).ok_or_else(|| {
                        PlannerError::InvalidSharedPool(format!(
                            "shared pool consumer '{}' has no exact placement",
                            gear.as_str()
                        ))
                    })?;
                    planned_gears
                        .iter()
                        .copied()
                        .find(|placement| &placement.placement_id == placement_id)
                        .ok_or_else(|| {
                            PlannerError::InvalidSharedPool(
                                "shared pool consumer placement disappeared".into(),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let admitted_lines = pool_member_lines(
                host,
                &pool.member_front,
                &consumers,
                bases,
                options,
                requirement,
            )?;
            realization_envelope.push(PoolRealizationEnvelope {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                capability_id: capability.capability_id.clone(),
                implementation_id: capability.implementation.implementation_id.clone(),
                artifact_id: capability.implementation.artifact_id.clone(),
                member_capacity,
                resources,
                admitted_lines,
            });
            needed -= member_capacity;
        }
        if needed != 0 {
            return Err(PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' lacks capacity for {} members",
                pool.pool_id.as_str(),
                pool.maximum_members
            )));
        }
        let consumers = pool
            .consumers
            .iter()
            .map(|gear| {
                placement_lookup.get(gear).cloned().ok_or_else(|| {
                    PlannerError::InvalidSharedPool(format!(
                        "shared pool consumer '{}' has no exact placement",
                        gear.as_str()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let planned = PlannedSharedPool {
            pool_id: pool.pool_id.clone(),
            declaration_id: pool.declaration_id.clone(),
            member_front: pool.member_front.clone(),
            maximum_members: pool.maximum_members,
            member_limits: requirement.member_limits,
            member_sessions_required: requirement.member_sessions_required,
            realization_envelope,
            selection_policy:
                SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
            admission_authority: requirement.admission_authority.grant_id.clone(),
            consumers,
        };
        planned.validate().map_err(|error| {
            PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' is invalid: {error:?}",
                pool.pool_id.as_str()
            ))
        })?;
        planned_pools.push(planned);
    }
    for pool in &planned_pools {
        if !pool.member_sessions_required {
            continue;
        }
        for realization in &pool.realization_envelope {
            if plan.fragments.iter().any(|fragment| {
                fragment.host_id == realization.host_id && fragment.boot_id == realization.boot_id
            }) {
                continue;
            }
            let host = hosts
                .iter()
                .find(|host| {
                    host.host_id == realization.host_id
                        && host.boot_id == realization.boot_id
                        && host.offer_generation == realization.offer_generation
                })
                .ok_or_else(|| {
                    PlannerError::InvalidSharedPool(
                        "shared pool realization Host disappeared before fragment sealing".into(),
                    )
                })?;
            let expected_sign = vec![
                conduit_core::ExpectedSign::PlanFragmentReceived,
                conduit_core::ExpectedSign::PlanTerminal,
            ];
            let sign_storage_budget =
                conduit_core::mandatory_sign_storage_requirement(&expected_sign).ok_or_else(
                    || PlannerError::SignBudgetOverflow("shared pool participant fragment".into()),
                )?;
            plan.fragments.push(conduit_core::PlanFragment {
                plan_id: conduit_core::PlanId::from(""),
                fragment_id: conduit_core::FragmentId::from(""),
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
                completion_policy: crate::plan_completion_policy(form.completion),
                realization_backs: form.realization_backs.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements: Vec::new(),
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                states: Vec::new(),
                connections: Vec::new(),
                fore_ports: Vec::new(),
                shared_pools: Vec::new(),
                startup_dependencies: Vec::new(),
                startup_order: Vec::new(),
                cancellation_policy:
                    conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals: vec![conduit_core::ExpectedTerminal::PlanCompleted],
                expected_sign,
                sign_storage_budget,
                plan_fragments: Vec::new(),
            });
        }
    }
    plan.fragments.sort_by(|left, right| {
        (&left.host_id, &left.boot_id).cmp(&(&right.host_id, &right.boot_id))
    });
    for fragment in &mut plan.fragments {
        fragment.shared_pools = planned_pools.clone();
    }
    Ok(conduit_core::seal_plan_with_completion(
        FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
        },
        crate::plan_completion_policy(form.completion),
        plan.fragments,
    ))
}

fn validate_pool_authority(
    grant: &AuthorityGrant,
    hosts: &[HostAdvertisement],
) -> Result<(), PlannerError> {
    let exact_scope = grant.contract_id.as_str() == SHARED_POOL_ADMIT_AUTHORITY_CONTRACT
        && grant.host_call_contract_id.as_str() == SHARED_POOL_ADMIT_HOST_CALL_CONTRACT
        && grant.subject_kind.as_str() == SHARED_POOL_AUTHORITY_SUBJECT_KIND
        && !grant.grant_id.as_str().is_empty()
        && hosts.iter().any(|host| {
            host.host_id == grant.host_id
                && host.boot_id == grant.boot_id
                && host
                    .capabilities
                    .iter()
                    .any(|capability| capability.capability_id == grant.capability_id)
        });
    if !exact_scope {
        return Err(PlannerError::InvalidSharedPool(
            "shared-pool admission authority is missing or has the wrong exact scope".into(),
        ));
    }
    Ok(())
}

fn pool_member_lines(
    member_host: &HostAdvertisement,
    member_front: &conduit_core::CheckedFront,
    consumers: &[&PlannedGear],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirement: &SharedPoolPlanningRequirement,
) -> Result<Vec<AdmittedLine>, PlannerError> {
    if !requirement.member_sessions_required {
        return Ok(Vec::new());
    }

    let line_policy = crate::contract::LineMechanismPolicy::new(bases);
    let mut admitted = Vec::new();
    let mut identities = BTreeSet::new();
    for consumer in consumers {
        if consumer.host_id == member_host.host_id && consumer.boot_id == member_host.boot_id {
            continue;
        }
        let mut directions = Vec::with_capacity(2);
        if !member_front.inputs().is_empty() {
            directions.push((
                &consumer.host_id,
                &consumer.boot_id,
                &member_host.host_id,
                &member_host.boot_id,
                "consumer-to-member",
            ));
        }
        if !member_front.outputs().is_empty() {
            directions.push((
                &member_host.host_id,
                &member_host.boot_id,
                &consumer.host_id,
                &consumer.boot_id,
                "member-to-consumer",
            ));
        }
        for (source_host, source_boot, sink_host, sink_boot, direction) in directions {
            let matches = options
                .line_offers
                .iter()
                .filter(|offer| {
                    offer.binding.source.host_id == *source_host
                        && offer.binding.source.boot_id == *source_boot
                        && offer.binding.sink.host_id == *sink_host
                        && offer.binding.sink.boot_id == *sink_boot
                        && line_policy.permits_remote(&offer.binding.base)
                        && offer.validate_sign_identity()
                        && offer.availability.availability == LineAvailability::Ready
                        && offer.binding.limits.maximum_in_flight_items
                            >= requirement.member_limits.queue_item_capacity
                        && offer.binding.limits.maximum_payload_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_buffered_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_frame_bytes
                            >= offer.binding.limits.maximum_payload_bytes
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(PlannerError::InvalidSharedPool(format!(
                    "shared pool member session requires one exact ready bounded {direction} Line between '{}@{}' and '{}@{}'; found {}",
                    source_host.as_str(), source_boot.as_str(), sink_host.as_str(), sink_boot.as_str(), matches.len()
                )));
            }
            let line = matches[0].admitted_line();
            if !identities.insert((line.line_id.clone(), line.binding.binding_id.clone())) {
                return Err(PlannerError::InvalidSharedPool(
                    "shared pool member session repeats an admitted Line identity".into(),
                ));
            }
            admitted.push(line);
        }
    }
    Ok(admitted)
}
