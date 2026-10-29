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

/// Prepared immutable template with only checked changing coordinates retained.
/// Full canonical bytes are composed into the scheduler-owned destination.
pub struct PreparedObservationEmitter {
    input: PreparedCanonicalInput,
    failure: alloc::vec::Vec<u8>,
    staged: Option<StagedObservation>,
}
#[derive(Clone, Copy)]
enum StagedObservation {
    Quantities(i16, u16),
    Unavailable,
}
impl PreparedObservationEmitter {
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        use super::template::material;
        let input = PreparedCanonicalInput::with_static_schema(
            contract.input_type(),
            include_bytes!(concat!(env!("OUT_DIR"), "/decoded.type")),
            1024,
        )?;
        if validate(material::RESULT)?.type_bytes() != contract.result_type().canonical_bytes()? {
            return Err(R::WrongType);
        }
        let failure_type = c::StructuredInfoType::from_canonical_bytes(material::FAILURE_TYPE)?;
        let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(&failure_type)
            .map_err(|_| R::CanonicalEncodingTooLarge)? as usize;
        let maximum_node = maximum
            .checked_sub(material::FAILURE_TYPE.len())
            .ok_or(R::CanonicalEncodingTooLarge)?;
        if material::UNAVAILABLE_PREFIX
            .len()
            .saturating_add(maximum_node)
            > material::RESULT_MAXIMUM
        {
            return Err(R::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            input,
            failure: alloc::vec::Vec::with_capacity(maximum_node + 1),
            staged: None,
        })
    }
    pub fn stage(&mut self, input: &[u8]) -> Result<usize, ObservationRefusal> {
        use super::template::material;
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
            // The same authored Forms validate before staging; execution does
            // not infer units or bypass the checked coordinate domain.
            forms::DeviceDht11TemperatureForm::decode(&temperature.to_le_bytes())?;
            forms::DeviceDht11HumidityForm::decode(&humidity.to_le_bytes())?;
            self.failure.clear();
            self.failure.push(0);
            self.failure.extend_from_slice(&temperature.to_le_bytes());
            self.failure.extend_from_slice(&humidity.to_le_bytes());
            self.staged = Some(StagedObservation::Quantities(temperature, humidity));
            Ok(material::RESULT.len())
        } else {
            let failure = decoded
                .variant_payload("unavailable")?
                .ok_or(R::WrongType)?;
            if failure.type_bytes() != material::FAILURE_TYPE {
                return Err(R::WrongType.into());
            }
            let bytes = failure.value_node();
            if bytes.len() + 1 > self.failure.capacity() {
                return Err(R::CanonicalEncodingTooLarge.into());
            }
            self.failure.clear();
            self.failure.push(1);
            self.failure.extend_from_slice(bytes);
            self.staged = Some(StagedObservation::Unavailable);
            Ok(material::UNAVAILABLE_PREFIX.len() + bytes.len())
        }
    }
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, ObservationRefusal> {
        use super::template::material;
        let length = match self.staged.ok_or(R::WrongType)? {
            StagedObservation::Quantities(temperature, humidity) => {
                let out = output
                    .get_mut(..material::RESULT.len())
                    .ok_or(R::CanonicalEncodingTooLarge)?;
                out.copy_from_slice(material::RESULT);
                out[material::RESULT_TEMPERATURE].copy_from_slice(
                    &forms::DeviceDht11TemperatureForm::decode(&temperature.to_le_bytes())?
                        .encode(),
                );
                out[material::RESULT_HUMIDITY].copy_from_slice(
                    &forms::DeviceDht11HumidityForm::decode(&humidity.to_le_bytes())?.encode(),
                );
                material::RESULT.len()
            }
            StagedObservation::Unavailable => {
                let length = material::UNAVAILABLE_PREFIX.len() + self.failure.len() - 1;
                let out = output
                    .get_mut(..length)
                    .ok_or(R::CanonicalEncodingTooLarge)?;
                let (prefix, node) = out.split_at_mut(material::UNAVAILABLE_PREFIX.len());
                prefix.copy_from_slice(material::UNAVAILABLE_PREFIX);
                node.copy_from_slice(&self.failure[1..]);
                length
            }
        };
        validate(&output[..length])?;
        Ok(length)
    }
    pub(crate) fn compact_output(&self) -> Option<&[u8]> {
        self.staged.map(|_| self.failure.as_slice())
    }
    pub fn clear(&mut self) {
        self.staged = None;
    }
}
