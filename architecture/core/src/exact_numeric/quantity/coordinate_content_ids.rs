//! Pure content correlation for coordinate reports; execution remains in Signs.
use super::*;
use crate::projection::*;
pub struct QuantityCoordinateContentIds {
    source: [u8; 64],
    target: [u8; 64],
    report: [u8; 64],
}
fn hex(digest: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    for (i, byte) in digest.iter().enumerate() {
        out[2 * i] = b"0123456789abcdef"[(byte >> 4) as usize];
        out[2 * i + 1] = b"0123456789abcdef"[(byte & 15) as usize];
    }
    out
}
impl QuantityCoordinateContentIds {
    pub fn new(source: &[u8], target: &[u8], unit: Unit) -> Self {
        let source_id = crate::semantic_digest(crate::QUANTITY_INFO_ID, source);
        let target_id = crate::semantic_digest(crate::QUANTITY_INFO_ID, target);
        let mut binding = [0; 64 + crate::UNIT_ENCODED_LEN];
        binding[..32].copy_from_slice(&source_id);
        binding[32..64].copy_from_slice(&target_id);
        binding[64..].copy_from_slice(&unit.encode());
        Self {
            source: hex(source_id),
            target: hex(target_id),
            report: hex(crate::semantic_digest(
                "physical/quantity-coordinate-report@1",
                &binding,
            )),
        }
    }
    pub fn basis(&self) -> ProjectionBasis<'_> {
        let text = |bytes| ProjectionText::new(core::str::from_utf8(bytes).unwrap()).unwrap();
        ProjectionBasis {
            report: text(&self.report),
            source: text(&self.source),
            target: Some(text(&self.target)),
            projector: ProjectionText::new(MATHEMATICAL_QUANTITY_CONVERSION_LAW).unwrap(),
            requested_route: ProjectionText::new("math/coordinates").unwrap(),
            boundary: None,
        }
    }
}
