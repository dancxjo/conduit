//! Retained, bounded projection of the successfully executed Tour Plan.
use alloc::{format, string::String, vec::Vec};
use conduit_core::Plan;
use conduit_presentation::{
    Presentation, PresentationRelationship, PresentationRelationshipKind, PresentationRole,
    PresentationSubject,
};
use conduit_tour_model::{CANONICAL_PATCHBAY_GEARS, canonical_gear_contract};

use super::{PreparationError, TourProduct};

pub(super) struct RunInspection {
    placements: Vec<GearPlacement>,
}

struct GearPlacement {
    subject: &'static str,
    implementation: String,
    placement: String,
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
            });
        }
        Ok(Self { placements })
    }
}

impl TourProduct {
    pub fn inspector_presentation(&self) -> Result<Option<Presentation>, &'static str> {
        let Some(mut presentation) = self.controller.state().inspector_presentation()? else {
            return Ok(None);
        };
        let (Some(snapshot), Some(proof)) = (&self.inspection, self.controller.last_run()) else {
            return Ok(Some(presentation));
        };
        let selected = self
            .controller
            .state()
            .selected_patchbay_subject
            .as_deref()
            .ok_or("inspection-selection-missing")?;
        let placement = snapshot
            .placements
            .iter()
            .find(|placement| placement.subject == selected)
            .ok_or("inspection-placement-missing")?;
        for item in &mut presentation.text {
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
            }
        }
        presentation.basis.source_document_id = Some(proof.source_document_id.clone());
        presentation.basis.checked_form_id = Some(proof.checked_form_id.clone());
        // A native Tour run has no Body identity in its receipt. Keep its
        // recorded realization as observed subjects, not an invented embodied basis.
        for (identity, role, label) in [
            (
                proof.expanded_form_id.as_str(),
                PresentationRole::Form,
                "Recorded expanded Form",
            ),
            (
                proof.plan_id.as_str(),
                PresentationRole::Plan,
                "Recorded Plan",
            ),
            (
                proof.active_play_id.as_str(),
                PresentationRole::Play,
                "Recorded Play",
            ),
        ] {
            presentation.subjects.push(PresentationSubject {
                identity: identity.into(),
                role,
                label: label.into(),
                accessibility_name: label.into(),
            });
            presentation.relationships.push(PresentationRelationship {
                source: format!("{selected}/inspection"),
                target: identity.into(),
                kind: PresentationRelationshipKind::Observes,
            });
        }
        Presentation::new(
            presentation.revision,
            presentation.basis,
            presentation.subjects,
            presentation.relationships,
            presentation.properties,
            presentation.text,
        )
        .map(Some)
        .map_err(|_| "inspection-evidence-refused")
    }
}
