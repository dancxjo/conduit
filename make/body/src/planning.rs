//! Exact Form expansion inputs and planning for a Body workset.

use conduit_body::{BodyFormPlan, BodyPlanningSessionError, BodyWorkset, ResidentForm};
use conduit_core::{BaseImplementationId, HostAdvertisement, KindId};
use conduit_form::ExpandedCanonicalForm;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPlanningForm {
    resident: ResidentForm,
    expanded: ExpandedCanonicalForm,
}

impl BodyPlanningForm {
    pub fn new(
        resident: ResidentForm,
        expanded: ExpandedCanonicalForm,
    ) -> Result<Self, BodyPlanningSessionError> {
        if resident.source_document_id != expanded.source_document_id
            || resident.checked_form_id != expanded.checked_form_id
        {
            return Err(BodyPlanningSessionError::InvalidForm(
                "expanded Form identity disagrees with its resident Body identity".into(),
            ));
        }
        Ok(Self { resident, expanded })
    }

    pub fn resident(&self) -> &ResidentForm {
        &self.resident
    }

    pub fn expanded(&self) -> &ExpandedCanonicalForm {
        &self.expanded
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningRequirements {
    pub kind_ids: Vec<KindId>,
}

pub fn body_planning_requirements(
    workset: &BodyWorkset,
    candidates: &[BodyPlanningForm],
) -> Result<BodyPlanningRequirements, BodyPlanningSessionError> {
    let forms = exact_workset_forms(workset, candidates)?;
    let mut kind_ids = forms
        .iter()
        .flat_map(|candidate| {
            candidate
                .expanded
                .gears
                .iter()
                .map(|gear| gear.kind_id.clone())
        })
        .collect::<Vec<_>>();
    kind_ids.sort();
    kind_ids.dedup();
    Ok(BodyPlanningRequirements { kind_ids })
}

pub fn plan_body_workset_on_host(
    workset: &BodyWorkset,
    candidates: &[BodyPlanningForm],
    host: &HostAdvertisement,
    bases: &[BaseImplementationId],
) -> Result<Vec<BodyFormPlan>, BodyPlanningSessionError> {
    exact_workset_forms(workset, candidates)?
        .into_iter()
        .map(|candidate| {
            let hosts = [host.clone()];
            let placements =
                conduit_planner::default_expanded_placements(&candidate.expanded, &hosts)
                    .map_err(|error| BodyPlanningSessionError::Planning(error.to_string()))?;
            let mut limits = std::collections::BTreeMap::new();
            for cord in &candidate.expanded.connections {
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
                &candidate.expanded,
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
                form: candidate.resident.clone(),
                plan,
            })
        })
        .collect()
}

fn exact_workset_forms<'a>(
    workset: &BodyWorkset,
    candidates: &'a [BodyPlanningForm],
) -> Result<Vec<&'a BodyPlanningForm>, BodyPlanningSessionError> {
    workset
        .forms()
        .iter()
        .map(|resident| {
            candidates
                .iter()
                .find(|candidate| &candidate.resident == resident)
                .ok_or(BodyPlanningSessionError::MissingForm)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{resource_offer, INPUT_RESOURCE_CLASS};
    use conduit_form::{
        check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
        StartupCatalog,
    };

    #[test]
    fn canonical_button_body_planning_respects_both_selected_queue_limits() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
            .unwrap();
        let source = include_str!("../../../forms/button-across-room/main.conduit");
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "button_across_room", &profile).unwrap();
        let resident = ResidentForm::new(
            expanded.source_document_id.clone(),
            expanded.checked_form_id.clone(),
        );
        let candidate = BodyPlanningForm::new(resident.clone(), expanded).unwrap();
        let workset = BodyWorkset::one(resident).unwrap();
        let mut host = conduit_std_host::StdHost::new().advertisement().clone();
        host.capabilities = vec![
            conduit_std_offers::button::offer(),
            conduit_std_offers::button::mapper_offer(),
            conduit_std_offers::button::indicator_offer(),
        ];
        host.capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        host.resources
            .push(resource_offer("proof/body-button", INPUT_RESOURCE_CLASS, 1));
        host.resources.sort();

        let plans = plan_body_workset_on_host(
            &workset,
            &[candidate],
            &host,
            &["conduit.base/local@1".into()],
        )
        .unwrap();

        let fragment = &plans[0].plan.fragments[0];
        let indicator_cord = fragment
            .connections
            .iter()
            .find(|cord| cord.value_kind.as_str() == conduit_core::BOOL_INFO_ID)
            .unwrap();
        assert_eq!(indicator_cord.item_capacity, 1);
        assert_eq!(indicator_cord.byte_capacity, 1);
    }

    #[test]
    fn mismatched_resident_and_expanded_identities_are_refused() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
            .unwrap();
        let source = include_str!("../../../forms/button-across-room/main.conduit");
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "button_across_room", &profile).unwrap();
        let resident = ResidentForm::new("source/stale".into(), expanded.checked_form_id.clone());

        assert!(matches!(
            BodyPlanningForm::new(resident, expanded),
            Err(BodyPlanningSessionError::InvalidForm(_))
        ));
    }
}
