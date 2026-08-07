use conduit_core::{verify_plan, ConnectionId, HostAdvertisement, Observation, PlacementId, Plan};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REPORT_SCHEMA: &str = "conduit.observatory.runtime-report/v1";
const RETAINED_OBSERVATION_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionReport {
    pub item_capacity: u32,
    pub retained_items: u32,
    pub dropped_items: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReportArtifact {
    pub schema: String,
    pub advertisements: Vec<HostAdvertisement>,
    pub plans: Vec<Plan>,
    pub observations: Vec<Observation>,
    pub retention: RetentionReport,
}

impl RuntimeReportArtifact {
    pub fn from_execution(
        advertisements: Vec<HostAdvertisement>,
        plans: Vec<Plan>,
        observations: Vec<Observation>,
    ) -> Self {
        let dropped_items = observations
            .len()
            .saturating_sub(RETAINED_OBSERVATION_CAPACITY);
        let observations = observations
            .into_iter()
            .skip(dropped_items)
            .collect::<Vec<_>>();
        Self {
            schema: REPORT_SCHEMA.to_string(),
            advertisements,
            plans,
            retention: RetentionReport {
                item_capacity: RETAINED_OBSERVATION_CAPACITY as u32,
                retained_items: observations.len() as u32,
                dropped_items: dropped_items as u64,
            },
            observations,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != REPORT_SCHEMA {
            return Err(format!(
                "unsupported runtime report schema: {}",
                self.schema
            ));
        }
        if self.retention.item_capacity != RETAINED_OBSERVATION_CAPACITY as u32 {
            return Err("runtime report retention capacity does not match its schema".to_string());
        }
        if self.retention.retained_items != self.observations.len() as u32
            || self.retention.retained_items > self.retention.item_capacity
        {
            return Err("runtime report retention accounting is invalid".to_string());
        }

        let mut host_boots = BTreeSet::new();
        for advertisement in &self.advertisements {
            if !host_boots.insert((advertisement.host_id.clone(), advertisement.boot_id.clone())) {
                return Err("duplicate host/boot report".to_string());
            }
        }
        let mut plan_ids = BTreeSet::new();
        for plan in &self.plans {
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
        let mut evidence_ids = BTreeSet::new();

        for observation in &self.observations {
            if !host_boots.contains(&(observation.host_id.clone(), observation.boot_id.clone())) {
                return Err(format!(
                    "observation {} names an unreported host/boot",
                    observation.evidence_id.as_str()
                ));
            }
            if observation
                .plan_id
                .as_ref()
                .is_some_and(|plan_id| !plan_ids.contains(plan_id))
            {
                return Err(format!(
                    "observation {} names an unreported plan",
                    observation.evidence_id.as_str()
                ));
            }
            if observation.presentation_id.is_some() && observation.active_play_id.is_none() {
                return Err(format!(
                    "observation {} has a presentation without an active play",
                    observation.evidence_id.as_str()
                ));
            }
            if let (Some(plan_id), Some(placement_id)) =
                (&observation.plan_id, &observation.placement_id)
            {
                let plan = self
                    .plans
                    .iter()
                    .find(|plan| &plan.plan_id == plan_id)
                    .expect("plan identity membership checked above");
                if !plan_contains_placement(plan, placement_id) {
                    return Err(format!(
                        "observation {} names an unreported placement",
                        observation.evidence_id.as_str()
                    ));
                }
            }
            if let (Some(plan_id), Some(connection_id)) =
                (&observation.plan_id, &observation.connection_id)
            {
                let plan = self
                    .plans
                    .iter()
                    .find(|plan| &plan.plan_id == plan_id)
                    .expect("plan identity membership checked above");
                if !plan_contains_connection(plan, connection_id) {
                    return Err(format!(
                        "observation {} names an unreported connection",
                        observation.evidence_id.as_str()
                    ));
                }
            }
            if !evidence_ids.insert(observation.evidence_id.clone()) {
                return Err(format!(
                    "duplicate evidence identity {}",
                    observation.evidence_id.as_str()
                ));
            }
        }

        Ok(())
    }
}

fn plan_contains_placement(plan: &Plan, placement_id: &PlacementId) -> bool {
    plan.fragments.iter().any(|fragment| {
        fragment
            .placements
            .iter()
            .any(|placement| &placement.placement_id == placement_id)
    })
}

fn plan_contains_connection(plan: &Plan, connection_id: &ConnectionId) -> bool {
    plan.fragments.iter().any(|fragment| {
        fragment
            .connections
            .iter()
            .any(|connection| &connection.connection_id == connection_id)
    })
}

pub fn write_report(path: &Path, artifact: &RuntimeReportArtifact) -> Result<(), String> {
    artifact.validate()?;
    let encoded = serde_json::to_vec_pretty(artifact).map_err(|error| error.to_string())?;
    let temporary = temporary_path(path);
    fs::write(&temporary, encoded).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

pub fn read_report(path: &Path) -> Result<RuntimeReportArtifact, String> {
    let encoded = fs::read(path).map_err(|error| error.to_string())?;
    let artifact = serde_json::from_slice::<RuntimeReportArtifact>(&encoded)
        .map_err(|error| error.to_string())?;
    artifact.validate()?;
    Ok(artifact)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(temporary)
}
