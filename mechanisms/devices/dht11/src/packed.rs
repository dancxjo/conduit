//! Exact composition of the two authored quantity Forms for compact retention.
//! These bytes are representation material, not a new observation Type or a
//! self-identifying transport record. The selected session must retain the exact
//! semantic contract and both Form identities, plus its independent provenance.
use super::{field, forms, ObservationContract, ObservationRefusal, MAXIMUM_BYTES};
use conduit_core::{
    validate_canonical_structured_value as validate, BorrowedStructuredValueValidator,
    PreparedStructuredComposer as Composer, Quantity, StructuredInfoRefusal as R,
    QUANTITY_ENCODED_LEN,
};

/// Write-only exact quantity Forms. Linked schema validation needs no owned
/// composer output or schema tree. This creates no storage/admission authority.
pub struct PreparedObservationQuantityPacker {
    validator: BorrowedStructuredValueValidator<'static>,
}
impl PreparedObservationQuantityPacker {
    pub const TEMPERATURE_FORM_ID: &'static str = forms::DeviceDht11TemperatureForm::IDENTITY;
    pub const HUMIDITY_FORM_ID: &'static str = forms::DeviceDht11HumidityForm::IDENTITY;
    pub const PACKED_BYTES: usize = 4;
    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        const OBSERVATION_SCHEMA: &[u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/observation.type"));
        if contract.observation_type().canonical_bytes()? != OBSERVATION_SCHEMA {
            return Err(R::WrongType);
        }
        Ok(Self {
            validator: BorrowedStructuredValueValidator::new(
                OBSERVATION_SCHEMA,
                MAXIMUM_BYTES as usize,
            )?,
        })
    }
    /// Owned heap backing only; inline Self and shared immutable linked schema
    /// are separate. No heap allocation is retained by this pack-only codec.
    pub const fn retained_backing_bytes(&self) -> usize {
        0
    }
    /// Accept only the exact canonical observation Type selected at preparation.
    /// Inexact and overflowing quantities refuse rather than becoming display
    /// rounding. Output is owned so persistence need not hold this codec borrowed.
    pub fn pack(&self, canonical: &[u8]) -> Result<[u8; 4], ObservationRefusal> {
        let observation = self.validator.validate(canonical)?;
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
        let temperature = forms::DeviceDht11TemperatureForm::encode(quantity("temperature")?)?;
        let humidity = forms::DeviceDht11HumidityForm::encode(quantity("humidity")?)?;
        Ok([temperature[0], temperature[1], humidity[0], humidity[1]])
    }
}

pub struct PreparedObservationQuantityForms {
    packer: PreparedObservationQuantityPacker,
    temperature: Composer,
    humidity: Composer,
    observation: Composer,
}
impl PreparedObservationQuantityForms {
    pub const TEMPERATURE_FORM_ID: &'static str = forms::DeviceDht11TemperatureForm::IDENTITY;
    pub const HUMIDITY_FORM_ID: &'static str = forms::DeviceDht11HumidityForm::IDENTITY;
    /// Temperature Form followed by humidity Form, two bytes each. No rounding.
    pub const PACKED_BYTES: usize = 4;

    pub fn prepare(contract: &ObservationContract) -> Result<Self, R> {
        let ty = contract.observation_type();
        const OBSERVATION_SCHEMA: &[u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/observation.type"));
        let packer = PreparedObservationQuantityPacker::prepare(contract)?;
        let leaf = |name, schema| {
            let ty = field(ty, name)?;
            Composer::with_static_schema(
                ty,
                schema,
                ty.canonical_bytes()?.len() + 5 + QUANTITY_ENCODED_LEN,
            )
        };
        Ok(Self {
            packer,
            temperature: leaf(
                "temperature",
                include_bytes!(concat!(env!("OUT_DIR"), "/observation-temperature.type")),
            )?,
            humidity: leaf(
                "humidity",
                include_bytes!(concat!(env!("OUT_DIR"), "/observation-humidity.type")),
            )?,
            observation: Composer::with_static_schema(
                ty,
                OBSERVATION_SCHEMA,
                MAXIMUM_BYTES as usize,
            )?,
        })
    }

    /// Exact write conversion, shared with the pack-only codec.
    pub fn pack(&mut self, canonical: &[u8]) -> Result<[u8; 4], ObservationRefusal> {
        self.packer.pack(canonical)
    }
    /// All owned composer buffers and field-range tables; inline Self and
    /// shared immutable schemas are separate from this checked heap sum.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.packer
            .retained_backing_bytes()
            .checked_add(self.temperature.prepared_heap_bytes())?
            .checked_add(self.humidity.prepared_heap_bytes())?
            .checked_add(self.observation.prepared_heap_bytes())
    }

    /// Reconstruct canonical first-class quantities, not integer sensor fields.
    /// The returned borrow expires at the next decode; there is no storage effect.
    pub fn unpack(&mut self, packed: &[u8]) -> Result<&[u8], ObservationRefusal> {
        if packed.len() != Self::PACKED_BYTES {
            return Err(conduit_core::QuantityFormRefusal::WrongLength.into());
        }
        let temperature = forms::DeviceDht11TemperatureForm::decode(&packed[..2])?;
        let humidity = forms::DeviceDht11HumidityForm::decode(&packed[2..])?;
        self.temperature.leaf(&temperature.encode())?;
        self.humidity.leaf(&humidity.encode())?;
        self.observation.record(&[
            validate(self.humidity.encoded())?,
            validate(self.temperature.encoded())?,
        ])?;
        Ok(self.observation.encoded())
    }
}
