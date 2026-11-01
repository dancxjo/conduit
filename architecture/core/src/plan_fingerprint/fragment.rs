//! One canonical fragment encoder, with bounded incremental placement input.
use super::*;
mod parts;
mod placement;
mod tail;
pub use parts::{
    FragmentFingerprintHeader, FragmentFingerprintHeaderRef, FragmentFingerprintTail,
    FragmentFingerprintTailRef,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentFingerprintRefusal {
    PlacementCountOverflow,
    UnexpectedPlacement,
    IncompletePlacements,
}

/// Canonical validation is distinct from descriptive output allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentFingerprintPreparationRefusal {
    Fingerprint(FragmentFingerprintRefusal),
    Capacity,
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
        Self::new_ref(header.into(), placements)
    }
    /// Begin descriptive borrowed encoding; no structural check or authority.
    pub fn new_ref(header: FragmentFingerprintHeaderRef<'_>, placements: usize) -> Self {
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
        let mut view = crate::PlannedGearRef::from(gear);
        view.configuration = configuration;
        self.push_placement_ref(view)
    }
    /// Hash a complete borrowed description. This creates no verification or authority.
    pub fn push_placement_ref(
        &mut self,
        gear: crate::PlannedGearRef<'_>,
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
        self.finish_ref(tail.into())
    }
    /// Finish the same canonical stream from borrowed tail records.
    pub fn finish_ref(
        self,
        tail: FragmentFingerprintTailRef<'_>,
    ) -> Result<FragmentId, FragmentFingerprintRefusal> {
        Ok(FragmentId::from(crate::hex_digest(
            &self.finish_ref_digest(tail)?,
        )))
    }
    /// Preserve complete canonical validation before requesting output storage.
    pub fn try_finish_ref(
        self,
        tail: FragmentFingerprintTailRef<'_>,
    ) -> Result<FragmentId, FragmentFingerprintPreparationRefusal> {
        let digest = self
            .finish_ref_digest(tail)
            .map_err(FragmentFingerprintPreparationRefusal::Fingerprint)?;
        crate::try_hex_digest(&digest)
            .map(FragmentId::from)
            .map_err(|_| FragmentFingerprintPreparationRefusal::Capacity)
    }
    pub(crate) fn finish_ref_digest(
        self,
        tail: FragmentFingerprintTailRef<'_>,
    ) -> Result<[u8; 32], FragmentFingerprintRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        if self.remaining != 0 {
            return Err(FragmentFingerprintRefusal::IncompletePlacements);
        }
        let mut canonical = self.canonical;
        tail::push_tail(&mut canonical, tail);
        Ok(canonical.finish())
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
fn push_header(canonical: &mut impl CanonicalSink, header: FragmentFingerprintHeaderRef<'_>) {
    push_string(canonical, header.source_document_id);
    push_string(canonical, header.checked_plot_id);
    push_string(canonical, header.expanded_plot_id);
    canonical.push(header.completion_policy as u8);
    if !header.realization_backs.is_empty() {
        plan_realization::push_canonical(canonical, header.realization_backs);
    }
    push_string(canonical, header.host_id);
    push_string(canonical, header.boot_id);
    push_u64(canonical, header.offer_generation.0);
    execution_regions::push_canonical(canonical, header.execution_regions);
    execution_fusion::push_canonical(canonical, header.execution_fusions);
}
