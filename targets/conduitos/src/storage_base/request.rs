//! Exact canonical request validation, with no allocation in Play.
use super::contract::{BLOCK_READ_DATA_BYTES, BlockReadContract};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};
#[derive(Debug, PartialEq, Eq)]
pub enum BlockReadRequestRefusal {
    Canonical(StructuredInfoRefusal),
    Geometry,
    Address,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockReadRequest {
    pub block_size: u16,
    pub count: u16,
    pub lba: u32,
}
impl BlockReadRequest {
    pub fn checked(block_size: u64, count: u64, lba: u64) -> Result<Self, BlockReadRequestRefusal> {
        if block_size != u64::from(BLOCK_READ_DATA_BYTES) || count != 1 {
            return Err(BlockReadRequestRefusal::Geometry);
        }
        Ok(Self {
            block_size: BLOCK_READ_DATA_BYTES,
            count: 1,
            lba: u32::try_from(lba).map_err(|_| BlockReadRequestRefusal::Address)?,
        })
    }
}
pub struct PreparedBlockReadRequestDecoder {
    validator: PreparedStructuredValueValidator,
}
impl PreparedBlockReadRequestDecoder {
    pub fn new(contract: &BlockReadContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode(&self, bytes: &[u8]) -> Result<BlockReadRequest, BlockReadRequestRefusal> {
        use BlockReadRequestRefusal as E;
        self.validator.validate(bytes).map_err(E::Canonical)?;
        let value = validate_canonical_structured_value(bytes).map_err(E::Canonical)?;
        let integer = |name| -> Result<u64, E> {
            let bytes = value
                .record_field(name)
                .map_err(E::Canonical)?
                .ok_or(E::Canonical(StructuredInfoRefusal::WrongRecordFields))?
                .primitive_bytes("value/u64")
                .map_err(E::Canonical)?;
            Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| {
                E::Canonical(StructuredInfoRefusal::WrongType)
            })?))
        };
        BlockReadRequest::checked(integer("block_size")?, integer("count")?, integer("lba")?)
    }
}
