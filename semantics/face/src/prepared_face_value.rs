//! Cold verification of one exact bounded Face value for allocation-free dispatch.
use crate::Face;
use sha2::{Digest, Sha256};
/// A byte pin, not authority, freshness or a substitute for the semantic Face.
/// A Host prepares this before Play and checks its actual Host Call input later.
pub struct PreparedFaceValue {
    bytes: usize,
    digest: [u8; 32],
}
/// Complete semantic validation of this exact immutable borrow. This view
/// conveys no freshness, membership, execution or physical authority.
/// The borrow prevents changing the validated Face while the view is used.
/// ```compile_fail
/// fn mutate(face: &mut conduit_face::Face) {
///     let view = conduit_face::ValidatedFace::prepare(face).unwrap();
///     face.revision += 1;
///     let _ = view.face();
/// }
/// ```
#[derive(Clone, Copy)]
pub struct ValidatedFace<'a> {
    face: &'a Face,
}
impl<'a> ValidatedFace<'a> {
    pub fn prepare(face: &'a Face) -> Result<Self, &'static str> {
        face.validate().map_err(|_| "invalid-face")?;
        Ok(Self { face })
    }
    pub fn face(self) -> &'a Face {
        self.face
    }
}
impl PreparedFaceValue {
    pub fn prepare(face: &Face, wire: &[u8], maximum_bytes: usize) -> Result<Self, &'static str> {
        Self::prepare_with_view(face, wire, maximum_bytes).map(|(value, _)| value)
    }
    pub fn prepare_with_view<'a>(
        face: &'a Face,
        wire: &[u8],
        maximum_bytes: usize,
    ) -> Result<(Self, ValidatedFace<'a>), &'static str> {
        if wire.is_empty() || wire.len() > maximum_bytes {
            return Err("face-value-capacity");
        }
        let view = ValidatedFace::prepare(face)?;
        let decoded: Face = serde_json::from_slice(wire).map_err(|_| "face-value-json")?;
        if decoded != *face {
            return Err("face-value-mismatch");
        }
        Ok((
            Self {
                bytes: wire.len(),
                digest: Sha256::digest(wire).into(),
            },
            view,
        ))
    }
    pub fn matches(&self, wire: &[u8]) -> bool {
        wire.len() == self.bytes && <[u8; 32]>::from(Sha256::digest(wire)) == self.digest
    }
}
