//! Preparation-side canonical Forms for the ordinary history read ports.
use super::contract::HISTORY_MAXIMUM_BYTES;
use super::{HistoryReadContract, MAXIMUM_SIGNED_BATCH_BYTES};
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, PreparedStructuredComposer, StructuredInfoRefusal,
    StructuredInfoTypeShape,
};

pub fn encode_request(
    contract: &HistoryReadContract,
    cursor: u32,
) -> Result<Vec<u8>, StructuredInfoRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        return Err(StructuredInfoRefusal::WrongType);
    };
    let field = fields
        .iter()
        .find(|field| field.name() == "cursor")
        .ok_or(StructuredInfoRefusal::WrongRecordFields)?;
    let mut leaf = PreparedStructuredComposer::new(field.value_type(), 64)?;
    let bytes = leaf.leaf(&cursor.to_le_bytes())?;
    let mut request =
        PreparedStructuredComposer::new(contract.request_type(), HISTORY_MAXIMUM_BYTES as usize)?;
    Ok(request
        .record(&[validate_canonical_structured_value(bytes)?])?
        .to_vec())
}
/// Prepared nominal Type prefix plus the existing Core primitive composer.
pub struct PreparedSignedBatchEncoder {
    output: Vec<u8>,
    prefix: usize,
    leaf_prefix: usize,
    leaf: PreparedStructuredComposer,
}
impl PreparedSignedBatchEncoder {
    pub fn new(contract: &HistoryReadContract) -> Result<Self, StructuredInfoRefusal> {
        let StructuredInfoTypeShape::Nominal { representation, .. } =
            contract.result_type().shape()
        else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let mut output = Vec::with_capacity(HISTORY_MAXIMUM_BYTES as usize);
        output.extend_from_slice(&contract.result_type().canonical_bytes()?);
        let prefix = output.len();
        if prefix + 5 + MAXIMUM_SIGNED_BATCH_BYTES > HISTORY_MAXIMUM_BYTES as usize {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            output,
            prefix,
            leaf_prefix: representation.canonical_bytes()?.len(),
            leaf: PreparedStructuredComposer::new(representation, HISTORY_MAXIMUM_BYTES as usize)?,
        })
    }
    pub fn encode(&mut self, batch: &[u8]) -> Result<&[u8], StructuredInfoRefusal> {
        if batch.len() > MAXIMUM_SIGNED_BATCH_BYTES {
            return Err(StructuredInfoRefusal::LeafTooLarge);
        }
        let leaf = self.leaf.leaf(batch)?;
        self.output.truncate(self.prefix);
        self.output.extend_from_slice(&leaf[self.leaf_prefix..]);
        Ok(&self.output)
    }
}
pub fn encode_result(
    contract: &HistoryReadContract,
    batch: &[u8],
) -> Result<Vec<u8>, StructuredInfoRefusal> {
    Ok(PreparedSignedBatchEncoder::new(contract)?
        .encode(batch)?
        .to_vec())
}
pub fn decode_result<'a>(
    contract: &HistoryReadContract,
    input: &'a [u8],
) -> Result<&'a [u8], StructuredInfoRefusal> {
    conduit_core::PreparedStructuredValueValidator::new(
        contract.result_type(),
        HISTORY_MAXIMUM_BYTES as usize,
    )?
    .validate(input)?;
    let bytes = validate_canonical_structured_value(input)?
        .nominal_representation()?
        .primitive_bytes("value/bytes")?;
    if bytes.len() > MAXIMUM_SIGNED_BATCH_BYTES {
        return Err(StructuredInfoRefusal::LeafTooLarge);
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_cursor_and_finite_signed_payload() {
        let source = super::super::contract::HISTORY_TYPES
            .split("plot history-read")
            .next()
            .unwrap();
        let syntax = conduit_plot::parse_syntax_document(source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked =
            conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new());
        assert!(checked.is_ok(), "{:?}", checked.err());
        let contract = HistoryReadContract::prepare().unwrap();
        assert_eq!(contract.checked_document().unwrap().plots.len(), 1);
        let request = encode_request(&contract, 0x0400000f).unwrap();
        contract
            .request_validator()
            .unwrap()
            .validate(&request)
            .unwrap();
        assert_eq!(
            super::super::owner::decode_request(&request).unwrap(),
            0x0400000f
        );
        let payload = [7; MAXIMUM_SIGNED_BATCH_BYTES];
        let result = encode_result(&contract, &payload).unwrap();
        assert_eq!(decode_result(&contract, &result).unwrap(), payload);
        assert!(result.len() <= HISTORY_MAXIMUM_BYTES as usize);
        assert!(encode_result(&contract, &[0; MAXIMUM_SIGNED_BATCH_BYTES + 1]).is_err());
        assert!(decode_result(&contract, &request).is_err());
    }
}
