//! Producer-owned observations from the canonical screen-free Mask journey.
//!
//! These records describe semantic, planning, and output-effect truth. A spoken
//! artifact receipt is deliberately not evidence that a person heard the Show
//! or that room audio was captured and correlated back through the Mask.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskJourneyObservation {
    pub action_id: String,
    pub concrete_event: String,
    pub presentation_id: String,
    pub selected_mask_form_id: Option<String>,
    pub plan_id: String,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

impl SpokenMaskJourneyObservation {
    pub fn action(&self) -> Option<conduit_presentation::MaskJourneyAction> {
        conduit_presentation::MASK_JOURNEY_ACTIONS
            .into_iter()
            .find(|action| action.id() == self.action_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskJourneyEvidence {
    pub schema: String,
    pub observations: Vec<SpokenMaskJourneyObservation>,
    /// Always false for artifact-only proof. Human hearing requires separate
    /// attended evidence; physical re-observation requires capture + correlation.
    pub human_hearing_observed: bool,
    pub inward_face_interaction_observed: bool,
}

impl SpokenMaskJourneyEvidence {
    pub const SCHEMA: &'static str = "conduit.std/spoken-mask-journey@1";

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != Self::SCHEMA {
            return Err("unknown spoken Mask journey evidence schema");
        }
        if self.observations.len() != conduit_presentation::MASK_JOURNEY_ACTIONS.len() {
            return Err("spoken Mask journey does not contain ten observations");
        }
        for (observation, action) in self
            .observations
            .iter()
            .zip(conduit_presentation::MASK_JOURNEY_ACTIONS)
        {
            if observation.action_id != action.id() {
                return Err("spoken Mask journey observations are out of order");
            }
        }
        let Some(presentation) = self.observations.first().map(|item| &item.presentation_id) else {
            return Err("spoken Mask journey omitted its Presentation");
        };
        if self
            .observations
            .iter()
            .any(|item| &item.presentation_id != presentation)
        {
            return Err("spoken Mask journey changed Presentation identity");
        }
        if self.human_hearing_observed || self.inward_face_interaction_observed {
            return Err("artifact-only spoken Mask proof claimed sensory observation");
        }
        Ok(())
    }
}
