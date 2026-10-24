//! Finite Frame composition for a Mask producer, independent of packet execution.
use conduit_core::{
    validate_canonical_structured_value as validate, InfoBool, PreparedStructuredComposer as C,
    StructuredInfoRefusal as R, StructuredInfoType as T, StructuredInfoTypeShape as S,
};
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

/// Owns finite prepared canonical Frame storage. No packet program, GPIO,
/// caller permission, queue, clock or formatting policy is retained here.
pub struct PreparedDisplayFrame {
    octets: [C; 6],
    registers: C,
    brightness: C,
    enabled: C,
    frame: C,
}
impl PreparedDisplayFrame {
    pub fn prepare() -> Result<Self, R> {
        let input = T::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/tm1637-frame.type"
        )))?;
        let registers = field(&input, "registers")?;
        let S::Collection { element, length: 6 } = registers.shape() else {
            return Err(R::WrongType);
        };
        let octet = || C::new(element, 64);
        Ok(Self {
            octets: [octet()?, octet()?, octet()?, octet()?, octet()?, octet()?],
            registers: C::new(registers, 512)?,
            brightness: C::new(field(&input, "brightness")?, 64)?,
            enabled: C::new(field(&input, "enabled")?, 64)?,
            frame: C::new(&input, 1024)?,
        })
    }
    /// The result borrows reusable storage. Brightness outside 0..=7 remains
    /// representable so the checked output Plot owns its semantic refusal.
    pub fn encode(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<&[u8], R> {
        for (encoder, value) in self.octets.iter_mut().zip(registers) {
            encoder.leaf(&[value])?;
        }
        self.registers.collection(&[
            validate(self.octets[0].encoded())?,
            validate(self.octets[1].encoded())?,
            validate(self.octets[2].encoded())?,
            validate(self.octets[3].encoded())?,
            validate(self.octets[4].encoded())?,
            validate(self.octets[5].encoded())?,
        ])?;
        self.brightness.leaf(&[brightness])?;
        self.enabled.leaf(&InfoBool::new(enabled).encode())?;
        self.frame.record(&[
            validate(self.brightness.encoded())?,
            validate(self.enabled.encoded())?,
            validate(self.registers.encoded())?,
        ])?;
        Ok(self.frame.encoded())
    }
}
fn field<'a>(ty: &'a T, name: &str) -> Result<&'a T, R> {
    let S::Record { fields, .. } = ty.shape() else {
        return Err(R::WrongType);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type())
        .ok_or(R::WrongType)
}
