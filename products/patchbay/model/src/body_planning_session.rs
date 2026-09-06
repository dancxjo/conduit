//! Bounded ordinary Body-wide planning history for the Patchbay product.
//!
//! The session is orchestration, not a second planner or lifecycle. Callers
//! supply ordinary per-Form Plans; `BodyPlan` seals the exact workset and
//! `Wake` owns every accepted, superseded, playing, and unsatisfied state.

use conduit_body::{
    Body, BodyFormPlan, BodyId, BodyLifecycleError, BodyPlan, BodyPlanError, BodyPlayIdentity,
    BodyWorkset, Wake, WakeId, WakeLifecycle,
};
use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, KindId, PlanId, SignId,
};
use serde::{Deserialize, Serialize};

use crate::FormCandidate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningRequirements {
    pub kind_ids: Vec<KindId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPlanningTransition {
    pub unsatisfied_sign_id: Option<SignId>,
    pub plan_ready_sign_id: SignId,
    pub play_sequence: u64,
    pub play_started_sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningSessionSnapshot {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub lifecycle: WakeLifecycle,
    pub current_plan_id: PlanId,
    pub historical_plan_ids: Vec<PlanId>,
    pub current_hosts: Vec<BodyPlanningHost>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyPlanningHost {
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyPlanningSessionError {
    Lifecycle(BodyLifecycleError),
    Plan(BodyPlanError),
    MissingUnsatisfiedSign,
    StaleCurrentPlan,
    MissingForm,
    InvalidForm(String),
    Planning(String),
}

pub fn body_planning_requirements(
    workset: &BodyWorkset,
    candidates: &[FormCandidate],
) -> Result<BodyPlanningRequirements, BodyPlanningSessionError> {
    let expanded = expand_workset(workset, candidates)?;
    let mut kind_ids = expanded
        .iter()
        .flat_map(|(_, form)| form.gears.iter().map(|gear| gear.kind_id.clone()))
        .collect::<Vec<_>>();
    kind_ids.sort();
    kind_ids.dedup();
    Ok(BodyPlanningRequirements { kind_ids })
}

pub fn plan_body_workset_on_host(
    workset: &BodyWorkset,
    candidates: &[FormCandidate],
    host: &HostAdvertisement,
    bases: &[BaseImplementationId],
) -> Result<Vec<BodyFormPlan>, BodyPlanningSessionError> {
    expand_workset(workset, candidates)?
        .into_iter()
        .map(|(resident, expanded)| {
            let hosts = [host.clone()];
            let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
                .map_err(|error| BodyPlanningSessionError::Planning(error.to_string()))?;
            let mut limits = std::collections::BTreeMap::new();
            for cord in &expanded.connections {
                let selected = |gear| {
                    placements
                        .by_gear
                        .get(gear)
                        .and_then(|choice| {
                            host.capabilities
                                .iter()
                                .find(|offer| offer.capability_id == choice.capability_id)
                        })
                        .ok_or_else(|| {
                            BodyPlanningSessionError::Planning(
                                "Body Cord has no exact selected capability".into(),
                            )
                        })
                };
                let source = selected(&cord.source_gear_id)?;
                let sink = selected(&cord.sink_gear_id)?;
                limits.insert(
                    (
                        cord.source_gear_id.clone(),
                        cord.source_port_id.clone(),
                        cord.sink_gear_id.clone(),
                        cord.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: source
                            .limits
                            .max_queue_items
                            .min(sink.limits.max_queue_items)
                            .min(4),
                        byte_capacity: source
                            .limits
                            .max_queue_bytes
                            .min(sink.limits.max_queue_bytes),
                    },
                );
            }
            let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
                &expanded,
                &hosts,
                &placements,
                bases,
                conduit_planner::PlanningOptions {
                    connection_bases: &Default::default(),
                    line_candidates: &Default::default(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 1,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
                &limits,
            )
            .map_err(|error| BodyPlanningSessionError::Planning(error.to_string()))?;
            Ok(BodyFormPlan {
                form: resident,
                plan,
            })
        })
        .collect()
}

fn expand_workset(
    workset: &BodyWorkset,
    candidates: &[FormCandidate],
) -> Result<
    Vec<(
        conduit_body::ResidentForm,
        conduit_form::ExpandedCanonicalForm,
    )>,
    BodyPlanningSessionError,
> {
    workset
        .forms()
        .iter()
        .map(|resident| {
            let candidate = candidates
                .iter()
                .find(|candidate| {
                    candidate.source_document_id == resident.source_document_id
                        && candidate.checked_form_id == resident.checked_form_id
                })
                .ok_or(BodyPlanningSessionError::MissingForm)?;
            let editor = candidate
                .editor()
                .map_err(BodyPlanningSessionError::InvalidForm)?;
            let view = editor.view();
            let name = view
                .checked
                .forms
                .iter()
                .find(|form| form.checked_form_id == resident.checked_form_id)
                .map(|form| form.name.as_str())
                .ok_or_else(|| {
                    BodyPlanningSessionError::InvalidForm("checked Form is absent".into())
                })?;
            let expanded = editor
                .expand_form(name)
                .map_err(|error| BodyPlanningSessionError::InvalidForm(error.to_string()))?;
            Ok((resident.clone(), expanded))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct BodyPlanningSession {
    body: Body,
    wake: Wake,
    plans: Vec<BodyPlan>,
}

impl BodyPlanningSession {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        body: &Body,
        wake_sequence: u64,
        wake_sign_id: SignId,
        forms: Vec<BodyFormPlan>,
        plan_ready_sign_id: SignId,
        play_sequence: u64,
        play_started_sign_id: SignId,
    ) -> Result<Self, BodyPlanningSessionError> {
        let (body, wake) = body
            .wake(wake_sequence, wake_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let plan = BodyPlan::seal(&wake, forms).map_err(BodyPlanningSessionError::Plan)?;
        let wake = wake
            .body_plan_ready(&plan, plan_ready_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let play = BodyPlayIdentity::bind(&plan, play_sequence);
        let wake = wake
            .body_play_started(&plan, &play, play_started_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        Ok(Self {
            body,
            wake,
            plans: vec![plan],
        })
    }

    pub fn replan(
        &mut self,
        forms: Vec<BodyFormPlan>,
        transition: BodyPlanningTransition,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let mut wake = self.wake.clone();
        if wake.lifecycle == WakeLifecycle::Playing {
            let sign = transition
                .unsatisfied_sign_id
                .ok_or(BodyPlanningSessionError::MissingUnsatisfiedSign)?;
            wake = wake
                .became_unsatisfied(&self.current_plan().plan_id, sign)
                .map_err(BodyPlanningSessionError::Lifecycle)?;
        }
        if wake.lifecycle != WakeLifecycle::Unsatisfied {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        let replacement = BodyPlan::seal(&wake, forms).map_err(BodyPlanningSessionError::Plan)?;
        wake = wake
            .body_plan_ready(&replacement, transition.plan_ready_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let play = BodyPlayIdentity::bind(&replacement, transition.play_sequence);
        wake = wake
            .body_play_started(&replacement, &play, transition.play_started_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        self.wake = wake;
        self.plans.push(replacement);
        Ok(self.current_plan())
    }

    pub fn mark_current_unsatisfied(
        &mut self,
        sign_id: SignId,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let plan_id = self.current_plan().plan_id.clone();
        self.wake = self
            .wake
            .became_unsatisfied(&plan_id, sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        Ok(self.current_plan())
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn wake(&self) -> &Wake {
        &self.wake
    }

    pub fn current_plan(&self) -> &BodyPlan {
        self.plans.last().expect("a planning session has a Plan")
    }

    pub fn plan(&self, plan_id: &PlanId) -> Option<&BodyPlan> {
        self.plans.iter().find(|plan| &plan.plan_id == plan_id)
    }

    pub fn snapshot(&self) -> BodyPlanningSessionSnapshot {
        let mut current_hosts = self
            .current_plan()
            .forms
            .iter()
            .flat_map(|form| &form.plan.fragments)
            .map(|fragment| BodyPlanningHost {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
            })
            .collect::<Vec<_>>();
        current_hosts.sort();
        current_hosts.dedup();
        BodyPlanningSessionSnapshot {
            body_id: self.body.body_id.clone(),
            wake_id: self.wake.wake_id.clone(),
            lifecycle: self.wake.lifecycle,
            current_plan_id: self.current_plan().plan_id.clone(),
            historical_plan_ids: self.plans.iter().map(|plan| plan.plan_id.clone()).collect(),
            current_hosts,
        }
    }
}

#[cfg(test)]
mod tests;
