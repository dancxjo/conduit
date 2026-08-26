//! Shared bounded inspection of the exact renderer realization drawing Patchbay.

use conduit_core::{verify_plan, Plan, PlannedGear};
use conduit_presentation::{Manifestation, ManifestationError, Presentation, RENDERER_KIND};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererSelfInspection {
    pub plan: Plan,
    pub manifestation: Manifestation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererSelfInspectionError {
    InvalidPlan,
    InvalidManifestation(ManifestationError),
    MissingRendererPlacement,
    AmbiguousRendererPlacement,
}

impl core::fmt::Display for RendererSelfInspectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid renderer self-inspection: {self:?}")
    }
}

impl std::error::Error for RendererSelfInspectionError {}

impl RendererSelfInspection {
    pub fn new(
        presentation: &Presentation,
        plan: Plan,
        manifestation: Manifestation,
    ) -> Result<Self, RendererSelfInspectionError> {
        let value = Self {
            plan,
            manifestation,
        };
        value.validate_against(presentation)?;
        Ok(value)
    }

    pub fn validate_against(
        &self,
        presentation: &Presentation,
    ) -> Result<(), RendererSelfInspectionError> {
        if !verify_plan(&self.plan) {
            return Err(RendererSelfInspectionError::InvalidPlan);
        }
        self.manifestation
            .validate_against(presentation, &self.plan)
            .map_err(RendererSelfInspectionError::InvalidManifestation)?;
        self.renderer_placement()?;
        Ok(())
    }

    pub fn renderer_placement(&self) -> Result<&PlannedGear, RendererSelfInspectionError> {
        let mut matches = self
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.placements.iter())
            .filter(|placement| {
                placement.placement_id == self.manifestation.placement_id
                    && placement.kind_id.as_str() == RENDERER_KIND
            });
        let placement = matches
            .next()
            .ok_or(RendererSelfInspectionError::MissingRendererPlacement)?;
        if matches.next().is_some() {
            return Err(RendererSelfInspectionError::AmbiguousRendererPlacement);
        }
        Ok(placement)
    }
}
