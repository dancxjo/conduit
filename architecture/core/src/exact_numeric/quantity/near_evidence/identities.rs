//! Pure content identities shared by Near producers and independent readers.
use crate::{projection::*, *};
// Pure mathematical reports identify their actual content. Plan/Play execution
// custody is supplied separately by the kernel's mandatory emitted-value Signs.
pub struct NearProjectionContentIds {
    source: [u8; 64],
    target: [u8; 64],
    report: [u8; 64],
}
fn hex(digest: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    let alphabet = b"0123456789abcdef";
    for (i, b) in digest.iter().enumerate() {
        out[2 * i] = alphabet[(b >> 4) as usize];
        out[2 * i + 1] = alphabet[(b & 15) as usize];
    }
    out
}
impl NearProjectionContentIds {
    pub fn new(
        wire: &[u8],
        source: RationalQuantity,
        limit: Quantity,
        projected: Option<&PreparedQuantityBinary32Projection>,
    ) -> Self {
        let source_id = semantic_digest(QUANTITY_INFO_ID, wire);
        let mut target = [0; UNIT_ENCODED_LEN + 34];
        target[..UNIT_ENCODED_LEN].copy_from_slice(&source.unit().encode());
        target[UNIT_ENCODED_LEN] = match source.role() {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        if let Some(p) = projected {
            target[UNIT_ENCODED_LEN + 1] = 32;
            target[UNIT_ENCODED_LEN + 2..UNIT_ENCODED_LEN + 6]
                .copy_from_slice(&p.target().coordinate().bits().to_le_bytes());
        } else {
            target[UNIT_ENCODED_LEN + 2..].copy_from_slice(&source.coordinate().semantic_digest());
        }
        let target_id = semantic_digest("numeric/near-target@1", &target);
        let mut report = [0; 64 + QUANTITY_ENCODED_LEN];
        report[..32].copy_from_slice(&source_id);
        report[32..64].copy_from_slice(&target_id);
        report[64..].copy_from_slice(&limit.encode());
        Self {
            source: hex(source_id),
            target: hex(target_id),
            report: hex(semantic_digest("numeric/near-report@1", &report)),
        }
    }
}

impl NearProjectionContentIds {
    pub fn input<'a, D: ProjectionDomain>(
        &'a self,
        domain: &'a D,
        source: &'a D::Source,
        target: &'a D::Target,
        facts: &'a [ProjectionFact<'a, D>],
    ) -> ProjectionInput<'a, D> {
        let id = |s: &'a [u8]| ProjectionText::new(core::str::from_utf8(s).unwrap()).unwrap();
        ProjectionInput {
            basis: ProjectionBasis {
                report: id(&self.report),
                source: id(&self.source),
                target: Some(id(&self.target)),
                projector: domain.target_contract(),
                requested_route: ProjectionText::new("math/near").unwrap(),
                boundary: None,
            },
            source,
            target: Some(target),
            facts,
            native: &[],
            scores: &[],
            admitted: &[],
            attempts: &[],
            selected_attempt: None,
            mechanism: ProjectionMechanism::Completed,
            diagnostics: &[],
        }
    }
}
