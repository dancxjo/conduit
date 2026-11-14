//! Exact canonical requests/results; all preparation precedes electrical effects.
use super::contract::fixed_value_bound;
use super::{
    CheckedPulseRead, PulseReadCapture, PulseReadContract, PulseReadDisposition, PulseReadLimits,
    PulseReadRefusal, PulseReadRequest, PULSE_READ_MAXIMUM_BYTES,
};
use conduit_core::{
    BorrowedStructuredValue as V, BorrowedStructuredValueValidator,
    PreparedLinkedStructuredComposer, StructuredInfoRefusal,
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
    writer: PreparedLinkedStructuredComposer<'static>,
    output: alloc::vec::Vec<u8>,
}
impl PreparedPulseCodec {
    /// Retained heap backing; linked immutable schemas occupy no heap.
    /// Inline state and allocator overhead are separate.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.output.capacity())
    }
    pub fn prepare(contract: &PulseReadContract) -> Result<Self, StructuredInfoRefusal> {
        const REQUEST_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/request.type"));
        const CAPTURE_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/capture.type"));
        const RESULT_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/result.type"));
        if contract.request.canonical_bytes()? != REQUEST_SCHEMA
            || contract.capture.canonical_bytes()? != CAPTURE_SCHEMA
            || contract.result.canonical_bytes()? != RESULT_SCHEMA
        {
            return Err(StructuredInfoRefusal::WrongType);
        }
        // One canonical cache funds the complete fixed-width result domain.
        let maximum = fixed_value_bound(&contract.result)? as usize;
        Ok(Self {
            request: BorrowedStructuredValueValidator::new(
                REQUEST_SCHEMA,
                PULSE_READ_MAXIMUM_BYTES as usize,
            )?,
            writer: PreparedLinkedStructuredComposer::new(RESULT_SCHEMA, maximum)?,
            output: alloc::vec![0; maximum],
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
        self.encode("geometry-refused", V::Empty)
    }
    fn encode(&mut self, tag: &str, payload: V<'_>) -> Result<&[u8], PulseCodecRefusal> {
        Ok(self.writer.encode_into(
            V::Variant {
                tag,
                payload: &payload,
            },
            &mut self.output,
        )?)
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
            return self.encode("provider-failed", V::Empty);
        }
        let wire = capture
            .wire
            .each_ref()
            .map(|byte| V::Leaf(core::slice::from_ref(byte)));
        let length = [capture.length];
        let fields = [V::Leaf(&length), V::Collection(&wire)];
        let tag = if capture.disposition == PulseReadDisposition::Complete {
            "completed"
        } else {
            "timed-out"
        };
        self.encode(tag, V::Record(&fields))
    }
}
#[cfg(test)]
#[allow(dead_code)]
#[path = "codec_legacy.rs"]
mod legacy;
#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
