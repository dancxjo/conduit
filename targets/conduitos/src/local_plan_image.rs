//! Bounded development transport for a sealed local Plan and exact Source bytes.
//!
//! This verifies content correspondence, not Source checking, derivation, Native
//! laws, resource authority, or target availability. Those owners still admit the
//! retained Plan before preparation. A reference image is not a fresh Boot Plan.
use alloc::vec::Vec;
use conduit_core::{BootId, HostId, Plan, verify_plan};

pub const FORMAT: &str = "conduit/core-plan-serde-json-development@1";
pub const MAXIMUM_IMAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAXIMUM_SOURCE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalPlanImageBounds {
    pub image_bytes: usize,
    pub source_bytes: usize,
    pub placements: usize,
    pub cords: usize,
}
impl LocalPlanImageBounds {
    fn validate(self) -> Result<Self, LocalPlanImageRefusal> {
        if self.image_bytes == 0
            || self.image_bytes > MAXIMUM_IMAGE_BYTES
            || self.source_bytes == 0
            || self.source_bytes > MAXIMUM_SOURCE_BYTES
            || self.placements == 0
            || self.placements > u16::MAX as usize
            || self.cords == 0
            || self.cords > u16::MAX as usize
        {
            return Err(LocalPlanImageRefusal::Bounds);
        }
        Ok(self)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalPlanImageRefusal {
    Bounds,
    Encoding,
    NonCanonicalImage,
    Seal,
    Source,
    LocalFragment,
    Boot,
}

pub struct DecodedLocalPlanImage {
    plan: Plan,
    image_bytes: usize,
    source_bytes: usize,
}
impl DecodedLocalPlanImage {
    /// Decode during preparation. The decoder and exact re-encoding allocate;
    /// encoded byte limits are not a bound on peak decoded preparation storage.
    pub fn decode(
        image: &[u8],
        source: &[u8],
        bounds: LocalPlanImageBounds,
    ) -> Result<Self, LocalPlanImageRefusal> {
        let bounds = bounds.validate()?;
        if image.is_empty()
            || image.len() > bounds.image_bytes
            || source.is_empty()
            || source.len() > bounds.source_bytes
        {
            return Err(LocalPlanImageRefusal::Bounds);
        }
        let source = core::str::from_utf8(source).map_err(|_| LocalPlanImageRefusal::Source)?;
        let plan: Plan =
            serde_json::from_slice(image).map_err(|_| LocalPlanImageRefusal::Encoding)?;
        // Reject unknown fields, alternate whitespace/ordering and default-field
        // omissions rather than treating serde's permissive decode as this format.
        let canonical: Vec<u8> =
            serde_json::to_vec(&plan).map_err(|_| LocalPlanImageRefusal::Encoding)?;
        if canonical != image {
            return Err(LocalPlanImageRefusal::NonCanonicalImage);
        }
        if !verify_plan(&plan) {
            return Err(LocalPlanImageRefusal::Seal);
        }
        if conduit_plot::syntax_source_document_identity(source) != plan.source_document_id {
            return Err(LocalPlanImageRefusal::Source);
        }
        let [fragment] = plan.fragments.as_slice() else {
            return Err(LocalPlanImageRefusal::LocalFragment);
        };
        if fragment.placements.len() > bounds.placements
            || fragment.connections.len() > bounds.cords
        {
            return Err(LocalPlanImageRefusal::Bounds);
        }
        Ok(Self {
            plan,
            image_bytes: image.len(),
            source_bytes: source.len(),
        })
    }
    pub fn reference_plan(&self) -> &Plan {
        &self.plan
    }
    /// A reference fixture's identities cannot be substituted for this Boot.
    /// Matching here still does not admit operations, schemas or resources.
    pub fn plan_for_boot(
        &self,
        host: &HostId,
        boot: &BootId,
    ) -> Result<&Plan, LocalPlanImageRefusal> {
        let fragment = &self.plan.fragments[0];
        if &fragment.host_id != host || &fragment.boot_id != boot {
            return Err(LocalPlanImageRefusal::Boot);
        }
        Ok(&self.plan)
    }
    pub fn encoded_extents(&self) -> (usize, usize) {
        (self.image_bytes, self.source_bytes)
    }
}

#[cfg(test)]
mod tests;
