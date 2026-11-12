//! Previous production codec retained only as a byte oracle.
use super::*;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
};
pub struct PreparedObservationCodec {
    input: c::BorrowedStructuredValueValidator<'static>,
    temperature: Composer,
    humidity: Composer,
    observation: Composer,
    result: Composer,
}
impl PreparedObservationCodec {
    pub fn retained_backing_bytes(&self) -> usize {
        [
            &self.temperature,
            &self.humidity,
            &self.observation,
            &self.result,
        ]
        .iter()
        .map(|c| c.prepared_heap_bytes())
        .sum()
    }
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        let Shape::Record { fields, .. } = contract.observation.shape() else {
            return Err(R::WrongType);
        };
        if fields.len() != 2 || fields[0].name() != "humidity" || fields[1].name() != "temperature"
        {
            return Err(R::WrongRecordFields);
        }
        let temperature_type = field(&contract.observation, "temperature")?;
        let humidity_type = field(&contract.observation, "humidity")?;
        let leaf_bytes = |ty: &Type| -> Result<usize, R> {
            let bytes = ty.canonical_bytes()?.len() + 5 + c::QUANTITY_ENCODED_LEN;
            if bytes > MAXIMUM_BYTES as usize {
                return Err(R::CanonicalEncodingTooLarge);
            };
            Ok(bytes)
        };
        const INPUT_SCHEMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/decoded.type"));
        if contract.input.canonical_bytes()? != INPUT_SCHEMA {
            return Err(R::WrongType);
        }
        Ok(Self {
            input: c::BorrowedStructuredValueValidator::new(INPUT_SCHEMA, 1024)?,
            temperature: Composer::with_static_schema(
                temperature_type,
                include_bytes!(concat!(env!("OUT_DIR"), "/observation-temperature.type")),
                leaf_bytes(temperature_type)?,
            )?,
            humidity: Composer::with_static_schema(
                humidity_type,
                include_bytes!(concat!(env!("OUT_DIR"), "/observation-humidity.type")),
                leaf_bytes(humidity_type)?,
            )?,
            observation: Composer::with_static_schema(
                &contract.observation,
                include_bytes!(concat!(env!("OUT_DIR"), "/observation.type")),
                MAXIMUM_BYTES as usize,
            )?,
            result: Composer::with_static_schema(
                &contract.output,
                RESULT_TYPE_BYTES,
                MAXIMUM_BYTES as usize,
            )?,
        })
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
            self.temperature.leaf(&temperature.encode())?;
            self.humidity.leaf(&humidity.encode())?;
            self.observation.record(&[
                // Structured records use canonical field order, independent
                // of the order of fields in the authored declaration.
                validate(self.humidity.encoded())?,
                validate(self.temperature.encoded())?,
            ])?;
            self.result
                .variant("observation", validate(self.observation.encoded())?)?;
        } else {
            self.result.variant(
                "unavailable",
                decoded
                    .variant_payload("unavailable")?
                    .ok_or(R::WrongType)?,
            )?;
        }
        Ok(self.result.encoded())
    }
    pub fn output(&self) -> &[u8] {
        self.result.encoded()
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
