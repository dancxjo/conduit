//! Bounded storage observations; transport/class integrity is verified elsewhere.
use super::{
    contract::{BLOCK_READ_DATA_BYTES, BLOCK_READ_MAXIMUM_BYTES, BlockReadContract},
    request::BlockReadRequest,
};
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape,
    kind_id, validate_canonical_structured_value,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReadDisposition {
    Stalled,
    ProviderLost,
    Unsupported,
    Timeout,
    CommandFailed,
}
#[derive(Debug, PartialEq, Eq)]
pub enum BlockReadResultRefusal {
    Canonical(StructuredInfoRefusal),
    Geometry,
    Extent,
}
pub struct PreparedBlockReadResultEncoder {
    result: PreparedStructuredComposer,
    observation: PreparedStructuredComposer,
    identity: PreparedStructuredComposer,
    actual: PreparedStructuredComposer,
    block_size: PreparedStructuredComposer,
    bytes: PreparedStructuredComposer,
    count: PreparedStructuredComposer,
    lba: PreparedStructuredComposer,
    ordinal: PreparedStructuredComposer,
    residue: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
}
impl PreparedBlockReadResultEncoder {
    pub fn new(contract: &BlockReadContract) -> Result<Self, StructuredInfoRefusal> {
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
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )?,
            observation: PreparedStructuredComposer::new(
                payload("completed")?,
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )?,
            identity: PreparedStructuredComposer::new(
                payload("unsupported")?,
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )?,
            actual: integer()?,
            block_size: integer()?,
            count: integer()?,
            lba: integer()?,
            ordinal: integer()?,
            residue: integer()?,
            bytes: PreparedStructuredComposer::new(
                &primitive("value/bytes")?,
                BLOCK_READ_MAXIMUM_BYTES as usize,
            )?,
            short: PreparedStructuredComposer::new(&primitive("value/bool")?, 64)?,
        })
    }
    /// Complete only after the selected implementation has verified its own
    /// protocol integrity. This encoder cannot turn raw transport bytes into proof.
    pub fn observed(
        &mut self,
        request: BlockReadRequest,
        ordinal: u64,
        bytes: &[u8],
        residue: u16,
    ) -> Result<&[u8], BlockReadResultRefusal> {
        use BlockReadResultRefusal as E;
        let request = BlockReadRequest::checked(
            u64::from(request.block_size),
            u64::from(request.count),
            u64::from(request.lba),
        )
        .map_err(|_| E::Geometry)?;
        if bytes.len() > usize::from(BLOCK_READ_DATA_BYTES)
            || bytes.len() + usize::from(residue) != usize::from(BLOCK_READ_DATA_BYTES)
        {
            return Err(E::Extent);
        }
        let completed = residue == 0;
        let actual = self
            .actual
            .leaf(&(bytes.len() as u64).to_le_bytes())
            .map_err(E::Canonical)?;
        let block_size = self
            .block_size
            .leaf(&u64::from(request.block_size).to_le_bytes())
            .map_err(E::Canonical)?;
        let bytes = self.bytes.leaf(bytes).map_err(E::Canonical)?;
        let count = self
            .count
            .leaf(&u64::from(request.count).to_le_bytes())
            .map_err(E::Canonical)?;
        let lba = self
            .lba
            .leaf(&u64::from(request.lba).to_le_bytes())
            .map_err(E::Canonical)?;
        let ordinal = self
            .ordinal
            .leaf(&ordinal.to_le_bytes())
            .map_err(E::Canonical)?;
        let residue = self
            .residue
            .leaf(&u64::from(residue).to_le_bytes())
            .map_err(E::Canonical)?;
        let short = self
            .short
            .leaf(&[u8::from(!completed)])
            .map_err(E::Canonical)?;
        let payload = self
            .observation
            .record(&[
                validate_canonical_structured_value(actual).map_err(E::Canonical)?,
                validate_canonical_structured_value(block_size).map_err(E::Canonical)?,
                validate_canonical_structured_value(bytes).map_err(E::Canonical)?,
                validate_canonical_structured_value(count).map_err(E::Canonical)?,
                validate_canonical_structured_value(lba).map_err(E::Canonical)?,
                validate_canonical_structured_value(ordinal).map_err(E::Canonical)?,
                validate_canonical_structured_value(residue).map_err(E::Canonical)?,
                validate_canonical_structured_value(short).map_err(E::Canonical)?,
            ])
            .map_err(E::Canonical)?;
        self.result
            .variant(
                if completed { "completed" } else { "incomplete" },
                validate_canonical_structured_value(payload).map_err(E::Canonical)?,
            )
            .map_err(E::Canonical)
    }
    pub fn disposition(
        &mut self,
        request: BlockReadRequest,
        ordinal: u64,
        disposition: BlockReadDisposition,
    ) -> Result<&[u8], BlockReadResultRefusal> {
        use BlockReadResultRefusal as E;
        let request = BlockReadRequest::checked(
            u64::from(request.block_size),
            u64::from(request.count),
            u64::from(request.lba),
        )
        .map_err(|_| E::Geometry)?;
        let block_size = self
            .block_size
            .leaf(&u64::from(request.block_size).to_le_bytes())
            .map_err(E::Canonical)?;
        let count = self
            .count
            .leaf(&u64::from(request.count).to_le_bytes())
            .map_err(E::Canonical)?;
        let lba = self
            .lba
            .leaf(&u64::from(request.lba).to_le_bytes())
            .map_err(E::Canonical)?;
        let ordinal = self
            .ordinal
            .leaf(&ordinal.to_le_bytes())
            .map_err(E::Canonical)?;
        let payload = self
            .identity
            .record(&[
                validate_canonical_structured_value(block_size).map_err(E::Canonical)?,
                validate_canonical_structured_value(count).map_err(E::Canonical)?,
                validate_canonical_structured_value(lba).map_err(E::Canonical)?,
                validate_canonical_structured_value(ordinal).map_err(E::Canonical)?,
            ])
            .map_err(E::Canonical)?;
        let tag = match disposition {
            BlockReadDisposition::Stalled => "stalled",
            BlockReadDisposition::ProviderLost => "provider-lost",
            BlockReadDisposition::Unsupported => "unsupported",
            BlockReadDisposition::Timeout => "timeout",
            BlockReadDisposition::CommandFailed => "command-failed",
        };
        self.result
            .variant(
                tag,
                validate_canonical_structured_value(payload).map_err(E::Canonical)?,
            )
            .map_err(E::Canonical)
    }
}
