//! Content identities for ordinary general projection reports.
use super::*;
// Pure mathematical reports identify their actual content. Plan/Play execution
// custody is supplied separately by the kernel's mandatory emitted-value Signs.
pub(super) struct ProjectionIds {
    pub(super) source: [u8; 64],
    pub(super) target: [u8; 64],
    pub(super) report: [u8; 64],
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
impl ProjectionIds {
    pub(super) fn new(
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
