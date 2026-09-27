//! Inspectable runtime eligibility and immutable-Plan Mask reconciliation.

use alloc::{string::String, vec::Vec};
use conduit_body::{BodyId, WakeId};
use conduit_core::PlanId;
use serde::{Deserialize, Serialize};

use crate::{MaskSpecificationId, MaskStageId, MAX_MASK_STAGES};

pub const MAX_WORN_MASKS: usize = 16;
pub const MAX_SEALED_MASK_ROUTES: usize = 32;
pub const MAX_MASK_ROUTE_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskWardrobeLifetime {
    Wake,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskWardrobe {
    pub revision: u64,
    pub lifetime: MaskWardrobeLifetime,
    pub worn: Vec<MaskSpecificationId>,
    /// Most preferred first. Every entry must also be worn. Worn Masks absent
    /// from this list retain eligibility after explicitly preferred Masks.
    pub preference: Vec<MaskSpecificationId>,
}

/// Exact Body-owned runtime configuration scope. Wake-scoped wardrobes bind
/// one Wake; Body-scoped wardrobes deliberately survive Wake replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyMaskWardrobe {
    pub body_id: BodyId,
    pub wake_id: Option<WakeId>,
    pub wardrobe: MaskWardrobe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedMaskRoute {
    pub route_id: String,
    pub specification_id: MaskSpecificationId,
    pub plan_id: PlanId,
    /// Ordered stage path already sealed into the immutable Plan.
    pub stage_ids: Vec<MaskStageId>,
    pub currently_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedMaskRoute {
    pub route_id: String,
    pub specification_id: MaskSpecificationId,
    pub plan_id: PlanId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskShowDisposition {
    Retain(SelectedMaskRoute),
    SelectSealed {
        prior: Option<SelectedMaskRoute>,
        selected: SelectedMaskRoute,
    },
    NoCurrentShow {
        prior: Option<SelectedMaskRoute>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskPlanningDisposition {
    NotRequired,
    /// An authorized control loop may request replacement planning. This fact
    /// is deliberately not itself planning or deployment authority.
    ReplacementRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskReconciliation {
    pub show: MaskShowDisposition,
    pub planning: MaskPlanningDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskWardrobeError {
    StaleRevision,
    CapacityExceeded,
    DuplicateMask,
    UnknownPreference,
    InvalidRoute,
    RouteCapacityExceeded,
    DuplicateRoute,
    StaleSelection,
    InvalidLifetimeScope,
}

impl BodyMaskWardrobe {
    pub fn new(
        body_id: BodyId,
        wake_id: Option<WakeId>,
        wardrobe: MaskWardrobe,
    ) -> Result<Self, MaskWardrobeError> {
        wardrobe.validate()?;
        if matches!(wardrobe.lifetime, MaskWardrobeLifetime::Wake) != wake_id.is_some() {
            return Err(MaskWardrobeError::InvalidLifetimeScope);
        }
        Ok(Self {
            body_id,
            wake_id,
            wardrobe,
        })
    }

    pub fn wear(
        &self,
        offered_revision: u64,
        specification_id: MaskSpecificationId,
    ) -> Result<Self, MaskWardrobeError> {
        let mut next = self.clone();
        next.wardrobe = self.wardrobe.wear(offered_revision, specification_id)?;
        Ok(next)
    }

    pub fn doff(
        &self,
        offered_revision: u64,
        specification_id: &MaskSpecificationId,
    ) -> Result<Self, MaskWardrobeError> {
        let mut next = self.clone();
        next.wardrobe = self.wardrobe.doff(offered_revision, specification_id)?;
        Ok(next)
    }

    pub fn prefer(
        &self,
        offered_revision: u64,
        preference: Vec<MaskSpecificationId>,
    ) -> Result<Self, MaskWardrobeError> {
        let mut next = self.clone();
        next.wardrobe = self.wardrobe.prefer(offered_revision, preference)?;
        Ok(next)
    }
}

impl MaskWardrobe {
    pub fn new(
        lifetime: MaskWardrobeLifetime,
        worn: Vec<MaskSpecificationId>,
        preference: Vec<MaskSpecificationId>,
    ) -> Result<Self, MaskWardrobeError> {
        let wardrobe = Self {
            revision: 0,
            lifetime,
            worn,
            preference,
        };
        wardrobe.validate()?;
        Ok(wardrobe)
    }

    pub fn wear(
        &self,
        offered_revision: u64,
        specification_id: MaskSpecificationId,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(offered_revision)?;
        if self.worn.contains(&specification_id) {
            return Err(MaskWardrobeError::DuplicateMask);
        }
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        next.worn.push(specification_id);
        next.validate()?;
        Ok(next)
    }

    pub fn doff(
        &self,
        offered_revision: u64,
        specification_id: &MaskSpecificationId,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(offered_revision)?;
        let Some(index) = self.worn.iter().position(|worn| worn == specification_id) else {
            return Err(MaskWardrobeError::UnknownPreference);
        };
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        next.worn.remove(index);
        next.preference.retain(|mask| mask != specification_id);
        next.validate()?;
        Ok(next)
    }

    pub fn prefer(
        &self,
        offered_revision: u64,
        preference: Vec<MaskSpecificationId>,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(offered_revision)?;
        let mut next = self.clone();
        next.revision = next.revision.saturating_add(1);
        next.preference = preference;
        next.validate()?;
        Ok(next)
    }

    pub fn reconcile(
        &self,
        active_plan_id: &PlanId,
        routes: &[SealedMaskRoute],
        selected: Option<&SelectedMaskRoute>,
    ) -> Result<MaskReconciliation, MaskWardrobeError> {
        self.validate()?;
        validate_routes(active_plan_id, routes)?;
        if let Some(selected) = selected {
            if &selected.plan_id != active_plan_id
                || !routes.iter().any(|route| {
                    route.route_id == selected.route_id
                        && route.specification_id == selected.specification_id
                })
            {
                return Err(MaskWardrobeError::StaleSelection);
            }
            if self.worn.contains(&selected.specification_id)
                && routes
                    .iter()
                    .any(|route| route.route_id == selected.route_id && route.currently_available)
            {
                return Ok(MaskReconciliation {
                    show: MaskShowDisposition::Retain(selected.clone()),
                    planning: MaskPlanningDisposition::NotRequired,
                });
            }
        }

        let selected_route = self.ordered_worn().find_map(|mask| {
            routes
                .iter()
                .find(|route| &route.specification_id == mask && route.currently_available)
        });
        if let Some(route) = selected_route {
            return Ok(MaskReconciliation {
                show: MaskShowDisposition::SelectSealed {
                    prior: selected.cloned(),
                    selected: SelectedMaskRoute {
                        route_id: route.route_id.clone(),
                        specification_id: route.specification_id.clone(),
                        plan_id: route.plan_id.clone(),
                    },
                },
                planning: MaskPlanningDisposition::NotRequired,
            });
        }
        Ok(MaskReconciliation {
            show: MaskShowDisposition::NoCurrentShow {
                prior: selected.cloned(),
            },
            planning: if self.worn.is_empty() {
                MaskPlanningDisposition::NotRequired
            } else {
                MaskPlanningDisposition::ReplacementRequired
            },
        })
    }

    fn ordered_worn(&self) -> impl Iterator<Item = &MaskSpecificationId> {
        self.preference.iter().chain(
            self.worn
                .iter()
                .filter(|mask| !self.preference.contains(mask)),
        )
    }

    fn require_revision(&self, offered_revision: u64) -> Result<(), MaskWardrobeError> {
        if offered_revision == self.revision {
            Ok(())
        } else {
            Err(MaskWardrobeError::StaleRevision)
        }
    }

    fn validate(&self) -> Result<(), MaskWardrobeError> {
        if self.worn.len() > MAX_WORN_MASKS || self.preference.len() > MAX_WORN_MASKS {
            return Err(MaskWardrobeError::CapacityExceeded);
        }
        if has_duplicates(&self.worn) || has_duplicates(&self.preference) {
            return Err(MaskWardrobeError::DuplicateMask);
        }
        if self
            .preference
            .iter()
            .any(|preferred| !self.worn.contains(preferred))
        {
            return Err(MaskWardrobeError::UnknownPreference);
        }
        Ok(())
    }
}

fn validate_routes(
    active_plan_id: &PlanId,
    routes: &[SealedMaskRoute],
) -> Result<(), MaskWardrobeError> {
    if routes.len() > MAX_SEALED_MASK_ROUTES {
        return Err(MaskWardrobeError::RouteCapacityExceeded);
    }
    for (index, route) in routes.iter().enumerate() {
        if route.plan_id != *active_plan_id
            || route.route_id.is_empty()
            || route.route_id.len() > MAX_MASK_ROUTE_IDENTITY_BYTES
            || route.stage_ids.is_empty()
            || route.stage_ids.len() > MAX_MASK_STAGES
            || route
                .stage_ids
                .iter()
                .enumerate()
                .any(|(stage_index, stage)| route.stage_ids[stage_index + 1..].contains(stage))
        {
            return Err(MaskWardrobeError::InvalidRoute);
        }
        if routes[index + 1..]
            .iter()
            .any(|other| other.route_id == route.route_id)
        {
            return Err(MaskWardrobeError::DuplicateRoute);
        }
    }
    Ok(())
}

fn has_duplicates(values: &[MaskSpecificationId]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
