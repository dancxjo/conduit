//! Checked finite spoken expression; Face origins, not application state or voice policy.
use alloc::{string::String, vec::Vec};
use conduit_core::{PlanId, PlotIdentity};
use serde::{Deserialize, Serialize};

#[path = "spoken_template_looks/selection.rs"]
mod selection;
#[path = "spoken_template_looks/source.rs"]
mod source;
pub use selection::{prepare_spoken_template_slots, SpokenTemplateSlots};
pub(crate) use source::expected_spoken_template_wording;
pub use source::{prepare_checked_spoken_template, PreparedSpokenTemplate};

pub const SPOKEN_TEMPLATE_LOOKS_SOURCE: &str =
    include_str!("../components/spoken-template-looks.conduit");
pub const MAX_SPOKEN_TEMPLATE_BYTES: usize = 8192;
pub const MAX_SPOKEN_TEMPLATE_SLOT_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpokenTemplateRefusal {
    Source,
    Shape,
    Selection,
    StaleFace,
    Disclosure,
    MissingEssential,
    Bound,
    Evaluation,
}

/// Inspectable source-to-wording evidence. This is not a Play, Show or audio receipt.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SpokenTemplateWording {
    pub template_identity: String,
    pub template_revision: u16,
    pub template_source_digest: [u8; 32],
    pub producer_plot: PlotIdentity,
    pub producer_plan_id: PlanId,
    pub source_face_identity: String,
    pub source_face_revision: u64,
    pub utterance_plan_digest: [u8; 32],
    pub selected_clause_indices: Vec<u32>,
    pub selected_action_ids: Vec<String>,
    pub segments: Vec<String>,
    pub wording: String,
    pub wording_digest: [u8; 32],
}

#[cfg(test)]
#[path = "spoken_template_looks/tests.rs"]
mod tests;
