//! Finite delivery envelope for one portable Conduit Presentation.

use conduit_core::Plan;
use conduit_presentation::{Manifestation, Presentation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSnapshot {
    pub schema: String,
    pub revision: u64,
    pub presentation: Presentation,
    pub renderer_plan: Plan,
    pub manifestation: Manifestation,
}
