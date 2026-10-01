//! Body-owned runtime eligibility for ordinary Forms serving as Masks.

use alloc::{string::String, vec::Vec};
use conduit_body::{BodyId, WakeId};
use conduit_core::{FormIdentity, PlacementId, PlanId};
use serde::{Deserialize, Serialize};

use crate::{MaskPlanningDisposition, MaskWardrobeError, MaskWardrobeLifetime};

pub const MAX_WORN_MASK_FORMS: usize = 16;
pub const MAX_SEALED_MASK_ROUTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskWardrobe {
    pub revision: u64,
    pub lifetime: MaskWardrobeLifetime,
    pub worn: Vec<FormIdentity>,
    /// Most preferred first. Preference never confers eligibility.
    pub preference: Vec<FormIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyMaskWardrobe {
    pub body_id: BodyId,
    pub wake_id: Option<WakeId>,
    pub wardrobe: MaskWardrobe,
}

/// One realization route already sealed by an immutable ordinary Plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedMaskFormRoute {
    pub route_id: String,
    pub mask_form: FormIdentity,
    pub plan_id: PlanId,
    pub placement_ids: Vec<PlacementId>,
    pub currently_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedMaskFormRoute {
    pub route_id: String,
    pub mask_form: FormIdentity,
    pub plan_id: PlanId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskShowDisposition {
    Retain(SelectedMaskFormRoute),
    SelectSealed {
        prior: Option<SelectedMaskFormRoute>,
        selected: SelectedMaskFormRoute,
    },
    NoCurrentShow {
        prior: Option<SelectedMaskFormRoute>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskReconciliation {
    pub show: MaskShowDisposition,
    pub planning: MaskPlanningDisposition,
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
}

impl MaskWardrobe {
    pub fn new(
        lifetime: MaskWardrobeLifetime,
        worn: Vec<FormIdentity>,
        preference: Vec<FormIdentity>,
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
        basis_revision: u64,
        mask_form: FormIdentity,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(basis_revision)?;
        if self.worn.contains(&mask_form) {
            return Err(MaskWardrobeError::DuplicateMask);
        }
        let mut next = self.clone();
        next.revision += 1;
        next.worn.push(mask_form);
        next.validate()?;
        Ok(next)
    }

    pub fn doff(
        &self,
        basis_revision: u64,
        mask_form: &FormIdentity,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(basis_revision)?;
        let Some(index) = self.worn.iter().position(|worn| worn == mask_form) else {
            return Err(MaskWardrobeError::UnknownMask);
        };
        let mut next = self.clone();
        next.revision += 1;
        next.worn.remove(index);
        next.preference.retain(|preferred| preferred != mask_form);
        next.validate()?;
        Ok(next)
    }

    pub fn prefer(
        &self,
        basis_revision: u64,
        preference: Vec<FormIdentity>,
    ) -> Result<Self, MaskWardrobeError> {
        self.require_revision(basis_revision)?;
        let mut next = self.clone();
        next.revision += 1;
        next.preference = preference;
        next.validate()?;
        Ok(next)
    }

    pub fn reconcile(
        &self,
        active_plan_id: &PlanId,
        routes: &[SealedMaskFormRoute],
        selected: Option<&SelectedMaskFormRoute>,
    ) -> Result<MaskReconciliation, MaskWardrobeError> {
        self.validate()?;
        validate_routes(active_plan_id, routes)?;
        if let Some(selected) = selected {
            let route = routes.iter().find(|route| {
                route.route_id == selected.route_id
                    && route.mask_form == selected.mask_form
                    && route.plan_id == selected.plan_id
            });
            if selected.plan_id != *active_plan_id || route.is_none() {
                return Err(MaskWardrobeError::StaleSelection);
            }
            if self.worn.contains(&selected.mask_form)
                && route.is_some_and(|route| route.currently_available)
            {
                return Ok(MaskReconciliation {
                    show: MaskShowDisposition::Retain(selected.clone()),
                    planning: MaskPlanningDisposition::NotRequired,
                });
            }
        }
        if let Some(route) = self.ordered_worn().find_map(|mask| {
            routes
                .iter()
                .find(|route| &route.mask_form == mask && route.currently_available)
        }) {
            return Ok(MaskReconciliation {
                show: MaskShowDisposition::SelectSealed {
                    prior: selected.cloned(),
                    selected: SelectedMaskFormRoute {
                        route_id: route.route_id.clone(),
                        mask_form: route.mask_form.clone(),
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

    fn ordered_worn(&self) -> impl Iterator<Item = &FormIdentity> {
        self.preference.iter().chain(
            self.worn
                .iter()
                .filter(|mask| !self.preference.contains(mask)),
        )
    }

    fn require_revision(&self, basis_revision: u64) -> Result<(), MaskWardrobeError> {
        (basis_revision == self.revision)
            .then_some(())
            .ok_or(MaskWardrobeError::StaleRevision)
    }

    fn validate(&self) -> Result<(), MaskWardrobeError> {
        if self.worn.len() > MAX_WORN_MASK_FORMS || self.preference.len() > MAX_WORN_MASK_FORMS {
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
    routes: &[SealedMaskFormRoute],
) -> Result<(), MaskWardrobeError> {
    if routes.len() > MAX_SEALED_MASK_ROUTES {
        return Err(MaskWardrobeError::CapacityExceeded);
    }
    for (index, route) in routes.iter().enumerate() {
        if route.plan_id != *active_plan_id
            || route.route_id.is_empty()
            || route.placement_ids.is_empty()
            || has_duplicates(&route.placement_ids)
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

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
