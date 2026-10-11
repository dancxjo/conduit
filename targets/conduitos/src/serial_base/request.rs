//! Checked requests borrow bounded storage; decoding allocates nothing in Play.
use super::contract::{SERIAL_DUPLEX_DATA_BYTES, SerialDuplexContract};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

#[derive(Debug, PartialEq, Eq)]
pub enum SerialRequestRefusal {
    Canonical(StructuredInfoRefusal),
    Length,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SerialRequest<'a> {
    Transmit(&'a [u8]),
    Receive(u16),
}
impl SerialRequest<'_> {
    pub fn requested(&self) -> u16 {
        match self {
            Self::Transmit(bytes) => bytes.len() as u16,
            Self::Receive(maximum) => *maximum,
        }
    }
}
pub struct PreparedSerialRequestDecoder {
    validator: PreparedStructuredValueValidator,
}
impl PreparedSerialRequestDecoder {
    pub fn new(contract: &SerialDuplexContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode<'a>(&self, bytes: &'a [u8]) -> Result<SerialRequest<'a>, SerialRequestRefusal> {
        use SerialRequestRefusal as E;
        self.validator.validate(bytes).map_err(E::Canonical)?;
        let value = validate_canonical_structured_value(bytes).map_err(E::Canonical)?;
        let tag = value.variant_tag().map_err(E::Canonical)?;
        let payload = value
            .variant_payload(tag)
            .map_err(E::Canonical)?
            .ok_or(E::Canonical(StructuredInfoRefusal::UnknownVariantTag))?;
        let field = |name| {
            payload
                .record_field(name)
                .map_err(E::Canonical)?
                .ok_or(E::Canonical(StructuredInfoRefusal::WrongRecordFields))
        };
        match tag {
            "transmit" => {
                let data = field("bytes")?
                    .primitive_bytes("value/bytes")
                    .map_err(E::Canonical)?;
                if data.len() > usize::from(SERIAL_DUPLEX_DATA_BYTES) {
                    return Err(E::Length);
                }
                Ok(SerialRequest::Transmit(data))
            }
            "receive" => {
                let data = field("maximum")?
                    .primitive_bytes("value/u64")
                    .map_err(E::Canonical)?;
                let maximum = u64::from_le_bytes(data.try_into().map_err(|_| E::Length)?);
                if maximum == 0 || maximum > u64::from(SERIAL_DUPLEX_DATA_BYTES) {
                    return Err(E::Length);
                }
                Ok(SerialRequest::Receive(maximum as u16))
            }
            _ => Err(E::Canonical(StructuredInfoRefusal::UnknownVariantTag)),
        }
    }
}
