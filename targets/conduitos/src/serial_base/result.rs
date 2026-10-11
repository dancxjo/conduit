//! Provider observations preserve exact byte extents and direction.
use super::contract::{
    SerialDuplexContract, SERIAL_DUPLEX_DATA_BYTES, SERIAL_DUPLEX_MAXIMUM_BYTES,
};
use alloc::vec::Vec;
use conduit_core::{
    kind_id, validate_canonical_structured_value, PreparedStructuredComposer,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SerialDisposition {
    Stalled,
    ProviderLost,
    Unsupported,
    Timeout,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SerialResultRefusal {
    Canonical(StructuredInfoRefusal),
    Extent,
}
pub struct PreparedSerialResultEncoder {
    result: PreparedStructuredComposer,
    transmitted: PreparedStructuredComposer,
    received: PreparedStructuredComposer,
    ordinal: PreparedStructuredComposer,
    requested: PreparedStructuredComposer,
    actual: PreparedStructuredComposer,
    bytes: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
    unit: Vec<u8>,
}
impl PreparedSerialResultEncoder {
    pub fn new(contract: &SerialDuplexContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let payload = |tag| {
            cases
                .iter()
                .find(|case| case.tag() == tag)
                .map(|case| case.payload_type())
                .ok_or(StructuredInfoRefusal::UnknownVariantTag)
        };
        let primitive = |name| StructuredInfoType::leaf(kind_id(name));
        let integer = || PreparedStructuredComposer::new(&primitive("value/u64")?, 64);
        let mut unit = PreparedStructuredComposer::new(&primitive("value/empty")?, 64)?;
        let unit = unit.leaf(&[])?.to_vec();
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                SERIAL_DUPLEX_MAXIMUM_BYTES as usize,
            )?,
            transmitted: PreparedStructuredComposer::new(
                payload("transmitted")?,
                SERIAL_DUPLEX_MAXIMUM_BYTES as usize,
            )?,
            received: PreparedStructuredComposer::new(
                payload("received")?,
                SERIAL_DUPLEX_MAXIMUM_BYTES as usize,
            )?,
            ordinal: integer()?,
            requested: integer()?,
            actual: integer()?,
            bytes: PreparedStructuredComposer::new(
                &primitive("value/bytes")?,
                SERIAL_DUPLEX_MAXIMUM_BYTES as usize,
            )?,
            short: PreparedStructuredComposer::new(&primitive("value/bool")?, 64)?,
            unit,
        })
    }
    pub fn transmitted(
        &mut self,
        ordinal: u64,
        requested: u16,
        actual: u16,
    ) -> Result<&[u8], SerialResultRefusal> {
        use SerialResultRefusal as E;
        if requested > SERIAL_DUPLEX_DATA_BYTES || actual > requested {
            return Err(E::Extent);
        }
        let actual = self
            .actual
            .leaf(&u64::from(actual).to_le_bytes())
            .map_err(E::Canonical)?;
        let ordinal = self
            .ordinal
            .leaf(&ordinal.to_le_bytes())
            .map_err(E::Canonical)?;
        let requested = self
            .requested
            .leaf(&u64::from(requested).to_le_bytes())
            .map_err(E::Canonical)?;
        let payload = self
            .transmitted
            .record(&[
                validate_canonical_structured_value(actual).map_err(E::Canonical)?,
                validate_canonical_structured_value(ordinal).map_err(E::Canonical)?,
                validate_canonical_structured_value(requested).map_err(E::Canonical)?,
            ])
            .map_err(E::Canonical)?;
        self.result
            .variant(
                "transmitted",
                validate_canonical_structured_value(payload).map_err(E::Canonical)?,
            )
            .map_err(E::Canonical)
    }
    pub fn received(
        &mut self,
        ordinal: u64,
        requested: u16,
        actual: u16,
        bytes: &[u8],
    ) -> Result<&[u8], SerialResultRefusal> {
        use SerialResultRefusal as E;
        if requested == 0
            || requested > SERIAL_DUPLEX_DATA_BYTES
            || actual > requested
            || bytes.len() != usize::from(actual)
        {
            return Err(E::Extent);
        }
        let actual_value = self
            .actual
            .leaf(&u64::from(actual).to_le_bytes())
            .map_err(E::Canonical)?;
        let bytes = self.bytes.leaf(bytes).map_err(E::Canonical)?;
        let ordinal = self
            .ordinal
            .leaf(&ordinal.to_le_bytes())
            .map_err(E::Canonical)?;
        let requested_value = self
            .requested
            .leaf(&u64::from(requested).to_le_bytes())
            .map_err(E::Canonical)?;
        let short = self
            .short
            .leaf(&[u8::from(actual < requested)])
            .map_err(E::Canonical)?;
        let payload = self
            .received
            .record(&[
                validate_canonical_structured_value(actual_value).map_err(E::Canonical)?,
                validate_canonical_structured_value(bytes).map_err(E::Canonical)?,
                validate_canonical_structured_value(ordinal).map_err(E::Canonical)?,
                validate_canonical_structured_value(requested_value).map_err(E::Canonical)?,
                validate_canonical_structured_value(short).map_err(E::Canonical)?,
            ])
            .map_err(E::Canonical)?;
        self.result
            .variant(
                "received",
                validate_canonical_structured_value(payload).map_err(E::Canonical)?,
            )
            .map_err(E::Canonical)
    }
    pub fn disposition(
        &mut self,
        disposition: SerialDisposition,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let tag = match disposition {
            SerialDisposition::Stalled => "stalled",
            SerialDisposition::ProviderLost => "provider-lost",
            SerialDisposition::Unsupported => "unsupported",
            SerialDisposition::Timeout => "timeout",
        };
        self.result
            .variant(tag, validate_canonical_structured_value(&self.unit)?)
    }
}
