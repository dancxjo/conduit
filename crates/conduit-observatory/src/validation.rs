use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use conduit_core::{verify_plan, ConnectionId, PlacementId, Plan, TerminalDisposition};

use crate::{CapabilityStatusReport, ObservatorySnapshot, PlanLifecycle, SNAPSHOT_SCHEMA};

pub fn validate_snapshot(snapshot: &ObservatorySnapshot) -> Result<(), String> {
    if snapshot.schema != SNAPSHOT_SCHEMA {
        return Err(format!(
            "unsupported Observatory snapshot schema: {}",
            snapshot.schema
        ));
    }
    validate_retention(snapshot)?;
    let host_boots = validate_hosts(snapshot)?;
    validate_bases_and_provenance(snapshot, &host_boots)?;
    validate_lines(snapshot, &host_boots)?;
    let plan_ids = validate_plans(snapshot, &host_boots)?;
    validate_plays(snapshot, &host_boots)?;
    validate_signs(snapshot, &host_boots, &plan_ids)
}

fn validate_retention(snapshot: &ObservatorySnapshot) -> Result<(), String> {
    let observation_count = u32::try_from(
        snapshot
            .observations
            .len()
            .checked_add(snapshot.historical_observations.len())
            .ok_or_else(|| "Observatory snapshot observation count overflowed".to_string())?,
    )
    .map_err(|_| "Observatory snapshot observation count exceeds u32".to_string())?;
    if snapshot.retention.retained_items != observation_count
        || snapshot.retention.retained_items > snapshot.retention.item_capacity
    {
        return Err("Observatory snapshot retention accounting is invalid".to_string());
    }
    Ok(())
}

fn validate_hosts(
    snapshot: &ObservatorySnapshot,
) -> Result<BTreeSet<(conduit_core::HostId, conduit_core::BootId)>, String> {
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
    Ok(host_boots)
}

fn validate_bases_and_provenance(
    snapshot: &ObservatorySnapshot,
    host_boots: &BTreeSet<(conduit_core::HostId, conduit_core::BootId)>,
) -> Result<(), String> {
    let mut base_ids = BTreeSet::new();
    for base in &snapshot.bases {
        if !host_boots.contains(&(base.host_id.clone(), base.boot_id.clone())) {
            return Err("Base names an unreported host/boot".to_string());
        }
        if base.base_id.as_str().is_empty()
            || base.kind_id.as_str().is_empty()
            || base.capacity_units == 0
        {
            return Err("Base report has an empty identity/kind or zero capacity".to_string());
        }
        if !base_ids.insert((
            base.host_id.clone(),
            base.boot_id.clone(),
            base.base_id.clone(),
        )) {
            return Err("duplicate Base report".to_string());
        }
    }

    let mut provenance_boots = BTreeSet::new();
    for provenance in &snapshot.sealed_boot_provenance {
        let host_boot = (provenance.host_id.clone(), provenance.boot_id.clone());
        if !host_boots.contains(&host_boot) {
            return Err("sealed boot provenance names an unreported host/boot".to_string());
        }
        if !provenance_boots.insert(host_boot) {
            return Err("duplicate sealed boot provenance".to_string());
        }
        if provenance.firmware_environment.is_empty()
            || provenance.adapter_name.is_empty()
            || provenance.adapter_version.is_empty()
            || provenance.adapter_revision.is_empty()
            || provenance.image_id.as_str().is_empty()
            || provenance.build_id.as_str().is_empty()
            || provenance.memory_map.normalized_region_count == 0
            || provenance.memory_map.runtime_arena_bytes == 0
        {
            return Err("sealed boot provenance has an empty required fact".to_string());
        }
        for framebuffer in &provenance.framebuffers {
            if !snapshot.bases.iter().any(|base| {
                base.host_id == provenance.host_id
                    && base.boot_id == provenance.boot_id
                    && base.base_id == framebuffer.base_id
            }) || framebuffer.width == 0
                || framebuffer.height == 0
                || framebuffer.pitch_bytes == 0
                || framebuffer.bits_per_pixel == 0
            {
                return Err("framebuffer provenance lacks one exact reported Base".to_string());
            }
        }
    }
    Ok(())
}

fn validate_lines(
    snapshot: &ObservatorySnapshot,
    host_boots: &BTreeSet<(conduit_core::HostId, conduit_core::BootId)>,
) -> Result<(), String> {
    let mut line_ids = BTreeSet::new();
    for line in &snapshot.lines {
        if !line_ids.insert(line.offer.line_id.clone()) {
            return Err("duplicate Line observation".to_string());
        }
        for endpoint in [&line.offer.binding.source, &line.offer.binding.sink] {
            if !host_boots.contains(&(endpoint.host_id.clone(), endpoint.boot_id.clone())) {
                return Err(format!(
                    "Line {} names an unreported host/boot",
                    line.offer.line_id.as_str()
                ));
            }
        }
    }
    Ok(())
}

fn validate_plans(
    snapshot: &ObservatorySnapshot,
    host_boots: &BTreeSet<(conduit_core::HostId, conduit_core::BootId)>,
) -> Result<BTreeSet<conduit_core::PlanId>, String> {
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
    Ok(plan_ids)
}

fn validate_plays(
    snapshot: &ObservatorySnapshot,
    host_boots: &BTreeSet<(conduit_core::HostId, conduit_core::BootId)>,
) -> Result<(), String> {
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
        validate_play_children(plan, play)?;
    }
    Ok(())
}

fn validate_play_children(plan: &Plan, play: &crate::PlayReport) -> Result<(), String> {
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
        let planned = plan_connection(plan, &connection.connection_id)
            .ok_or_else(|| "Play names an unreported connection".to_string())?;
        if let Some(terminal) = &connection.terminal_disposition {
            if connection.lifecycle != lifecycle_for_terminal(terminal.disposition) {
                return Err("connection terminal disposition disagrees with lifecycle".into());
            }
        }
        if connection.failure_message.is_some() && connection.lifecycle != PlanLifecycle::Failed {
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
                return Err("connection pressure exceeds planned Cord bounds".into());
            }
            if (pressure.pressure_events == 0) != pressure.last_pressure_sequence.is_none() {
                return Err("connection pressure event accounting is inconsistent".into());
            }
        }
    }
    Ok(())
}

fn validate_signs(
    snapshot: &ObservatorySnapshot,
    host_boots: &BTreeSet<(conduit_core::HostId, conduit_core::BootId)>,
    plan_ids: &BTreeSet<conduit_core::PlanId>,
) -> Result<(), String> {
    let mut sign_ids = BTreeSet::new();
    for observation in snapshot
        .observations
        .iter()
        .chain(&snapshot.historical_observations)
    {
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
            return Err("presentation Sign has no active Play identity".to_string());
        }
        validate_sign_plan_membership(snapshot, observation)?;
        if !sign_ids.insert(observation.sign_id.clone()) {
            return Err("duplicate Sign identity".to_string());
        }
    }
    Ok(())
}

fn validate_sign_plan_membership(
    snapshot: &ObservatorySnapshot,
    observation: &conduit_core::Observation,
) -> Result<(), String> {
    if let (Some(plan_id), Some(placement_id)) = (&observation.plan_id, &observation.placement_id) {
        let plan = snapshot
            .plans
            .iter()
            .find(|plan| &plan.plan_id == plan_id)
            .expect("plan membership checked above");
        if !plan_contains_placement(plan, placement_id) {
            return Err("observation names an unreported placement".to_string());
        }
    }
    if let (Some(plan_id), Some(connection_id)) = (&observation.plan_id, &observation.connection_id)
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

fn plan_contains_placement(plan: &Plan, placement_id: &PlacementId) -> bool {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| &placement.placement_id == placement_id)
}

fn plan_contains_connection(plan: &Plan, connection_id: &ConnectionId) -> bool {
    plan_connection(plan, connection_id).is_some()
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
    lifecycle: PlanLifecycle,
    terminal: Option<TerminalDisposition>,
    failure_message: Option<&str>,
) -> Result<(), String> {
    if terminal.is_some_and(|terminal| lifecycle != lifecycle_for_terminal(terminal)) {
        return Err("terminal disposition disagrees with lifecycle".into());
    }
    if failure_message.is_some() && lifecycle != PlanLifecycle::Failed {
        return Err("failure detail requires failed lifecycle".into());
    }
    Ok(())
}

fn lifecycle_for_terminal(disposition: TerminalDisposition) -> PlanLifecycle {
    match disposition {
        TerminalDisposition::Completed => PlanLifecycle::Completed,
        TerminalDisposition::Failed { .. } => PlanLifecycle::Failed,
        TerminalDisposition::Cancelled { .. } => PlanLifecycle::Cancelled,
    }
}
