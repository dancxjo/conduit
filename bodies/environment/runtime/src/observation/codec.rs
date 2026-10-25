//! The same authored quantity Forms, with one canonical output boundary.
use super::{forms, template::QuantityTemplate, ObservationContract, ObservationRefusal};
use crate::canonical_input::PreparedCanonicalInput;
use conduit_core::{
    self as c, validate_canonical_structured_value as validate, StructuredInfoRefusal as R,
    ValidatedCanonicalStructuredValue as Value,
};
pub struct PreparedObservationCodec {
    input: PreparedCanonicalInput,
    result: QuantityTemplate<true>,
}
impl PreparedObservationCodec {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        // Preserve the existing preparation envelope before discarding its
        // decoded validator tree and borrowing the exact linked Type instead.
        drop(c::PreparedStructuredValueValidator::new(
            contract.input_type(),
            1024,
        )?);
        Ok(Self {
            input: PreparedCanonicalInput::with_static_schema(
                contract.input_type(),
                include_bytes!(concat!(env!("OUT_DIR"), "/decoded.type")),
                1024,
            )?,
            result: QuantityTemplate::prepare(contract.result_type())?,
        })
    }
    pub fn convert(&mut self, input: &[u8]) -> Result<&[u8], ObservationRefusal> {
        self.input.validate(input)?;
        let decoded = validate(input)?;
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
            self.result
                .quantities(temperature, humidity)
                .map_err(Into::into)
        } else {
            self.result
                .unavailable(
                    decoded
                        .variant_payload("unavailable")?
                        .ok_or(R::WrongType)?,
                )
                .map_err(Into::into)
        }
    }
    pub fn output(&self) -> &[u8] {
        self.result.output()
    }
}
fn coordinate(value: Value<'_>) -> Result<i128, R> {
    let bytes = value.value_node();
    if bytes.get(..5) != Some(&[0, 16, 0, 0, 0]) {
        return Err(R::MalformedCanonicalEncoding);
    }
    Ok(i128::from_le_bytes(
        bytes[5..]
            .try_into()
            .map_err(|_| R::MalformedCanonicalEncoding)?,
    ))
}
