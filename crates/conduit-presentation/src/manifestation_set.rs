//! Finite admission and collection of planned Presenter realizations.

use conduit_core::{verify_plan, PlacementId, Plan, PlanId};
use serde::{Deserialize, Serialize};

use crate::{
    Manifestation, ManifestationError, Presentation, PresentationContentId, RENDERER_KIND,
};

pub const MAX_PRESENTATION_MANIFESTATIONS: usize = 64;

/// Exact finite Manifestation slots admitted from an immutable Plan before
/// any corresponding Play starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestationAdmission {
    pub plan_id: PlanId,
    pub placement_ids: alloc::vec::Vec<PlacementId>,
}

impl ManifestationAdmission {
    pub fn from_plan(plan: &Plan) -> Result<Self, ManifestationError> {
        if !verify_plan(plan) {
            return Err(ManifestationError::InvalidPlan);
        }
        let mut placement_ids = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.kind_id.as_str() == RENDERER_KIND)
            .map(|placement| placement.placement_id.clone())
            .collect::<alloc::vec::Vec<_>>();
        placement_ids.sort();
        if placement_ids.len() > MAX_PRESENTATION_MANIFESTATIONS {
            return Err(ManifestationError::TooManyManifestations);
        }
        if placement_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ManifestationError::DuplicateManifestation);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            placement_ids,
        })
    }
}

/// A finite collection of independent Presenter realizations for one exact
/// semantic Presentation revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestationSet {
    pub presentation_id: PresentationContentId,
    pub presentation_revision: u64,
    pub manifestations: alloc::vec::Vec<Manifestation>,
}

impl ManifestationSet {
    pub fn new(
        presentation: &Presentation,
        manifestations: alloc::vec::Vec<Manifestation>,
        plan: &Plan,
        admission: &ManifestationAdmission,
    ) -> Result<Self, ManifestationError> {
        presentation
            .validate()
            .map_err(|_| ManifestationError::InvalidPresentation)?;
        if !verify_plan(plan) || admission.plan_id != plan.plan_id {
            return Err(ManifestationError::InvalidPlan);
        }
        if manifestations.len() > MAX_PRESENTATION_MANIFESTATIONS {
            return Err(ManifestationError::TooManyManifestations);
        }
        for (index, manifestation) in manifestations.iter().enumerate() {
            if !admission
                .placement_ids
                .contains(&manifestation.placement_id)
            {
                return Err(ManifestationError::UnadmittedManifestation);
            }
            manifestation.validate_against(presentation, plan)?;
            if manifestations[..index].iter().any(|prior| {
                prior.manifestation_id == manifestation.manifestation_id
                    || prior.placement_id == manifestation.placement_id
            }) {
                return Err(ManifestationError::DuplicateManifestation);
            }
        }
        Ok(Self {
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            manifestations,
        })
    }
}
