//! Fixed canonical frame storage derived from the checked authored Source type.
//! No runtime schema owner, packet evaluator, allocation or bus effect.
use conduit_core::{validate_canonical_structured_value as validate, StructuredInfoRefusal as R};
mod material {
    include!(concat!(env!("OUT_DIR"), "/ht16k33-frame-material.rs"));
}
pub const DISPLAY_FRAME_BYTES: usize = material::TEMPLATE.len();
pub fn validate_display_frame(
    bytes: &[u8],
) -> Result<conduit_core::ValidatedCanonicalStructuredValue<'_>, R> {
    if bytes.len() != DISPLAY_FRAME_BYTES {
        return Err(R::WrongType);
    }
    let value = validate(bytes)?;
    if value.type_bytes() != include_bytes!(concat!(env!("OUT_DIR"), "/ht16k33-frame.type")) {
        return Err(R::WrongType);
    }
    Ok(value)
}
pub struct PreparedDisplayFrame {
    bytes: [u8; DISPLAY_FRAME_BYTES],
}
impl PreparedDisplayFrame {
    pub fn prepare() -> Result<Self, R> {
        validate_display_frame(&material::TEMPLATE)?;
        Ok(Self {
            bytes: material::TEMPLATE,
        })
    }
    /// Typed Address already enforces the controller range. Every RAM bit is
    /// preserved; only the selected write Plot's outcome can acknowledge output.
    pub fn encode(&mut self, address: crate::Address, ram: [u8; 16]) -> Result<&[u8], R> {
        for (offset, byte) in material::OFFSETS
            .into_iter()
            .zip(core::iter::once(address.get()).chain(ram))
        {
            self.bytes[offset] = byte;
        }
        validate_display_frame(&self.bytes)?;
        Ok(&self.bytes)
    }
}
