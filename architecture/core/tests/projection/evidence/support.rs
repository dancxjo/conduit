//! Shared domain/policy fixtures for canonical evidence transport.
use super::super::common::*;
use conduit_core::{claims::*, projection::*, *};

pub(super) struct Policy(pub(super) u64);
pub(super) fn configuration(value: u64) -> [u8; 32] {
    semantic_digest("test/projection-evidence-policy@1", &value.to_le_bytes())
}
impl ProjectionPolicy<Bits> for Policy {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/evidence-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(configuration(self.0))
    }
    fn permits(&self, source: &u64, target: &u64, facts: &[ProjectionFact<'_, Bits>]) -> bool {
        Budget(self.0 as usize).permits(source, target, facts)
    }
    fn permits_normalization(
        &self,
        original: ClaimScore<'_>,
        projected: ClaimScore<'_>,
        policy: ProjectionText<'_>,
    ) -> bool {
        policy.as_str() == "invert@1"
            && (0..=100).contains(&original.value())
            && original == calibrated("cost@1", "calibration/native@1", original.value())
            && projected
                == calibrated(
                    "inverse-cost@1",
                    "calibration/projected@1",
                    100 - original.value(),
                )
    }
}
pub(super) struct Codec(pub(super) u64);
impl ProjectionEvidenceCodec<Bits> for Codec {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/bits-evidence@1")
    }
    fn configuration_digest(&self) -> [u8; 32] {
        configuration(self.0)
    }
    fn source(
        &self,
        value: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.u64(*value)
    }
    fn target(
        &self,
        value: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.u64(*value)
    }
    fn detail(
        &self,
        value: &usize,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.u64(*value as u64)
    }
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.u64(self.0)
    }
}
pub(super) fn limits() -> ProjectionEvidenceLimits {
    ProjectionEvidenceLimits {
        source_bytes: 8,
        target_bytes: 8,
        detail_bytes: 8,
        policy_bytes: 8,
        total_bytes: 4096,
    }
}
pub(super) fn calibrated<'a>(scale: &'a str, calibration: &'a str, value: i64) -> ClaimScore<'a> {
    ClaimScore::new(
        ClaimText::new(scale).unwrap(),
        Some(ClaimText::new(calibration).unwrap()),
        ClaimText::new("provider@1").unwrap(),
        0,
        100,
        value,
    )
    .unwrap()
}
