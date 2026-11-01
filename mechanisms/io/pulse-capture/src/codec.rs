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
// Complete checked Source Type bounds for private transport copies only.
// Public native admission/capability envelopes remain PULSE_READ_MAXIMUM_BYTES.
include!(concat!(env!("OUT_DIR"), "/canonical_bounds.rs"));
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
            octets.push(Composer::with_static_schema(
                element,
                super::schema::OCTET,
                PULSE_READ_CANONICAL_OCTET_BYTES,
            )?);
        }
        Ok(Self {
            request: PreparedStructuredValueValidator::with_static_schema(
                &contract.request,
                super::schema::REQUEST,
                PULSE_READ_MAXIMUM_BYTES as usize,
            )?,
            octets: octets
                .try_into()
                .map_err(|_| StructuredInfoRefusal::WrongType)?,
            length: Composer::with_static_schema(
                field(&contract.capture, "length")?,
                super::schema::LENGTH,
                PULSE_READ_CANONICAL_LENGTH_BYTES,
            )?,
            wire: Composer::with_static_schema(
                wire_type,
                super::schema::WIRE,
                PULSE_READ_CANONICAL_WIRE_BYTES,
            )?,
            capture: Composer::with_static_schema(
                &contract.capture,
                super::schema::CAPTURE,
                PULSE_READ_CANONICAL_CAPTURE_BYTES,
            )?,
            empty: Composer::with_static_schema(
                empty_type,
                super::schema::EMPTY,
                PULSE_READ_CANONICAL_EMPTY_BYTES,
            )?,
            result: Composer::with_static_schema(
                &contract.result,
                super::schema::RESULT,
                PULSE_READ_CANONICAL_RESULT_BYTES,
            )?,
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

#[cfg(test)]
mod static_schema_tests {
    use super::*;
    use crate::allocation_probe;
    #[global_allocator]
    static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
    fn owned(contract: &PulseReadContract) -> PreparedPulseCodec {
        let wire = field(&contract.capture, "wire").unwrap();
        let StructuredInfoTypeShape::Collection { element, .. } = wire.shape() else {
            panic!("wire")
        };
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result.shape() else {
            panic!("result")
        };
        let empty = cases
            .iter()
            .find(|c| c.tag() == "provider-failed")
            .unwrap()
            .payload_type();
        PreparedPulseCodec {
            request: PreparedStructuredValueValidator::new(
                &contract.request,
                PULSE_READ_MAXIMUM_BYTES as usize,
            )
            .unwrap(),
            octets: core::array::from_fn(|_| {
                Composer::new(element, PULSE_READ_CANONICAL_OCTET_BYTES).unwrap()
            }),
            length: Composer::new(
                field(&contract.capture, "length").unwrap(),
                PULSE_READ_CANONICAL_LENGTH_BYTES,
            )
            .unwrap(),
            wire: Composer::new(wire, PULSE_READ_CANONICAL_WIRE_BYTES).unwrap(),
            capture: Composer::new(&contract.capture, PULSE_READ_CANONICAL_CAPTURE_BYTES).unwrap(),
            empty: Composer::new(empty, PULSE_READ_CANONICAL_EMPTY_BYTES).unwrap(),
            result: Composer::new(&contract.result, PULSE_READ_CANONICAL_RESULT_BYTES).unwrap(),
        }
    }
    #[test]
    fn linked_codec_preserves_all_result_bytes_refusals_and_allocation_free_play() {
        let contract = PulseReadContract::prepare().unwrap();
        assert_eq!(
            contract.request.canonical_bytes().unwrap(),
            super::super::schema::REQUEST
        );
        assert_eq!(
            contract.capture.canonical_bytes().unwrap(),
            super::super::schema::CAPTURE
        );
        assert_eq!(
            contract.result.canonical_bytes().unwrap(),
            super::super::schema::RESULT
        );
        let (mut linked, linked_memory) =
            allocation_probe::observe(|| PreparedPulseCodec::prepare(&contract).unwrap());
        let (mut original, owned_memory) = allocation_probe::observe(|| owned(&contract));
        let expected = super::super::schema::REQUEST.len()
            + 8 * super::super::schema::OCTET.len()
            + super::super::schema::LENGTH.len()
            + super::super::schema::WIRE.len()
            + super::super::schema::CAPTURE.len()
            + super::super::schema::EMPTY.len()
            + super::super::schema::RESULT.len();
        assert_eq!(
            owned_memory.retained_bytes() - linked_memory.retained_bytes(),
            expected
        );
        std::println!(
            "Pulse codec retained owned={} static={} exact shared-schema saving={expected}",
            owned_memory.retained_bytes(),
            linked_memory.retained_bytes()
        );
        let StructuredInfoTypeShape::Record { fields, .. } = contract.request.shape() else {
            panic!("request")
        };
        let mut parts: alloc::vec::Vec<Composer> = fields
            .iter()
            .map(|f| Composer::new(f.value_type(), 1024).unwrap())
            .collect();
        for (part, field) in parts.iter_mut().zip(fields) {
            let value: alloc::vec::Vec<u8> = match field.name() {
                "hold_low_us" => 1000u32.to_le_bytes().to_vec(),
                "skip_high_pulses" => alloc::vec![0],
                "sample_after_rise_us" => 30u16.to_le_bytes().to_vec(),
                "octets" => alloc::vec![5],
                "capture_timeout_us" => 10000u32.to_le_bytes().to_vec(),
                _ => panic!("checked request field"),
            };
            part.leaf(&value).unwrap();
        }
        let refs: alloc::vec::Vec<_> = parts
            .iter()
            .map(|p| validate(p.encoded()).unwrap())
            .collect();
        let mut request_composer = Composer::new(&contract.request, 1024).unwrap();
        let request = request_composer.record(&refs).unwrap().to_vec();
        let limits = PulseReadLimits {
            maximum_hold_low_us: 2000,
            maximum_skipped_pulses: 8,
            minimum_sample_offset_us: 1,
            maximum_sample_offset_us: 100,
            maximum_octets: 8,
            maximum_capture_timeout_us: 20000,
        };
        let mut corrupt = request.clone();
        corrupt[0] ^= 255;
        let oversized = alloc::vec![0;1025];
        assert_eq!(
            super::super::schema::OCTET.as_ptr(),
            super::super::schema::LENGTH.as_ptr()
        );
        let ((), play) = allocation_probe::observe(|| {
            for input in [
                &request[..],
                &request[..request.len() - 1],
                &corrupt,
                &oversized,
                &[],
            ] {
                assert_eq!(
                    linked.request(input, limits),
                    original.request(input, limits)
                );
                let mut strict = limits;
                strict.maximum_octets = 1;
                assert_eq!(
                    linked.request(input, strict),
                    original.request(input, strict)
                );
            }
            for disposition in [
                PulseReadDisposition::Complete,
                PulseReadDisposition::TimedOut,
                PulseReadDisposition::ProviderFailed,
            ] {
                for length in 0..=9 {
                    for byte in [0, 1, 127, 128, 255] {
                        let mut wire = [0; 8];
                        for value in wire.iter_mut().take(usize::from(length).min(8)) {
                            *value = byte;
                        }
                        let capture = PulseReadCapture {
                            disposition,
                            length,
                            wire,
                        };
                        assert_eq!(linked.result(capture), original.result(capture));
                    }
                }
            }
            let invalid = PulseReadCapture {
                disposition: PulseReadDisposition::TimedOut,
                length: 1,
                wire: [255; 8],
            };
            assert_eq!(linked.result(invalid), original.result(invalid));
            assert_eq!(linked.geometry_refused(), original.geometry_refused());
        });
        assert_eq!(play.allocations, 0);
        assert_eq!(play.reallocations, 0);
    }
}
