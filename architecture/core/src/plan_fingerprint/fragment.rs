//! One canonical fragment encoder, with bounded incremental placement input.
use super::*;
mod parts;
mod placement;
#[path = "fragment/recipe.rs"]
mod recipe;
mod tail;
use crate::canonical_bytes::{FragmentScopeField as Field, FragmentScopeRole as Role};
pub use parts::{FragmentFingerprintHeader, FragmentFingerprintTail};
pub use recipe::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentFingerprintRefusal {
    PlacementCountOverflow,
    UnexpectedPlacement,
    IncompletePlacements,
}

/// Canonical encoding only: no selection, sorting, semantic admission or effect
/// authority. Supply records in the exact canonical order selected by planning.
/// Header/placement input is borrowed only for each call, so an admitted
/// preparation path can retire each materialized placement immediately afterward.
/// A refused extra record poisons finalization; partial input has no fingerprint.
pub struct FragmentFingerprintBuilder {
    canonical: CanonicalHash,
    remaining: usize,
    refusal: Option<FragmentFingerprintRefusal>,
}
impl FragmentFingerprintBuilder {
    pub fn new(header: FragmentFingerprintHeader<'_>, placements: usize) -> Self {
        let mut canonical = CanonicalHash::new();
        push_header(&mut canonical, header);
        let count = u32::try_from(placements);
        if let Ok(count) = count {
            push_u32(&mut canonical, count);
        }
        Self {
            canonical,
            remaining: placements,
            refusal: count
                .err()
                .map(|_| FragmentFingerprintRefusal::PlacementCountOverflow),
        }
    }
    pub fn push_placement(
        &mut self,
        gear: &crate::PlannedGear,
    ) -> Result<(), FragmentFingerprintRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        if self.remaining == 0 {
            self.refusal = Some(FragmentFingerprintRefusal::UnexpectedPlacement);
            return Err(FragmentFingerprintRefusal::UnexpectedPlacement);
        }
        placement::push_placement(&mut self.canonical, gear);
        self.remaining -= 1;
        Ok(())
    }
    pub fn finish(
        self,
        tail: FragmentFingerprintTail<'_>,
    ) -> Result<FragmentId, FragmentFingerprintRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        if self.remaining != 0 {
            return Err(FragmentFingerprintRefusal::IncompletePlacements);
        }
        let mut canonical = self.canonical;
        tail::push_tail(&mut canonical, tail);
        Ok(FragmentId::from(crate::hex_digest(&canonical.finish())))
    }
}
pub fn compute_fragment_id(fragment: &PlanFragment) -> FragmentId {
    let mut builder = FragmentFingerprintBuilder::new(fragment.into(), fragment.placements.len());
    for gear in &fragment.placements {
        builder
            .push_placement(gear)
            .expect("count borrowed from this same immutable fragment");
    }
    builder
        .finish(fragment.into())
        .expect("complete placement sequence")
}
fn push_header(canonical: &mut impl CanonicalSink, header: FragmentFingerprintHeader<'_>) {
    push_string(canonical, header.source_document_id.as_str());
    push_string(canonical, header.checked_plot_id.as_str());
    push_string(canonical, header.expanded_plot_id.as_str());
    canonical.push(header.completion_policy as u8);
    if !header.realization_backs.is_empty() {
        plan_realization::push_canonical(canonical, header.realization_backs);
    }
    canonical.scope_string(Field::Header(Role::Host), header.host_id.as_str());
    canonical.scope_string(Field::Header(Role::Boot), header.boot_id.as_str());
    canonical.scope_u64(
        Field::Header(Role::OfferGeneration),
        header.offer_generation.0,
    );
    execution_regions::push_canonical(canonical, header.execution_regions);
    execution_fusion::push_canonical(canonical, header.execution_fusions);
}

mod recipe_identity;
pub use recipe_identity::fragment_seal_recipe_identity;
#[path = "fragment/resource_recipe.rs"]
mod resource_recipe;
pub use resource_recipe::fragment_seal_resource_matches;
