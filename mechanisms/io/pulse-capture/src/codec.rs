//! Exact canonical requests/results; all preparation precedes electrical effects.
use super::{
    CheckedPulseRead, PulseReadCapture, PulseReadContract, PulseReadDisposition, PulseReadLimits,
    PulseReadRefusal, PulseReadRequest, PULSE_READ_MAXIMUM_BYTES,
};
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
    PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape,
};
#[derive(Debug, PartialEq, Eq)]
pub enum PulseCodecRefusal {
    Canonical(StructuredInfoRefusal),
    Geometry(PulseReadRefusal),
    InvalidCapture,
}
impl From<StructuredInfoRefusal> for PulseCodecRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Canonical(value)
    }
}
pub struct PreparedPulseCodec {
    request: PreparedStructuredValueValidator,
    octets: [Composer; 8],
    length: Composer,
    wire: Composer,
    capture: Composer,
    empty: Composer,
    result: Composer,
}
fn field<'a>(
    ty: &'a StructuredInfoType,
    name: &str,
) -> Result<&'a StructuredInfoType, StructuredInfoRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(StructuredInfoRefusal::WrongType);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type())
        .ok_or(StructuredInfoRefusal::WrongType)
}
impl PreparedPulseCodec {
    pub fn prepare(contract: &PulseReadContract) -> Result<Self, StructuredInfoRefusal> {
        let wire_type = field(&contract.capture, "wire")?;
        let StructuredInfoTypeShape::Collection { element, length: 8 } = wire_type.shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result.shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let empty_type = cases
            .iter()
            .find(|c| c.tag() == "provider-failed")
            .ok_or(StructuredInfoRefusal::WrongType)?
            .payload_type();
        // Array construction is fallible during preparation; never while playing.
        let mut octets = alloc::vec::Vec::with_capacity(8);
        for _ in 0..8 {
            octets.push(Composer::new(element, 64)?);
        }
        Ok(Self {
            request: PreparedStructuredValueValidator::new(
                &contract.request,
                PULSE_READ_MAXIMUM_BYTES as usize,
            )?,
            octets: octets
                .try_into()
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            length: Composer::new(field(&contract.capture, "length")?, 64)?,
            wire: Composer::new(wire_type, 256)?,
            capture: Composer::new(&contract.capture, 512)?,
            empty: Composer::new(empty_type, 64)?,
            result: Composer::new(&contract.result, PULSE_READ_MAXIMUM_BYTES as usize)?,
        })
    }
    pub fn request(
        &self,
        input: &[u8],
        limits: PulseReadLimits,
    ) -> Result<CheckedPulseRead, PulseCodecRefusal> {
        self.request.validate(input)?;
        let value = validate(input)?;
        let read = |name| -> Result<u32, StructuredInfoRefusal> {
            let child = value
                .record_field(name)?
                .ok_or(StructuredInfoRefusal::WrongRecordFields)?;
            let bytes = child
                .value_node()
                .get(5..)
                .ok_or(StructuredInfoRefusal::WrongType)?;
            match bytes {
                [a] => Ok(u32::from(*a)),
                [a, b] => Ok(u32::from(u16::from_le_bytes([*a, *b]))),
                [a, b, c, d] => Ok(u32::from_le_bytes([*a, *b, *c, *d])),
                _ => Err(StructuredInfoRefusal::WrongType),
            }
        };
        let request = PulseReadRequest {
            hold_low_us: read("hold_low_us")?,
            skip_high_pulses: u8::try_from(read("skip_high_pulses")?)
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            sample_after_rise_us: u16::try_from(read("sample_after_rise_us")?)
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            octets: u8::try_from(read("octets")?).map_err(|_| StructuredInfoRefusal::WrongType)?,
            capture_timeout_us: read("capture_timeout_us")?,
        };
        CheckedPulseRead::admit(request, limits).map_err(PulseCodecRefusal::Geometry)
    }
    pub fn geometry_refused(&mut self) -> Result<&[u8], PulseCodecRefusal> {
        let empty = validate(self.empty.leaf(&[])?)?;
        Ok(self.result.variant("geometry-refused", empty)?)
    }
    pub fn result(&mut self, capture: PulseReadCapture) -> Result<&[u8], PulseCodecRefusal> {
        let count = usize::from(capture.length);
        if count > 8
            || capture.wire[count..].iter().any(|b| *b != 0)
            || (capture.disposition == PulseReadDisposition::Complete && count == 0)
        {
            return Err(PulseCodecRefusal::InvalidCapture);
        }
        if capture.disposition == PulseReadDisposition::ProviderFailed {
            if count != 0 {
                return Err(PulseCodecRefusal::InvalidCapture);
            }
            let empty = validate(self.empty.leaf(&[])?)?;
            return Ok(self.result.variant("provider-failed", empty)?);
        }
        for (encoder, byte) in self.octets.iter_mut().zip(capture.wire) {
            encoder.leaf(&[byte])?;
        }
        let octets: [_; 8] =
            core::array::from_fn(|i| validate(self.octets[i].encoded()).expect("prepared octet"));
        self.wire.collection(&octets)?;
        self.length.leaf(&[capture.length])?;
        self.capture.record(&[
            validate(self.length.encoded())?,
            validate(self.wire.encoded())?,
        ])?;
        let tag = if capture.disposition == PulseReadDisposition::Complete {
            "completed"
        } else {
            "timed-out"
        };
        Ok(self
            .result
            .variant(tag, validate(self.capture.encoded())?)?)
    }
}
