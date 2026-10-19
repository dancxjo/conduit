//! Exact compiled Type contracts; no Source, Host or authority acceptance.
extern crate alloc;
use alloc::{format, string::String, vec::Vec};
use conduit_core::{CheckedValueContract, StructuredInfoType};
use conduit_plot::{CheckedNativeType, NativeTypeValueContract, PortableExpressionProgram};

pub type CompiledType = (
    String,
    String,
    Vec<u8>,
    Vec<(String, CheckedValueContract)>,
    Vec<Vec<u8>>,
);

// Finite build artifact expansion, independent of any individual Type's bounds.
// The Source identity, every checked Type and every law survive compression.
pub const MAXIMUM_AUTHORING_BYTES: usize = 32 * 1024 * 1024;

pub fn decode(bytes: &[u8], source_id: &str) -> Result<Vec<CheckedNativeType>, String> {
    if bytes.len() > MAXIMUM_AUTHORING_BYTES {
        return Err("compiled speech Types exceed their compressed byte bound".into());
    }
    let unpacked =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, MAXIMUM_AUTHORING_BYTES)
            .map_err(|error| format!("compiled speech Types decompression: {:?}", error.status))?;
    let bytes = unpacked.as_slice();
    let ((retained_source, types), remaining): ((String, Vec<CompiledType>), &[u8]) =
        postcard::take_from_bytes(bytes)
            .map_err(|error| format!("compiled speech Types: {error}"))?;
    if !remaining.is_empty() {
        return Err("compiled speech Types have trailing bytes".into());
    }
    if retained_source != source_id {
        return Err("compiled speech Types belong to another Source".into());
    }
    types
        .into_iter()
        .map(|(name, identity, value_type, contracts, invariants)| {
            Ok(CheckedNativeType {
                name,
                identity: conduit_core::kind_id(&identity),
                value_type: StructuredInfoType::from_canonical_bytes(&value_type)
                    .map_err(|error| format!("compiled speech Type: {error:?}"))?,
                value_contracts: contracts
                    .into_iter()
                    .map(|(representation_path, contract)| {
                        Ok(NativeTypeValueContract {
                            representation_path,
                            contract: CheckedValueContract::new(
                                contract.value_kind,
                                contract.maximum_bytes,
                                contract.constraints,
                            )
                            .map_err(|error| format!("compiled speech contract: {error:?}"))?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
                invariants: invariants
                    .into_iter()
                    .map(|program| {
                        PortableExpressionProgram::from_canonical_bytes(&program)
                            .map_err(|error| format!("compiled speech law: {error:?}"))
                    })
                    .collect::<Result<_, String>>()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_table() -> Vec<u8> {
        postcard::to_allocvec(&("source@1", Vec::<CompiledType>::new())).unwrap()
    }
    fn packed(bytes: &[u8]) -> Vec<u8> {
        miniz_oxide::deflate::compress_to_vec_zlib(bytes, 9)
    }
    #[test]
    fn compressed_metadata_retains_source_identity() {
        let bytes = packed(&empty_table());
        assert!(decode(&bytes, "source@1").unwrap().is_empty());
        assert!(decode(&bytes, "source@2")
            .unwrap_err()
            .contains("another Source"));
    }
    #[test]
    fn corrupted_or_truncated_compression_refuses() {
        let bytes = packed(&empty_table());
        assert!(decode(&bytes[..bytes.len() - 1], "source@1").is_err());
        let mut corrupted = bytes;
        corrupted[2] ^= 0x40;
        assert!(decode(&corrupted, "source@1").is_err());
    }
    #[test]
    fn decoded_trailing_bytes_do_not_become_another_type_table() {
        let mut bytes = empty_table();
        bytes.push(0);
        assert!(decode(&packed(&bytes), "source@1")
            .unwrap_err()
            .contains("trailing bytes"));
    }
    #[test]
    fn compressed_input_and_expansion_have_finite_bounds() {
        assert!(
            decode(&alloc::vec![0; MAXIMUM_AUTHORING_BYTES + 1], "source@1")
                .unwrap_err()
                .contains("compressed byte bound")
        );
        let expansion = packed(&alloc::vec![0; MAXIMUM_AUTHORING_BYTES + 1]);
        assert!(decode(&expansion, "source@1")
            .unwrap_err()
            .contains("HasMoreOutput"));
    }
}
