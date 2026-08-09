use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::{
    verify_plan, ConnectionId, ObservationKind, PlacementId, Plan, PlanId, TerminalDisposition,
};

use crate::{
    CapabilityAvailability, CapabilityRow, CapabilityStatusReport, CapabilitySupport, FragmentRow,
    HostRow, LineRow, ObservatoryReport, ObservatorySnapshot, OfferFreshness, OperationalState,
    PlacementRow, PlanRow, RetentionRow, SignRow,
};

pub const SNAPSHOT_SCHEMA: &str = "conduit.observatory.snapshot/v1";

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
                        capability_id: placement.capability_id.clone(),
                        kind_id: placement.kind_id.clone(),
                        kind_contract_revision: placement.kind_contract_revision.clone(),
                        execution_profile_id: placement.execution_profile_id.clone(),
                        implementation_id: placement.implementation_id.clone(),
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

    let sign = snapshot
        .observations
        .iter()
        .map(|observation| SignRow {
            sign_id: observation.sign_id.clone(),
            active_play_id: observation.active_play_id.clone(),
            presentation_id: observation.presentation_id.clone(),
            host_id: observation.host_id.clone(),
            boot_id: observation.boot_id.clone(),
            plan_id: observation.plan_id.clone(),
            placement_id: observation.placement_id.clone(),
            connection_id: observation.connection_id.clone(),
            kind: observation.kind.clone(),
        })
        .collect::<Vec<_>>();
    let host_gaps = snapshot
        .observations
        .iter()
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
        lines,
        plans,
        fragments,
        placements,
        connections,
        plays: snapshot.plays.clone(),
        signs: sign,
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

pub fn validate_snapshot(snapshot: &ObservatorySnapshot) -> Result<(), String> {
    if snapshot.schema != SNAPSHOT_SCHEMA {
        return Err(format!(
            "unsupported Observatory snapshot schema: {}",
            snapshot.schema
        ));
    }
    let observation_count = u32::try_from(snapshot.observations.len())
        .map_err(|_| "Observatory snapshot observation count exceeds u32".to_string())?;
    if snapshot.retention.retained_items != observation_count
        || snapshot.retention.retained_items > snapshot.retention.item_capacity
    {
        return Err("Observatory snapshot retention accounting is invalid".to_string());
    }

    let mut host_boots = BTreeSet::new();
    for host in &snapshot.hosts {
        let key = (
            host.advertisement.host_id.clone(),
            host.advertisement.boot_id.clone(),
        );
        if !host_boots.insert(key) {
            return Err("duplicate host/boot report".to_string());
        }
        validate_capability_statuses(&host.capabilities, &host.advertisement.capabilities)?;
    }

    let mut link_ids = BTreeSet::new();
    for link in &snapshot.lines {
        if !link_ids.insert(link.offer.line_id.clone()) {
            return Err("duplicate link observation".to_string());
        }
        for endpoint in [&link.offer.binding.source, &link.offer.binding.sink] {
            if !host_boots.contains(&(endpoint.host_id.clone(), endpoint.boot_id.clone())) {
                return Err(format!(
                    "link {} names an unreported host/boot",
                    link.offer.line_id.as_str()
                ));
            }
        }
    }

    let mut plan_ids = BTreeSet::new();
    for plan in &snapshot.plans {
        if !verify_plan(plan) {
            return Err(format!(
                "plan {} failed exact verification",
                plan.plan_id.as_str()
            ));
        }
        if !plan_ids.insert(plan.plan_id.clone()) {
            return Err(format!("duplicate plan identity {}", plan.plan_id.as_str()));
        }
        for fragment in &plan.fragments {
            if !host_boots.contains(&(fragment.host_id.clone(), fragment.boot_id.clone())) {
                return Err(format!(
                    "plan {} names an unreported fragment host/boot",
                    plan.plan_id.as_str()
                ));
            }
        }
    }

    let mut play_ids = BTreeSet::new();
    for play in &snapshot.plays {
        if !play_ids.insert(play.active_play_id.clone()) {
            return Err("duplicate active Play identity".to_string());
        }
        let plan = snapshot
            .plans
            .iter()
            .find(|plan| plan.plan_id == play.plan_id)
            .ok_or_else(|| "Play names an unreported plan".to_string())?;
        if !host_boots.contains(&(play.host_id.clone(), play.boot_id.clone())) {
            return Err("Play names an unreported host/boot".to_string());
        }
        if !plan
            .fragments
            .iter()
            .any(|fragment| fragment.host_id == play.host_id && fragment.boot_id == play.boot_id)
        {
            return Err("Play host/boot has no exact plan fragment".to_string());
        }
        validate_lifecycle(
            play.lifecycle,
            play.terminal_disposition,
            play.failure_message.as_deref(),
        )?;
        let mut placement_ids = BTreeSet::new();
        for placement in &play.placements {
            if !placement_ids.insert(placement.placement_id.clone()) {
                return Err("duplicate Play placement report".to_string());
            }
            if !plan_contains_placement(plan, &placement.placement_id) {
                return Err("Play names an unreported placement".to_string());
            }
            validate_lifecycle(
                placement.lifecycle,
                placement.terminal_disposition,
                placement.failure_message.as_deref(),
            )?;
        }
        let mut connection_ids = BTreeSet::new();
        for connection in &play.connections {
            if !connection_ids.insert(connection.connection_id.clone()) {
                return Err("duplicate Play connection report".to_string());
            }
            if !plan_contains_connection(plan, &connection.connection_id) {
                return Err("Play names an unreported connection".to_string());
            }
            let planned = plan_connection(plan, &connection.connection_id)
                .expect("connection membership checked above");
            if let Some(terminal) = &connection.terminal_disposition {
                if connection.lifecycle != lifecycle_for_terminal(terminal.disposition) {
                    return Err("connection terminal disposition disagrees with lifecycle".into());
                }
            }
            if connection.failure_message.is_some()
                && connection.lifecycle != crate::PlanLifecycle::Failed
            {
                return Err("connection failure detail requires failed lifecycle".into());
            }
            if let Some(pressure) = &connection.pressure {
                if pressure
                    .current_in_flight_items
                    .is_some_and(|items| items > planned.item_capacity)
                    || pressure
                        .current_buffered_bytes
                        .is_some_and(|bytes| bytes > planned.byte_capacity)
                {
                    return Err("connection pressure exceeds planned cord bounds".into());
                }
                if (pressure.pressure_events == 0) != pressure.last_pressure_sequence.is_none() {
                    return Err("connection pressure event accounting is inconsistent".into());
                }
            }
        }
    }

    let mut sign_ids = BTreeSet::new();
    for observation in &snapshot.observations {
        if !host_boots.contains(&(observation.host_id.clone(), observation.boot_id.clone())) {
            return Err(format!(
                "observation {} names an unreported host/boot",
                observation.sign_id.as_str()
            ));
        }
        if observation
            .plan_id
            .as_ref()
            .is_some_and(|plan_id| !plan_ids.contains(plan_id))
        {
            return Err("observation names an unreported plan".to_string());
        }
        if let Some(play_id) = &observation.active_play_id {
            let play = snapshot
                .plays
                .iter()
                .find(|play| &play.active_play_id == play_id)
                .ok_or_else(|| "observation names an unreported Play".to_string())?;
            if observation.plan_id.as_ref() != Some(&play.plan_id)
                || observation.host_id != play.host_id
                || observation.boot_id != play.boot_id
            {
                return Err("observation identity disagrees with its Play".to_string());
            }
        }
        if observation.presentation_id.is_some() && observation.active_play_id.is_none() {
            return Err("presentation sign has no active Play identity".to_string());
        }
        if let (Some(plan_id), Some(placement_id)) =
            (&observation.plan_id, &observation.placement_id)
        {
            let plan = snapshot
                .plans
                .iter()
                .find(|plan| &plan.plan_id == plan_id)
                .expect("plan membership checked above");
            if !plan_contains_placement(plan, placement_id) {
                return Err("observation names an unreported placement".to_string());
            }
        }
        if let (Some(plan_id), Some(connection_id)) =
            (&observation.plan_id, &observation.connection_id)
        {
            let plan = snapshot
                .plans
                .iter()
                .find(|plan| &plan.plan_id == plan_id)
                .expect("plan membership checked above");
            if !plan_contains_connection(plan, connection_id) {
                return Err("observation names an unreported connection".to_string());
            }
        }
        if !sign_ids.insert(observation.sign_id.clone()) {
            return Err("duplicate sign identity".to_string());
        }
    }
    Ok(())
}

fn validate_capability_statuses(
    statuses: &[CapabilityStatusReport],
    offers: &[conduit_core::CapabilityOffer],
) -> Result<(), String> {
    let offered = offers
        .iter()
        .map(|offer| offer.capability_id.clone())
        .collect::<BTreeSet<_>>();
    let mut reported = BTreeSet::new();
    for status in statuses {
        if !offered.contains(&status.capability_id) {
            return Err("capability status names an unadvertised capability".to_string());
        }
        if !reported.insert(status.capability_id.clone()) {
            return Err("duplicate capability status".to_string());
        }
    }
    Ok(())
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

fn plan_contains_placement(plan: &Plan, placement_id: &PlacementId) -> bool {
    distinct_placements(plan).contains(placement_id)
}

fn plan_contains_connection(plan: &Plan, connection_id: &ConnectionId) -> bool {
    distinct_connections(plan).contains(connection_id)
}

fn plan_connection<'a>(
    plan: &'a Plan,
    connection_id: &ConnectionId,
) -> Option<&'a conduit_core::PlannedConnection> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| &connection.connection_id == connection_id)
}

fn validate_lifecycle(
    lifecycle: crate::PlanLifecycle,
    terminal: Option<TerminalDisposition>,
    failure_message: Option<&str>,
) -> Result<(), String> {
    if terminal.is_some_and(|terminal| lifecycle != lifecycle_for_terminal(terminal)) {
        return Err("terminal disposition disagrees with lifecycle".into());
    }
    if failure_message.is_some() && lifecycle != crate::PlanLifecycle::Failed {
        return Err("failure detail requires failed lifecycle".into());
    }
    Ok(())
}

fn lifecycle_for_terminal(disposition: TerminalDisposition) -> crate::PlanLifecycle {
    match disposition {
        TerminalDisposition::Completed => crate::PlanLifecycle::Completed,
        TerminalDisposition::Failed { .. } => crate::PlanLifecycle::Failed,
        TerminalDisposition::Cancelled { .. } => crate::PlanLifecycle::Cancelled,
    }
}

pub fn unsupported_state() -> OperationalState {
    OperationalState::Unsupported
}
