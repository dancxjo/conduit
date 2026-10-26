//! Exact composition of the two authored quantity Forms for compact retention.
//! These bytes are representation material, not a new observation Type or a
//! self-identifying transport record. The selected session must retain the exact
//! semantic contract and both Form identities, plus its independent provenance.
use super::{
    forms, template::QuantityTemplate, ObservationContract, ObservationRefusal, MAXIMUM_BYTES,
};
use crate::canonical_input::PreparedCanonicalInput;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredValueValidator, Quantity,
    StructuredInfoRefusal as R,
};

pub struct PreparedObservationQuantityForms {
    validator: PreparedCanonicalInput,
    observation: QuantityTemplate<false>,
}
impl PreparedObservationQuantityForms {
    pub const TEMPERATURE_FORM_ID: &'static str = forms::DeviceDht11TemperatureForm::IDENTITY;
    pub const HUMIDITY_FORM_ID: &'static str = forms::DeviceDht11HumidityForm::IDENTITY;
    /// Temperature Form followed by humidity Form, two bytes each. No rounding.
    pub const PACKED_BYTES: usize = 4;

    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        let ty = contract.observation_type();
        drop(PreparedStructuredValueValidator::new(
            ty,
            MAXIMUM_BYTES as usize,
        )?);
        Ok(Self {
            validator: PreparedCanonicalInput::with_static_schema(
                ty,
                include_bytes!(concat!(env!("OUT_DIR"), "/observation.type")),
                MAXIMUM_BYTES as usize,
            )?,
            observation: QuantityTemplate::prepare(ty)?,
        })
    }

    /// Accept only the exact canonical observation Type selected at preparation.
    /// Inexact and overflowing quantities refuse rather than becoming display
    /// rounding. Output is owned so persistence need not hold this codec borrowed.
    pub fn pack(&mut self, canonical: &[u8]) -> Result<[u8; 4], ObservationRefusal> {
        self.validator.validate(canonical)?;
        let observation = validate(canonical)?;
        let quantity = |name| -> Result<Quantity, ObservationRefusal> {
            let leaf = observation
                .record_field(name)?
                .ok_or(R::WrongRecordFields)?;
            Quantity::decode(
                leaf.value_node()
                    .get(5..)
                    .ok_or(R::MalformedCanonicalEncoding)?,
            )
            .map_err(|_| R::MalformedCanonicalEncoding.into())
        };
        Self::pack_quantities(quantity("temperature")?, quantity("humidity")?)
    }
    /// Representation only. Callers must hold the independently validated
    /// observation/producer scope; encoding quantities confers no provenance.
    pub fn pack_quantities(
        temperature: Quantity,
        humidity: Quantity,
    ) -> Result<[u8; 4], ObservationRefusal> {
        let temperature = forms::DeviceDht11TemperatureForm::encode(temperature)?;
        let humidity = forms::DeviceDht11HumidityForm::encode(humidity)?;
        Ok([temperature[0], temperature[1], humidity[0], humidity[1]])
    }

    /// Reconstruct canonical first-class quantities, not integer sensor fields.
    /// The returned borrow expires at the next decode; there is no storage effect.
    pub fn unpack(&mut self, packed: &[u8]) -> Result<&[u8], ObservationRefusal> {
        if packed.len() != Self::PACKED_BYTES {
            return Err(conduit_core::QuantityFormRefusal::WrongLength.into());
        }
        let (temperature, humidity) = Self::unpack_quantities(packed)?;
        self.observation
            .quantities(temperature, humidity)
            .map_err(Into::into)
    }
    pub fn unpack_quantities(packed: &[u8]) -> Result<(Quantity, Quantity), ObservationRefusal> {
        if packed.len() != Self::PACKED_BYTES {
            return Err(conduit_core::QuantityFormRefusal::WrongLength.into());
        }
        Ok((
            forms::DeviceDht11TemperatureForm::decode(&packed[..2])?,
            forms::DeviceDht11HumidityForm::decode(&packed[2..])?,
        ))
    }
}
