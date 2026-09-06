use crate::prelude::*;
use crate::{
    default_placements_unvalidated, plan_validated_form,
    plan_validated_form_with_connection_limits, ConnectionEndpoints, ConnectionQueueLimits,
    PlacementChoices, PlannerError, PlanningOptions,
};
use alloc::collections::BTreeMap;
use conduit_core::{
    seal_plan, AuthorityGrant, BaseImplementationId, FormIdentity, HostAdvertisement, Plan,
    PlannedSharedPool, PoolMemberLimits, PoolRealizationEnvelope, ResourceBinding, SharedPoolId,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    expand_canonical_form, expand_canonical_form_with_backs, CanonicalBackCatalog, CheckedForm,
    CheckedSyntaxDocument, ExpandedCanonicalForm, ProfileCatalog,
};

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
}

pub fn default_expanded_placements(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    default_placements_unvalidated(&form.gears, hosts)
}

pub fn plan_expanded_canonical(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    plan_expanded_canonical_with_options(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

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
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let plan = plan_validated_form(&planning_form, hosts, placements, bases, options)?;
    Ok(conduit_core::seal_plan_with_realization_backs(
        conduit_core::FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
        },
        form.realization_backs.clone(),
        plan.fragments,
    ))
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
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let plan = plan_validated_form_with_connection_limits(
        &planning_form,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    Ok(conduit_core::seal_plan_with_realization_backs(
        conduit_core::FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
        },
        form.realization_backs.clone(),
        plan.fragments,
    ))
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
            .filter(|host| {
                plan.fragments.iter().any(|fragment| {
                    fragment.host_id == host.host_id && fragment.boot_id == host.boot_id
                })
            })
            .flat_map(|host| {
                host.capabilities
                    .iter()
                    .filter(|capability| capability.checked_face() == pool.member_face)
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
            let mut resources = Vec::new();
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
                resources.push(ResourceBinding {
                    content: None,
                    pool_id: offer.pool_id.clone(),
                    class_id: offer.class_id.clone(),
                    units: resource.units,
                    protected: None,
                    compute: None,
                });
            }
            if member_capacity == 0 {
                continue;
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
            realization_envelope.push(PoolRealizationEnvelope {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                capability_id: capability.capability_id.clone(),
                member_capacity,
                resources,
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
        let placement_lookup = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| (placement.gear_id.clone(), placement.placement_id.clone()))
            .collect::<BTreeMap<_, _>>();
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
            member_face: pool.member_face.clone(),
            maximum_members: pool.maximum_members,
            member_limits: requirement.member_limits,
            realization_envelope,
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
    for fragment in &mut plan.fragments {
        fragment.shared_pools = planned_pools.clone();
    }
    Ok(seal_plan(
        FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
        },
        plan.fragments,
    ))
}

fn validate_pool_authority(
    grant: &AuthorityGrant,
    hosts: &[HostAdvertisement],
) -> Result<(), PlannerError> {
    let exact_scope = grant.contract_id.as_str() == SHARED_POOL_ADMIT_AUTHORITY_CONTRACT
        && grant.host_operation_contract_id.as_str() == SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT
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
