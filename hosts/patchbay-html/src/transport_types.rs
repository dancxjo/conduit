//! Finite delivery envelope for one portable Conduit Presentation.

use conduit_presentation::Presentation;
use patchbay_model::{
    PartsView, PatchbayEntranceState, PatchbayNavigationProjection, RendererSelfInspection,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSnapshot {
    pub schema: String,
    pub revision: u64,
    pub presentation: Presentation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<PatchbayNavigationProjection>,
    pub renderer: RendererSelfInspection,
    pub entrance: PatchbayEntranceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<PartsView>,
    pub interaction: HtmlInteractionState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HtmlInteractionState {
    pub revision: u64,
    pub selected_subject: Option<String>,
    pub last_request_id: Option<String>,
    pub last_disposition: Option<String>,
    pub interaction_plan_id: Option<String>,
    pub interaction_play_id: Option<String>,
    pub selected_part: Option<String>,
    pub selected_candidate: Option<String>,
    pub parts_disposition: Option<String>,
    pub parts_feedback: Option<String>,
}
