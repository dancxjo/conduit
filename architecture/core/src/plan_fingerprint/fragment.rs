//! One canonical fragment encoder, with bounded incremental placement input.
use super::*;
mod parts;
mod placement;
mod tail;
pub use parts::{FragmentFingerprintHeader, FragmentFingerprintTail};

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
        self.push_placement_with_configuration(gear, &gear.configuration)
    }
    /// Canonical representation seam for immutable configuration held separately
    /// from placement metadata. Encodes these entries instead of `gear.configuration`.
    /// This performs no validation, selection or authority admission.
    pub fn push_placement_with_configuration(
        &mut self,
        gear: &crate::PlannedGear,
        configuration: &[crate::ConfigurationEntry],
    ) -> Result<(), FragmentFingerprintRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        if self.remaining == 0 {
            self.refusal = Some(FragmentFingerprintRefusal::UnexpectedPlacement);
            return Err(FragmentFingerprintRefusal::UnexpectedPlacement);
        }
        placement::push_placement(&mut self.canonical, gear, configuration);
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
    push_string(canonical, header.host_id.as_str());
    push_string(canonical, header.boot_id.as_str());
    push_u64(canonical, header.offer_generation.0);
    execution_regions::push_canonical(canonical, header.execution_regions);
    execution_fusion::push_canonical(canonical, header.execution_fusions);
}
