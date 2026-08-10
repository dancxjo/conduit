//! Finite delivery envelope for one portable Conduit Presentation.

use conduit_presentation::Presentation;
use patchbay_model::RendererSelfInspection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSnapshot {
    pub schema: String,
    pub revision: u64,
    pub presentation: Presentation,
    pub renderer: RendererSelfInspection,
    pub interaction: HtmlInteractionState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlInteractionState {
    pub revision: u64,
    pub selected_subject: Option<String>,
    pub last_request_id: Option<String>,
    pub last_disposition: Option<String>,
    pub interaction_plan_id: Option<String>,
    pub interaction_play_id: Option<String>,
}
