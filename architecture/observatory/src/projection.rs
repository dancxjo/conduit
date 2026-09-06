use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::{ConnectionId, ObservationKind, PlacementId, Plan, PlanId};

use crate::{
    validate_snapshot, CapabilityAvailability, CapabilityRow, CapabilitySupport, DeviceRow,
    ExecutionRegionRow, FragmentRow, HostRow, LineRow, ObservatoryReport, ObservatorySnapshot,
    OfferFreshness, OperationalState, PlacementRow, PlanRow, RetentionRow, SignRow,
};

pub const SNAPSHOT_SCHEMA: &str = "conduit.observatory.snapshot/v2";

pub fn build_report(snapshot: &ObservatorySnapshot) -> Result<ObservatoryReport, String> {
    validate_snapshot(snapshot)?;

    let hosts = snapshot
        .hosts
        .iter()
        .map(|report| HostRow {
            host_id: report.advertisement.host_id.clone(),
            boot_id: report.advertisement.boot_id.clone(),
            profile: report.advertisement.profile.clone(),
            offer_generation: report.advertisement.offer_generation,
            state: report.state,
            capability_count: report.advertisement.capabilities.len(),
            planner_capabilities: report.advertisement.planner_capabilities.clone(),
            resources: report.advertisement.resources.clone(),
        })
        .collect::<Vec<_>>();

    let capabilities = snapshot
        .hosts
        .iter()
        .flat_map(|host| {
            host.advertisement.capabilities.iter().map(|capability| {
                let status = host
                    .capabilities
                    .iter()
                    .find(|status| status.capability_id == capability.capability_id);
                CapabilityRow {
                    host_id: host.advertisement.host_id.clone(),
                    boot_id: host.advertisement.boot_id.clone(),
                    capability_id: capability.capability_id.clone(),
                    kind_id: capability.kind_id.clone(),
                    kind_contract_revision: capability.kind_contract_revision.clone(),
                    execution_profile_id: capability.implementation.execution_profile_id.clone(),
                    implementation_id: capability.implementation.implementation_id.clone(),
                    inputs: capability.inputs.clone(),
                    outputs: capability.outputs.clone(),
                    host_operations: capability.host_operations.clone(),
                    resource_requirements: capability.resource_requirements.clone(),
                    authority_requirements: capability.authority_requirements.clone(),
                    limits: capability.limits.clone(),
                    freshness: status.map_or(OfferFreshness::Unknown, |status| status.freshness),
                    support: status.map_or(CapabilitySupport::Unknown, |status| status.support),
                    availability: status.map_or(CapabilityAvailability::Unknown, |status| {
                        status.availability
                    }),
                }
            })
        })
        .collect::<Vec<_>>();

    let devices = snapshot
        .hosts
        .iter()
        .flat_map(|host| {
            host.devices
                .iter()
                .cloned()
                .map(|association| DeviceRow { association })
        })
        .collect::<Vec<_>>();

    let lines = snapshot
        .lines
        .iter()
        .map(|report| LineRow {
            offer: report.offer.clone(),
            state: report.state,
        })
        .collect::<Vec<_>>();

    let plans = snapshot
        .plans
        .iter()
        .map(|plan| PlanRow {
            plan_id: plan.plan_id.clone(),
            source_document_id: plan.source_document_id.clone(),
            checked_form_id: plan.checked_form_id.clone(),
            expanded_form_id: plan.expanded_form_id.clone(),
            fragment_count: plan.fragments.len(),
            placement_count: distinct_placements(plan).len(),
            connection_count: distinct_connections(plan).len(),
            execution_region_count: plan
                .fragments
                .iter()
                .map(|fragment| fragment.execution_regions.len())
                .sum(),
        })
        .collect::<Vec<_>>();

    let execution_regions = snapshot
        .plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().flat_map(move |fragment| {
                fragment
                    .execution_regions
                    .iter()
                    .map(move |region| ExecutionRegionRow {
                        plan_id: plan.plan_id.clone(),
                        fragment_id: fragment.fragment_id.clone(),
                        region_id: region.region_id.clone(),
                        admitted_placements: region.admitted_placements.clone(),
                        execution_profile_id: region.execution_profile_id.clone(),
                        scheduling: region.scheduling,
                        lane_count: region.lane_count,
                        lane_resource: region.lane_resource.clone(),
                        lane_base_id: region.lane_base_id.clone(),
                        requirements: region.requirements,
                        preemption_required: region.preemption_required,
                        isolation_required: region.isolation_required,
                    })
            })
        })
        .collect::<Vec<_>>();

    let fragments = snapshot
        .plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().map(|fragment| FragmentRow {
                plan_id: plan.plan_id.clone(),
                fragment_id: fragment.fragment_id.clone(),
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
            })
        })
        .collect::<Vec<_>>();

    let placements = snapshot
        .plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().flat_map(move |fragment| {
                fragment
                    .placements
                    .iter()
                    .map(move |placement| PlacementRow {
                        plan_id: plan.plan_id.clone(),
                        placement_id: placement.placement_id.clone(),
                        host_id: placement.host_id.clone(),
                        boot_id: placement.boot_id.clone(),
                        offer_generation: placement.offer_generation,
                        capability_id: placement.capability_id.clone(),
                        kind_id: placement.kind_id.clone(),
                        kind_contract_revision: placement.kind_contract_revision.clone(),
                        execution_profile_id: placement.execution_profile_id.clone(),
                        implementation_id: placement.implementation_id.clone(),
                        artifact_id: placement.artifact_id.clone(),
                        host_operations: placement.host_operations.clone(),
                        resources: placement.resources.clone(),
                        authority: placement.authority.clone(),
                    })
            })
        })
        .collect::<Vec<_>>();

    let connections = snapshot
        .plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().flat_map(move |fragment| {
                fragment
                    .connections
                    .iter()
                    .map(move |connection| crate::ConnectionRow {
                        plan_id: plan.plan_id.clone(),
                        connection_id: connection.connection_id.clone(),
                        source_placement_id: connection.source_placement_id.clone(),
                        sink_placement_id: connection.sink_placement_id.clone(),
                        value_kind: connection.value_kind.clone(),
                        selected_line: connection.selected_line.clone(),
                        admitted_lines: connection.admitted_lines.clone(),
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                    })
            })
        })
        .fold(
            BTreeMap::<(PlanId, ConnectionId), crate::ConnectionRow>::new(),
            |mut rows, row| {
                rows.entry((row.plan_id.clone(), row.connection_id.clone()))
                    .or_insert(row);
                rows
            },
        )
        .into_values()
        .collect::<Vec<_>>();

    let mut signs = snapshot
        .observations
        .iter()
        .map(|observation| sign_row(observation, false))
        .collect::<Vec<_>>();
    signs.extend(
        snapshot
            .historical_observations
            .iter()
            .map(|observation| sign_row(observation, true)),
    );
    let host_gaps = snapshot
        .observations
        .iter()
        .chain(&snapshot.historical_observations)
        .filter_map(|observation| match observation.kind {
            ObservationKind::SignGap { dropped } => Some(dropped),
            _ => None,
        })
        .try_fold(0u64, u64::checked_add)
        .ok_or_else(|| "host sign gap count overflowed".to_string())?;
    let visible_gap_count = host_gaps
        .checked_add(snapshot.retention.dropped_items)
        .ok_or_else(|| "combined retention gap count overflowed".to_string())?;

    Ok(ObservatoryReport {
        hosts,
        capabilities,
        devices,
        bases: snapshot.bases.clone(),
        lines,
        plans,
        execution_regions,
        fragments,
        placements,
        connections,
        plays: snapshot.plays.clone(),
        signs,
        sealed_boot_provenance: snapshot.sealed_boot_provenance.clone(),
        retention: RetentionRow {
            bounded: true,
            item_capacity: snapshot.retention.item_capacity,
            retained_items: snapshot.retention.retained_items,
            visible_gap_count,
            explanation: format!(
                "snapshot retained {} of {} sign slots; snapshot dropped {}; hosts reported {} additional gaps",
                snapshot.retention.retained_items,
                snapshot.retention.item_capacity,
                snapshot.retention.dropped_items,
                host_gaps
            ),
        },
    })
}

fn sign_row(observation: &conduit_core::Observation, historical: bool) -> SignRow {
    SignRow {
        sign_id: observation.sign_id.clone(),
        active_play_id: observation.active_play_id.clone(),
        presentation_id: observation.presentation_id.clone(),
        host_id: observation.host_id.clone(),
        boot_id: observation.boot_id.clone(),
        plan_id: observation.plan_id.clone(),
        placement_id: observation.placement_id.clone(),
        connection_id: observation.connection_id.clone(),
        kind: observation.kind.clone(),
        historical,
    }
}

fn distinct_placements(plan: &Plan) -> BTreeSet<PlacementId> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.placement_id.clone())
        .collect()
}

fn distinct_connections(plan: &Plan) -> BTreeSet<ConnectionId> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .map(|connection| connection.connection_id.clone())
        .collect()
}

pub fn unsupported_state() -> OperationalState {
    OperationalState::Unsupported
}
