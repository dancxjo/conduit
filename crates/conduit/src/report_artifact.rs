use conduit_core::{
    ConnectionTerminalDisposition, ExpectedTerminal, HostAdvertisement, LinkAvailability,
    Observation, ObservationKind, Plan, TerminalDisposition,
};
use conduit_observatory::{
    validate_snapshot, CapabilityAvailability, CapabilityStatusReport, CapabilitySupport,
    HostReport, LinkReport, ObservatorySnapshot, OfferFreshness, OperationalState, PlanLifecycle,
    PlayConnectionReport, PlayPlacementReport, PlayReport, RetentionReport, SNAPSHOT_SCHEMA,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const RETAINED_OBSERVATION_CAPACITY: usize = 256;

pub fn snapshot_from_execution(
    advertisements: Vec<HostAdvertisement>,
    plans: Vec<Plan>,
    observations: Vec<Observation>,
) -> ObservatorySnapshot {
    let dropped_items = observations
        .len()
        .saturating_sub(RETAINED_OBSERVATION_CAPACITY);
    let observations = observations
        .into_iter()
        .skip(dropped_items)
        .collect::<Vec<_>>();
    let hosts = advertisements
        .into_iter()
        .map(|advertisement| HostReport {
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|capability| CapabilityStatusReport {
                    capability_id: capability.capability_id.clone(),
                    freshness: OfferFreshness::Fresh,
                    support: CapabilitySupport::Supported,
                    availability: CapabilityAvailability::Available,
                })
                .collect(),
            advertisement,
            state: OperationalState::Available,
        })
        .collect::<Vec<_>>();
    let links = links_from_plans(&plans);
    let plays = plays_from_observations(&plans, &observations);
    ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.to_string(),
        hosts,
        links,
        plans,
        plays,
        retention: RetentionReport {
            item_capacity: RETAINED_OBSERVATION_CAPACITY as u32,
            retained_items: observations.len() as u32,
            dropped_items: dropped_items as u64,
        },
        observations,
    }
}

fn links_from_plans(plans: &[Plan]) -> Vec<LinkReport> {
    plans
        .iter()
        .flat_map(|plan| &plan.fragments)
        .flat_map(|fragment| &fragment.connections)
        .filter_map(|connection| connection.link_binding.as_ref())
        .fold(BTreeMap::new(), |mut links, binding| {
            links
                .entry(binding.binding_id.clone())
                .or_insert_with(|| LinkReport {
                    state: match binding.availability {
                        LinkAvailability::Ready => OperationalState::Available,
                        LinkAvailability::Unavailable => OperationalState::Unknown,
                    },
                    binding: binding.clone(),
                });
            links
        })
        .into_values()
        .collect()
}

fn plays_from_observations(plans: &[Plan], observations: &[Observation]) -> Vec<PlayReport> {
    let identities = observations
        .iter()
        .filter_map(|observation| {
            Some((
                observation.active_play_id.clone()?,
                observation.plan_id.clone()?,
                observation.host_id.clone(),
                observation.boot_id.clone(),
            ))
        })
        .collect::<BTreeSet<_>>();
    identities
        .into_iter()
        .filter_map(|(active_play_id, plan_id, host_id, boot_id)| {
            let plan = plans.iter().find(|plan| plan.plan_id == plan_id)?;
            let play_observations = observations
                .iter()
                .filter(|observation| observation.active_play_id.as_ref() == Some(&active_play_id))
                .collect::<Vec<_>>();
            let (lifecycle, terminal_disposition) = play_lifecycle(&play_observations);
            let failure_message = play_observations.iter().find_map(|observation| {
                if let ObservationKind::Failure { message, .. } = &observation.kind {
                    message.clone()
                } else {
                    None
                }
            });
            Some(PlayReport {
                active_play_id,
                plan_id,
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                lifecycle,
                terminal_disposition,
                failure_message,
                placements: play_placements(
                    plan,
                    &host_id,
                    &boot_id,
                    terminal_disposition,
                    &play_observations,
                ),
                connections: play_connections(plan, terminal_disposition, &play_observations),
            })
        })
        .collect()
}

fn play_lifecycle(observations: &[&Observation]) -> (PlanLifecycle, Option<TerminalDisposition>) {
    let mut lifecycle = PlanLifecycle::Unknown;
    let mut terminal = None;
    for observation in observations {
        match observation.kind {
            ObservationKind::PlanTerminal { disposition } => {
                lifecycle = lifecycle_for_terminal(disposition);
                terminal = Some(disposition);
            }
            ObservationKind::PlanCompleted => lifecycle = PlanLifecycle::Completed,
            ObservationKind::PlanActivated => lifecycle = PlanLifecycle::Active,
            ObservationKind::PlanFragmentReceived => lifecycle = PlanLifecycle::Prepared,
            ObservationKind::Released => lifecycle = PlanLifecycle::Released,
            _ => {}
        }
    }
    (lifecycle, terminal)
}

fn play_placements(
    plan: &Plan,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
    play_terminal: Option<TerminalDisposition>,
    observations: &[&Observation],
) -> Vec<PlayPlacementReport> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| &placement.host_id == host_id && &placement.boot_id == boot_id)
        .map(|placement| {
            let mut lifecycle = PlanLifecycle::Unknown;
            let mut terminal = None;
            let mut failure_message = None;
            for observation in observations.iter().filter(|observation| {
                observation.placement_id.as_ref() == Some(&placement.placement_id)
            }) {
                match &observation.kind {
                    ObservationKind::PlacementPrepared => lifecycle = PlanLifecycle::Prepared,
                    ObservationKind::PlacementCompleted => lifecycle = PlanLifecycle::Completed,
                    ObservationKind::PlacementTerminal { disposition } => {
                        lifecycle = lifecycle_for_terminal(*disposition);
                        terminal = Some(*disposition);
                    }
                    ObservationKind::Failure { message, .. } => {
                        lifecycle = PlanLifecycle::Failed;
                        failure_message = message.clone();
                    }
                    _ => {}
                }
            }
            if lifecycle == PlanLifecycle::Unknown
                && play_terminal == Some(TerminalDisposition::Completed)
                && plan.fragments.iter().any(|fragment| {
                    fragment.expected_terminals.iter().any(|terminal| {
                        terminal
                            == &ExpectedTerminal::PlacementCompleted(placement.placement_id.clone())
                    })
                })
            {
                lifecycle = PlanLifecycle::Completed;
                terminal = Some(TerminalDisposition::Completed);
            }
            PlayPlacementReport {
                placement_id: placement.placement_id.clone(),
                lifecycle,
                terminal_disposition: terminal,
                failure_message,
            }
        })
        .collect()
}

fn play_connections(
    plan: &Plan,
    play_terminal: Option<TerminalDisposition>,
    observations: &[&Observation],
) -> Vec<PlayConnectionReport> {
    let mut connection_ids = BTreeSet::new();
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| connection_ids.insert(connection.connection_id.clone()))
        .map(|connection| {
            let mut lifecycle = PlanLifecycle::Unknown;
            let mut terminal: Option<ConnectionTerminalDisposition> = None;
            let mut failure_message = None;
            for observation in observations.iter().filter(|observation| {
                observation.connection_id.as_ref() == Some(&connection.connection_id)
            }) {
                match &observation.kind {
                    ObservationKind::ConnectionTerminal { disposition } => {
                        lifecycle = lifecycle_for_terminal(disposition.disposition);
                        terminal = Some(disposition.clone());
                    }
                    ObservationKind::Failure { message, .. } => {
                        lifecycle = PlanLifecycle::Failed;
                        failure_message = message.clone();
                    }
                    _ => {}
                }
            }
            if lifecycle == PlanLifecycle::Unknown
                && play_terminal == Some(TerminalDisposition::Completed)
                && plan.fragments.iter().any(|fragment| {
                    fragment.expected_terminals.iter().any(|terminal| {
                        terminal
                            == &ExpectedTerminal::ConnectionCompleted(
                                connection.connection_id.clone(),
                            )
                    })
                })
            {
                lifecycle = PlanLifecycle::Completed;
            }
            PlayConnectionReport {
                connection_id: connection.connection_id.clone(),
                lifecycle,
                terminal_disposition: terminal,
                pressure: None,
                failure_message,
            }
        })
        .collect()
}

fn lifecycle_for_terminal(disposition: TerminalDisposition) -> PlanLifecycle {
    match disposition {
        TerminalDisposition::Completed => PlanLifecycle::Completed,
        TerminalDisposition::Failed { .. } => PlanLifecycle::Failed,
        TerminalDisposition::Cancelled { .. } => PlanLifecycle::Cancelled,
    }
}

pub fn write_report(path: &Path, snapshot: &ObservatorySnapshot) -> Result<(), String> {
    validate_snapshot(snapshot)?;
    let encoded = serde_json::to_vec_pretty(snapshot).map_err(|error| error.to_string())?;
    let temporary = temporary_path(path);
    fs::write(&temporary, encoded).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn read_report(path: &Path) -> Result<ObservatorySnapshot, String> {
    let encoded = fs::read(path).map_err(|error| error.to_string())?;
    let snapshot = serde_json::from_slice::<ObservatorySnapshot>(&encoded)
        .map_err(|error| error.to_string())?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(temporary)
}
