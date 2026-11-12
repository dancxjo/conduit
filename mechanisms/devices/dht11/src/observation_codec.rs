//! Canonical observation conversion using linked schemas and one output cache.
use super::*;
use conduit_core::{BorrowedStructuredValue as V, ValidatedCanonicalStructuredValue as Value};
pub struct PreparedObservationCodec {
    input: c::BorrowedStructuredValueValidator<'static>,
    writer: c::PreparedLinkedStructuredComposer<'static>,
    output: alloc::vec::Vec<u8>,
    length: usize,
}
impl PreparedObservationCodec {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        let Shape::Record { fields, .. } = contract.observation.shape() else {
            return Err(R::WrongType);
        };
        if fields.len() != 2 || fields[0].name() != "humidity" || fields[1].name() != "temperature"
        {
            return Err(R::WrongRecordFields);
        }
        const INPUT_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/decoded.type"));
        const OBSERVATION_SCHEMA: &[u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/observation.type"));
        if contract.input.canonical_bytes()? != INPUT_SCHEMA
            || contract.observation.canonical_bytes()? != OBSERVATION_SCHEMA
            || contract.output.canonical_bytes()? != RESULT_TYPE_BYTES
        {
            return Err(R::WrongType);
        }
        let maximum = storage_ceiling::bytes(&contract.output, MAXIMUM_BYTES as usize)?;
        Ok(Self {
            input: c::BorrowedStructuredValueValidator::new(INPUT_SCHEMA, 1024)?,
            writer: c::PreparedLinkedStructuredComposer::new(RESULT_TYPE_BYTES, maximum)?,
            output: alloc::vec![0; maximum],
            length: 0,
        })
    }
    /// Retained mutable heap backing; immutable schemas stay linked.
    pub fn retained_backing_bytes(&self) -> usize {
        self.output.capacity()
    }
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8], ObservationRefusal> {
        let decoded = self.input.validate(input)?;
        if let Some(frame) = decoded.variant_payload("frame")? {
            let temperature = i16::try_from(coordinate(
                frame.record_field("temperature")?.ok_or(R::WrongType)?,
            )?)
            .map_err(|_| ObservationRefusal::CoordinateOutOfRange)?;
            let humidity = u16::try_from(coordinate(
                frame.record_field("humidity")?.ok_or(R::WrongType)?,
            )?)
            .map_err(|_| ObservationRefusal::CoordinateOutOfRange)?;
            let temperature =
                forms::DeviceDht11TemperatureForm::decode(&temperature.to_le_bytes())?;
            let humidity = forms::DeviceDht11HumidityForm::decode(&humidity.to_le_bytes())?;
            let temperature = temperature.encode();
            let humidity = humidity.encode();
            let fields = [V::Leaf(&humidity), V::Leaf(&temperature)];
            let payload = V::Record(&fields);
            self.length = self
                .writer
                .encode_into(
                    V::Variant {
                        tag: "observation",
                        payload: &payload,
                    },
                    &mut self.output,
                )?
                .len();
        } else {
            let payload = V::Canonical(
                decoded
                    .variant_payload("unavailable")?
                    .ok_or(R::WrongType)?,
            );
            self.length = self
                .writer
                .encode_into(
                    V::Variant {
                        tag: "unavailable",
                        payload: &payload,
                    },
                    &mut self.output,
                )?
                .len();
        }
        Ok(self.output())
    }
    pub fn output(&self) -> &[u8] {
        &self.output[..self.length]
    }
}
fn coordinate(value: Value<'_>) -> Result<i128, R> {
    let bytes = value.value_node();
    if bytes.get(..5) != Some(&[0, 16, 0, 0, 0]) {
        return Err(R::MalformedCanonicalEncoding);
    };
    Ok(i128::from_le_bytes(
        bytes[5..]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    ))
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "observation_codec_legacy.rs"]
mod legacy;
#[cfg(test)]
#[path = "observation_codec_tests.rs"]
mod tests;
