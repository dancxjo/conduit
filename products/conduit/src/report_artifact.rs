use conduit_core::{
    ConnectionTerminalDisposition, ExpectedTerminal, HostAdvertisement, LineOffer, Observation,
    ObservationKind, Plan, TerminalDisposition,
};
use conduit_observatory::{
    validate_plan_artifact, validate_play_artifact, validate_sign_artifact, validate_snapshot,
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport, LineReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, PlanArtifact, PlanLifecycle,
    PlayArtifact, PlayConnectionReport, PlayPlacementReport, PlayReport, RetentionReport,
    SignArtifact, PLAN_ARTIFACT_SCHEMA, PLAY_ARTIFACT_SCHEMA, SIGN_ARTIFACT_SCHEMA,
    SNAPSHOT_SCHEMA,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const RETAINED_OBSERVATION_CAPACITY: usize = 256;

pub fn snapshot_from_execution(
    advertisements: Vec<HostAdvertisement>,
    line_offers: Vec<LineOffer>,
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
            devices: Vec::new(),
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
    let lines = line_offers
        .into_iter()
        .map(|offer| LineReport {
            offer,
            state: OperationalState::Available,
        })
        .collect();
    let plays = plays_from_observations(&plans, &observations);
    ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.to_string(),
        hosts,
        bases: Vec::new(),
        lines,
        plans,
        plays,
        retention: RetentionReport {
            item_capacity: RETAINED_OBSERVATION_CAPACITY as u32,
            retained_items: observations.len() as u32,
            dropped_items: dropped_items as u64,
        },
        observations,
        historical_observations: Vec::new(),
        sealed_boot_provenance: Vec::new(),
    }
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
            ObservationKind::PlanPlayStarted => lifecycle = PlanLifecycle::Active,
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

pub fn write_execution_artifacts(
    directory: &Path,
    snapshot: &ObservatorySnapshot,
    active_plays: &[conduit_core::ActivePlayIdentity],
    sign_identities: &[conduit_core::SignIdentity],
) -> Result<(), String> {
    validate_snapshot(snapshot)?;
    if directory.exists() {
        return Err(format!(
            "execution artifact destination already exists: {}",
            directory.display()
        ));
    }
    let plans = snapshot
        .plans
        .iter()
        .cloned()
        .map(|plan| PlanArtifact {
            schema: PLAN_ARTIFACT_SCHEMA.into(),
            plan,
        })
        .collect::<Vec<_>>();
    for artifact in &plans {
        validate_plan_artifact(artifact)?;
    }
    let plays = active_plays
        .iter()
        .map(|identity| {
            let plan = snapshot
                .plans
                .iter()
                .find(|plan| plan.plan_id == identity.plan_id)
                .ok_or("retained Play identity lacks its exact Plan")?;
            let play = snapshot
                .plays
                .iter()
                .find(|play| play.active_play_id == identity.active_play_id)
                .ok_or("retained Play identity lacks its exact report")?;
            let artifact = PlayArtifact {
                schema: PLAY_ARTIFACT_SCHEMA.into(),
                identity: identity.clone(),
                plan: plan.clone(),
                play: play.clone(),
            };
            validate_play_artifact(&artifact)?;
            Ok(artifact)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let signs = sign_identities
        .iter()
        .map(|identity| {
            let sign = snapshot
                .observations
                .iter()
                .chain(&snapshot.historical_observations)
                .find(|sign| sign.sign_id == identity.sign_id)
                .ok_or("retained Sign identity lacks its exact observation")?;
            let play = identity
                .active_play_id
                .as_ref()
                .map(|active_play_id| {
                    plays
                        .iter()
                        .find(|play| &play.identity.active_play_id == active_play_id)
                        .cloned()
                        .ok_or("retained Sign identity lacks its exact Play artifact")
                })
                .transpose()?;
            let plan = if play.is_none() {
                sign.plan_id
                    .as_ref()
                    .map(|plan_id| {
                        snapshot
                            .plans
                            .iter()
                            .find(|plan| &plan.plan_id == plan_id)
                            .cloned()
                            .ok_or("retained Sign identity lacks its exact Plan artifact")
                    })
                    .transpose()?
            } else {
                None
            };
            let artifact = SignArtifact {
                schema: SIGN_ARTIFACT_SCHEMA.into(),
                identity: identity.clone(),
                plan,
                play,
                sign: sign.clone(),
            };
            validate_sign_artifact(&artifact)?;
            Ok(artifact)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if signs.len()
        != snapshot
            .observations
            .len()
            .saturating_add(snapshot.historical_observations.len())
    {
        return Err("not every retained Sign has an exact standalone identity".into());
    }

    let temporary = temporary_path(directory);
    fs::create_dir(&temporary).map_err(|error| error.to_string())?;
    let result = (|| {
        for artifact in &plans {
            write_json_artifact(
                &temporary.join(format!("plan-{}.json", artifact.plan.plan_id.as_str())),
                artifact,
            )?;
        }
        for artifact in &plays {
            write_json_artifact(
                &temporary.join(format!(
                    "play-{}.json",
                    artifact.identity.active_play_id.as_str()
                )),
                artifact,
            )?;
        }
        for artifact in &signs {
            write_json_artifact(
                &temporary.join(format!("sign-{}.json", artifact.identity.sign_id.as_str())),
                artifact,
            )?;
        }
        fs::rename(&temporary, directory).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

fn write_json_artifact(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let mut encoded = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    encoded.push(b'\n');
    fs::write(path, encoded).map_err(|error| error.to_string())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(temporary)
}
