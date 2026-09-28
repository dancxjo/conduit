//! Form expansion and planner composition for Body-owned planning sessions.

use conduit_body::{BodyFormPlan, BodyWorkset};
use conduit_core::{BaseImplementationId, HostAdvertisement, KindId};
use serde::{Deserialize, Serialize};

use crate::FormCandidate;

pub use conduit_body::{
    BodyExecutionClaim, BodyExecutionClaimError, BodyExecutionPhase, BodyPlanningHost,
    BodyPlanningSession, BodyPlanningSessionError, BodyPlanningSessionSnapshot,
    BodyPlanningTransition,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningRequirements {
    pub kind_ids: Vec<KindId>,
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
                    BodyPlanningSessionError::InvalidForm("checked form is absent".into())
                })?;
            let expanded = editor
                .expand_form(name)
                .map_err(|error| BodyPlanningSessionError::InvalidForm(error.to_string()))?;
            Ok((resident.clone(), expanded))
        })
        .collect()
}

#[cfg(test)]
mod execution_tests;
#[cfg(test)]
mod tests;
