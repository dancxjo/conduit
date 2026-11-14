//! Exact-schema preparation and finite canonical I2C request decoding.
use super::{
    contract::{I2cContract, I2C_MAXIMUM_BYTES},
    transaction::{I2cDisposition, I2cTransaction, MAXIMUM_TRANSACTION_BYTES},
};
use conduit_core::{BorrowedStructuredValueValidator, StructuredInfoRefusal};

pub struct PreparedI2cDecoder {
    validator: BorrowedStructuredValueValidator<'static>,
}

pub struct DecodedI2cRequest {
    address: u8,
    write: [u8; MAXIMUM_TRANSACTION_BYTES],
    write_length: usize,
    read: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum I2cDecodeRefusal {
    Canonical(StructuredInfoRefusal),
    Geometry(I2cDisposition),
}

impl PreparedI2cDecoder {
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }

    pub fn new(_contract: &I2cContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: BorrowedStructuredValueValidator::new(
                include_bytes!(concat!(env!("OUT_DIR"), "/request.type")),
                I2C_MAXIMUM_BYTES as usize,
            )?,
        })
    }
    pub fn decode(&self, input: &[u8]) -> Result<DecodedI2cRequest, I2cDecodeRefusal> {
        let request = self.canonical(input).map_err(I2cDecodeRefusal::Canonical)?;
        request.transaction().map_err(I2cDecodeRefusal::Geometry)?;
        Ok(request)
    }
    fn canonical(&self, input: &[u8]) -> Result<DecodedI2cRequest, StructuredInfoRefusal> {
        let value = self.validator.validate(input)?;
        let field = |name| {
            value
                .record_field(name)?
                .ok_or(StructuredInfoRefusal::WrongRecordFields)
        };
        let address = field("address")?.primitive_bytes("value/u8")?;
        let read = field("read")?.primitive_bytes("value/u8")?;
        if address.len() != 1 || read.len() != 1 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let mut write = [0; MAXIMUM_TRANSACTION_BYTES];
        let write_length = field("write")?.copy_octet_collection("value/u8", &mut write)?;
        Ok(DecodedI2cRequest {
            address: address[0],
            write,
            write_length,
            read: read[0],
        })
    }
}

impl DecodedI2cRequest {
    pub fn transaction(&self) -> Result<I2cTransaction<'_>, I2cDisposition> {
        I2cTransaction::new(self.address, &self.write[..self.write_length], self.read)
    }
}
