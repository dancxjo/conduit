//! Finite Frame composition for a Mask producer, independent of packet execution.
use conduit_core::{validate_canonical_structured_value as validate, StructuredInfoRefusal as R};
mod material {
    include!(concat!(env!("OUT_DIR"), "/tm1637-frame-material.rs"));
}
/// Exact canonical extent derived from the linked finite Source Frame type.
pub const DISPLAY_FRAME_BYTES: usize = material::TEMPLATE.len();
/// Check a local Fore value against the independently linked Source type.
/// This grants no caller authority, queues no input and performs no effect.
/// Brightness above seven remains typed input for the Plot's refusal branch.
/// Canonical validation checks primitive payloads as well as the envelope.
pub fn validate_display_frame(
    encoded: &[u8],
) -> Result<conduit_core::ValidatedCanonicalStructuredValue<'_>, R> {
    if encoded.len() > 1024 {
        return Err(R::CanonicalEncodingTooLarge);
    }
    let value = validate(encoded)?;
    if value.type_bytes() != include_bytes!(concat!(env!("OUT_DIR"), "/tm1637-frame.type")) {
        return Err(R::WrongType);
    }
    Ok(value)
}

/// Owns only finite canonical bytes. Build-time ordinary composition derives
/// the template and all scalar locations from the independently checked Source
/// type. No heap schema, packet evaluator, authority or formatting is retained.
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
    /// Preserve every register bit and all brightness values. The output Plot
    /// still owns brightness refusal and packet command synthesis.
    pub fn encode(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<&[u8], R> {
        for (offset, value) in material::OFFSETS
            .into_iter()
            .zip(registers.into_iter().chain([brightness, u8::from(enabled)]))
        {
            self.bytes[offset] = value;
        }
        validate_display_frame(&self.bytes)?;
        Ok(&self.bytes)
    }
}
