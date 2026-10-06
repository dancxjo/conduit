//! Finite delivery envelope for one portable Conduit Presentation.

use conduit_presentation::{ModelTemporalContextFact, Presentation, RendererSelfInspection};
use patchbay_application::{
    DebuggerPresentation, PartsView, PatchbayEntranceState, PatchbayNavigationProjection,
};
use serde::{Deserialize, Serialize};

pub const MAX_BROWSER_PALETTE_ENTRIES: usize = conduit_semantic_catalog::MAX_PALETTE_ENTRIES;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserPalettePort {
    pub identity: String,
    pub info: String,
    pub temporal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserPaletteConfiguration {
    pub key: String,
    pub default_value: conduit_core::ConfigurationValue,
    pub rule: conduit_semantic_catalog::KindConfigurationRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserPaletteEntry {
    pub kind_id: String,
    pub kind_contract_revision: conduit_core::KindIdentity,
    pub front: conduit_core::CheckedFront,
    pub limits: conduit_core::CapabilityLimits,
    pub authorable: bool,
    pub startup_parameters: Vec<conduit_patchbay_workbench::AuthoringStartupParameter>,
    pub name: String,
    pub summary: String,
    pub category: String,
    pub tags: Vec<String>,
    pub icon: String,
    pub inputs: Vec<BrowserPalettePort>,
    pub outputs: Vec<BrowserPalettePort>,
    pub configuration: Vec<BrowserPaletteConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserAuthoring {
    pub source_document_id: String,
    pub checked_plot_id: String,
    pub source_revision: u64,
    pub saved_revision: u64,
    pub expanded_plot_id: String,
    pub source_path: String,
    pub palette: Vec<BrowserPaletteEntry>,
    pub configuration: Vec<BrowserGearConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserGearConfiguration {
    pub gear_identity: String,
    pub key: String,
    pub value: conduit_core::ConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSnapshot {
    pub schema: String,
    pub revision: u64,
    pub presentation: Presentation,
    #[serde(default)]
    pub temporal_context: Vec<ModelTemporalContextFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<PatchbayNavigationProjection>,
    pub renderer: RendererSelfInspection,
    pub entrance: PatchbayEntranceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<PartsView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authoring: Option<BrowserAuthoring>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_workbench: Option<BrowserBodyWorkbench>,
    /// Current, display-only evidence reported by an admitted host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_host_offer_evidence: Option<conduit_body::HostOfferProjection>,
    /// Explicitly policy-admitted current offer detail; not itself a plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_host_planning_offer: Option<conduit_body::HostOfferProjection>,
    /// Ordinary body/Wake/Plan state derived from policy-admitted offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_planning: Option<conduit_body::BodyPlanningSessionSnapshot>,
    /// Portable semantic projection of the selected body Mask chains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_topology: Option<Presentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debugger: Option<DebuggerPresentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watches: Option<patchbay_application::DebuggerWatchSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline: Option<patchbay_application::DebuggerTimeline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_projection: Option<patchbay_application::DebuggerTimelineProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debugger_control: Option<patchbay_application::DebuggerExecutionControl>,
    pub interaction: HtmlInteractionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserBodyWorkbench {
    pub schema: String,
    pub evidence_revision: u64,
    pub encoded_evidence: Vec<u8>,
    pub entrance: BrowserBodyWorkbenchEntrance,
    pub body_id: String,
    #[serde(default)]
    pub reviewed_plots: Vec<BrowserReviewedPlot>,
    pub current: serde_json::Value,
    pub history: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserReviewedPlot {
    pub label: String,
    pub source_document_id: String,
    pub checked_plot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", deny_unknown_fields)]
pub enum BrowserBodyWorkbenchEntrance {
    Hosted {
        plan_id: String,
        implementation_id: String,
    },
    ExternalReader,
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
