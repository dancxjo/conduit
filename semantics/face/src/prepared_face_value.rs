//! Cold verification of one exact bounded Face value for allocation-free dispatch.
use crate::Face;
use sha2::{Digest, Sha256};
/// A byte pin, not authority, freshness or a substitute for the semantic Face.
/// A Host prepares this before Play and checks its actual Host Call input later.
pub struct PreparedFaceValue {
    bytes: usize,
    digest: [u8; 32],
}
impl PreparedFaceValue {
    pub fn prepare(face: &Face, wire: &[u8], maximum_bytes: usize) -> Result<Self, &'static str> {
        if wire.is_empty() || wire.len() > maximum_bytes {
            return Err("face-value-capacity");
        }
        face.validate().map_err(|_| "invalid-face")?;
        let decoded: Face = serde_json::from_slice(wire).map_err(|_| "face-value-json")?;
        if decoded != *face {
            return Err("face-value-mismatch");
        }
        Ok(Self {
            bytes: wire.len(),
            digest: Sha256::digest(wire).into(),
        })
    }
    pub fn matches(&self, wire: &[u8]) -> bool {
        wire.len() == self.bytes && <[u8; 32]>::from(Sha256::digest(wire)) == self.digest
    }
}
