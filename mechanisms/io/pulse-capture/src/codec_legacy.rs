//! Exact canonical requests/results; all preparation precedes electrical effects.
use crate::contract::fixed_value_bound;
use crate::{
    CheckedPulseRead, PulseReadCapture, PulseReadContract, PulseReadDisposition, PulseReadLimits,
    PulseReadRefusal, PulseReadRequest, PULSE_READ_MAXIMUM_BYTES,
};
use conduit_core::{
    validate_canonical_structured_value as validate, BorrowedStructuredValueValidator,
    PreparedStructuredComposer as Composer, StructuredInfoRefusal, StructuredInfoType,
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
    request: BorrowedStructuredValueValidator<'static>,
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
    /// Retained heap backing; linked immutable schemas occupy no heap.
    /// Inline state and allocator overhead are separate.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = 0usize;
        for composer in [
            &self.length,
            &self.wire,
            &self.capture,
            &self.empty,
            &self.result,
        ] {
            bytes = bytes.checked_add(composer.prepared_heap_bytes())?;
        }
        Some(bytes)
    }

    pub fn prepare(contract: &PulseReadContract) -> Result<Self, StructuredInfoRefusal> {
        // Fund the complete canonical domain of every fixed-width Type, while
        // the public Host Call envelope remains PULSE_READ_MAXIMUM_BYTES.
        let length_type = field(&contract.capture, "length")?;
        let wire_type = field(&contract.capture, "wire")?;
        let StructuredInfoTypeShape::Collection { length: 8, .. } = wire_type.shape() else {
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
        const REQUEST_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/request.type"));
        if contract.request.canonical_bytes()? != REQUEST_SCHEMA {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Ok(Self {
            request: BorrowedStructuredValueValidator::new(
                REQUEST_SCHEMA,
                PULSE_READ_MAXIMUM_BYTES as usize,
            )?,
            length: Composer::with_static_schema(
                length_type,
                include_bytes!(concat!(env!("OUT_DIR"), "/length.type")),
                fixed_value_bound(length_type)? as usize,
            )?,
            wire: Composer::with_static_schema(
                wire_type,
                include_bytes!(concat!(env!("OUT_DIR"), "/wire.type")),
                fixed_value_bound(wire_type)? as usize,
            )?,
            capture: Composer::with_static_schema(
                &contract.capture,
                include_bytes!(concat!(env!("OUT_DIR"), "/capture.type")),
                fixed_value_bound(&contract.capture)? as usize,
            )?,
            empty: Composer::with_static_schema(
                empty_type,
                include_bytes!(concat!(env!("OUT_DIR"), "/empty.type")),
                fixed_value_bound(empty_type)? as usize,
            )?,
            result: Composer::with_static_schema(
                &contract.result,
                include_bytes!(concat!(env!("OUT_DIR"), "/result.type")),
                fixed_value_bound(&contract.result)? as usize,
            )?,
        })
    }
    pub fn request(
        &self,
        input: &[u8],
        limits: PulseReadLimits,
    ) -> Result<CheckedPulseRead, PulseCodecRefusal> {
        let value = self.request.validate(input)?;
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
        self.wire
            .leaf_collection(capture.wire.iter().map(core::slice::from_ref))?;
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
