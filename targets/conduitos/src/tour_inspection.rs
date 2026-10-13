//! Retained, bounded projection of the successfully executed Tour Plan.
use alloc::{format, string::String, vec::Vec};
use conduit_core::Plan;
use conduit_face::{Face, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject};
use conduit_tour_model::{CANONICAL_PATCHBAY_GEARS, canonical_gear_contract};

use super::{PreparationError, TourProduct};

pub(super) struct RunInspection {
    placements: Vec<GearPlacement>,
    pub(super) observations: crate::text_composition::TextObservations,
}

struct GearPlacement {
    subject: &'static str,
    implementation: String,
    placement: String,
    configured_text: Option<String>,
}

impl RunInspection {
    // The canonical specimen has exactly one occurrence of each of these
    // three Kinds. Refuse ambiguous or expanded Plans instead of guessing.
    pub(super) fn from_plan(plan: &Plan) -> Result<Self, PreparationError> {
        if plan
            .fragments
            .iter()
            .map(|fragment| fragment.placements.len())
            .sum::<usize>()
            != 3
        {
            return Err(PreparationError::PlacementRejected);
        }
        let mut placements = Vec::with_capacity(3);
        for subject in CANONICAL_PATCHBAY_GEARS {
            let contract =
                canonical_gear_contract(subject).ok_or(PreparationError::PlacementRejected)?;
            let mut matching = plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .filter(|placement| placement.kind_id == contract.kind_id);
            let placement = matching.next().ok_or(PreparationError::PlacementRejected)?;
            if matching.next().is_some() {
                return Err(PreparationError::PlacementRejected);
            }
            placements.push(GearPlacement {
                subject,
                implementation: placement.implementation_id.as_str().into(),
                placement: placement.placement_id.as_str().into(),
                configured_text: placement.configuration.iter().find_map(|entry| {
                    if entry.key == "value"
                        && let conduit_core::ConfigurationValue::Text(value) = &entry.value
                    {
                        return Some(value.clone());
                    }
                    None
                }),
            });
        }
        Ok(Self {
            placements,
            observations: Default::default(),
        })
    }
}

impl TourProduct {
    pub fn inspector_face(&self) -> Result<Option<Face>, &'static str> {
        let Some(mut face) = self.controller.state().inspector_face()? else {
            return Ok(None);
        };
        let selected = self
            .controller
            .state()
            .selected_patchbay_subject
            .as_deref()
            .ok_or("inspection-selection-missing")?;
        let graph = self
            .graph
            .as_ref()
            .map_err(|_| "inspection-graph-refused")?;
        face.basis.source_document_id = Some(graph.source_document_id.clone());
        face.basis.checked_plot_id = Some(graph.checked_plot_id.clone());
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id.as_str() == selected)
            .ok_or("inspection-gear-missing")?;
        if let Some(value) =
            gear.controls
                .iter()
                .find_map(|control| match (control.key.as_str(), &control.value) {
                    ("value", conduit_core::ConfigurationValue::Text(value)) => Some(value),
                    _ => None,
                })
        {
            let state_identity = format!("{selected}/inspection/state");
            let state = face
                .text
                .iter_mut()
                .find(|item| item.subject == state_identity)
                .ok_or("inspection-state-missing")?;
            state.text = format!("Configured value: {value:?}\nOutput not directly observed");
        }
        let (Some(snapshot), Some(proof)) = (&self.inspection, self.controller.last_run()) else {
            return validated(face);
        };
        let placement = snapshot
            .placements
            .iter()
            .find(|placement| placement.subject == selected)
            .ok_or("inspection-placement-missing")?;
        for item in &mut face.text {
            if item.subject == format!("{selected}/inspection/implementation") {
                item.text = placement.implementation.clone();
            } else if item.subject == format!("{selected}/inspection/placement") {
                item.text = placement.placement.clone();
            } else if item.subject == format!("{selected}/inspection/play") {
                item.text = format!(
                    "Recorded Plan: {}\nPlay: {}",
                    proof.plan_id.as_str(),
                    proof.active_play_id.as_str()
                );
            } else if item.subject == format!("{selected}/inspection/state") {
                let observed = &snapshot.observations;
                item.text = match selected {
                    "meet-one-gear/change" => format!(
                        "Last run\nin text: {:?}\nout text: {:?}",
                        observed
                            .upper_input
                            .text()
                            .ok_or("inspection-input-unobserved")?,
                        observed
                            .upper_output
                            .text()
                            .ok_or("inspection-output-unobserved")?
                    ),
                    "meet-one-gear/result" => format!(
                        "Last run\nin text: {:?}",
                        observed
                            .face_input
                            .text()
                            .ok_or("inspection-face-unobserved")?
                    ),
                    _ => placement.configured_text.as_ref().map_or_else(
                        || "Literal output not directly observed".into(),
                        |value| {
                            format!("Configured value: {value:?}\nOutput not directly observed")
                        },
                    ),
                };
            }
        }
        face.basis.source_document_id = Some(proof.source_document_id.clone());
        face.basis.checked_plot_id = Some(proof.checked_plot_id.clone());
        // A native Tour run has no Body identity in its receipt. Keep its
        // recorded realization as observed subjects, not an invented embodied basis.
        for (identity, role, label) in [
            (
                proof.expanded_plot_id.as_str(),
                FaceRole::Plot,
                "Recorded expanded Plot",
            ),
            (proof.plan_id.as_str(), FaceRole::Plan, "Recorded Plan"),
            (
                proof.active_play_id.as_str(),
                FaceRole::Play,
                "Recorded Play",
            ),
        ] {
            face.subjects.push(FaceSubject {
                identity: identity.into(),
                role,
                name: label.into(),
            });
            face.relationships.push(FaceRelationship {
                source: format!("{selected}/inspection"),
                target: identity.into(),
                kind: FaceRelationshipKind::Observes,
            });
        }
        validated(face)
    }
}

fn validated(face: Face) -> Result<Option<Face>, &'static str> {
    Face::new(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
    )
    .map(Some)
    .map_err(|_| "inspection-evidence-refused")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrun_literal_inspection_has_checked_configuration_but_no_execution_claim() {
        let mut product = TourProduct::canonical(1);
        product
            .controller
            .request(&conduit_face::ApplicationEvent {
                revision: 1,
                action: conduit_tour_model::OPEN_PATCHBAY_ACTION_ID.into(),
                kind: conduit_face::ApplicationEventKind::Activate,
                value: Vec::new(),
            })
            .unwrap();
        product.select_gear(2, "meet-one-gear/words").unwrap();
        let face = product.inspector_face().unwrap().unwrap();
        assert!(
            face.text.iter().any(
                |item| item.text == "Configured value: \"hello\"\nOutput not directly observed"
            )
        );
        assert!(
            face.text
                .iter()
                .any(|item| item.text == "No Tour result recorded")
        );
        assert!(
            face.subjects
                .iter()
                .all(|subject| !matches!(subject.role, FaceRole::Plan | FaceRole::Play))
        );
        let graph = product.graph.as_ref().unwrap();
        assert_eq!(
            face.basis.source_document_id.as_ref(),
            Some(&graph.source_document_id)
        );
        assert_eq!(
            face.basis.checked_plot_id.as_ref(),
            Some(&graph.checked_plot_id)
        );
        let identity = face.identity.clone();
        assert_eq!(validated(face).unwrap().unwrap().identity, identity);
    }
}
