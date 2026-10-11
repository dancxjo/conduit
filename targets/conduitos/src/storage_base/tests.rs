//! Pure contract evidence, not storage transport or admitted provider proof.
use super::{contract::*, request::*, result::*};
use alloc::vec::Vec;
use conduit_core::{
    StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    validate_canonical_structured_value,
};
fn request_bytes(contract: &BlockReadContract, size: u64, count: u64, lba: u64) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("request record")
    };
    let values = fields
        .iter()
        .zip([size, count, lba])
        .map(|(field, value)| {
            StructuredFieldValue::new(
                field.name(),
                StructuredInfoValue::leaf(field.value_type().clone(), value.to_le_bytes().to_vec())
                    .unwrap(),
            )
        })
        .collect();
    StructuredInfoValue::record(contract.request_type().clone(), values)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn checked_initial_geometry_and_full_lba_bound_are_explicit() {
    let contract = BlockReadContract::prepare().unwrap();
    let decoder = PreparedBlockReadRequestDecoder::new(&contract).unwrap();
    assert_eq!(
        decoder
            .decode(&request_bytes(&contract, 512, 1, u64::from(u32::MAX)))
            .unwrap()
            .lba,
        u32::MAX
    );
    for (size, count, lba) in [
        (0, 1, 0),
        (4096, 1, 0),
        (512, 0, 0),
        (512, 2, 0),
        (512, 1, u64::from(u32::MAX) + 1),
    ] {
        assert!(
            decoder
                .decode(&request_bytes(&contract, size, count, lba))
                .is_err()
        );
    }
    assert!(decoder.decode(&[0; 12]).is_err());
}
#[test]
fn exact_kind_catalogs_are_inert_and_preserve_nested_byte_bounds() {
    let contract = BlockReadContract::prepare().unwrap();
    assert_eq!(contract.kind().kind_id.as_str(), BLOCK_READ_KIND);
    assert_eq!(contract.kind().inputs.len(), 1);
    assert_eq!(contract.kind().outputs.len(), 1);
    let _catalogs = contract.catalogs();
    let encoder = PreparedBlockReadResultEncoder::new(&contract).unwrap();
    drop(encoder);
}
#[test]
fn complete_and_incomplete_preserve_request_ordinal_and_storage_residue() {
    let contract = BlockReadContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(&contract).unwrap();
    let request = BlockReadRequest::checked(512, 1, 7).unwrap();
    for (length, tag, residue) in [
        (512, "completed", 0),
        (17, "incomplete", 495),
        (0, "incomplete", 512),
    ] {
        let bytes = alloc::vec![0xa5; length];
        let result = encoder.observed(request, 9, &bytes, residue).unwrap();
        let value = validate_canonical_structured_value(result).unwrap();
        assert_eq!(value.variant_tag().unwrap(), tag);
        let payload = value.variant_payload(tag).unwrap().unwrap();
        let integer = |name| {
            u64::from_le_bytes(
                payload
                    .record_field(name)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(integer("actual"), length as u64);
        assert_eq!(integer("residue"), u64::from(residue));
        assert_eq!(integer("ordinal"), 9);
        assert_eq!(integer("lba"), 7);
        assert_eq!(integer("count"), 1);
        assert_eq!(integer("block_size"), 512);
        assert_eq!(
            payload
                .record_field("bytes")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bytes")
                .unwrap(),
            bytes
        );
        assert_eq!(
            payload
                .record_field("short")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[u8::from(length != 512)]
        );
    }
    assert!(encoder.observed(request, 10, &[0; 513], 0).is_err());
    assert!(encoder.observed(request, 10, &[0; 17], 0).is_err());
    assert!(
        encoder
            .observed(
                BlockReadRequest {
                    block_size: 1,
                    ..request
                },
                10,
                &[0; 512],
                0
            )
            .is_err()
    );
}
#[test]
fn terminal_dispositions_preserve_request_identity_without_committed_bytes() {
    let contract = BlockReadContract::prepare().unwrap();
    let mut encoder = PreparedBlockReadResultEncoder::new(&contract).unwrap();
    for (disposition, tag) in [
        (BlockReadDisposition::Stalled, "stalled"),
        (BlockReadDisposition::ProviderLost, "provider-lost"),
        (BlockReadDisposition::Unsupported, "unsupported"),
        (BlockReadDisposition::Timeout, "timeout"),
        (BlockReadDisposition::CommandFailed, "command-failed"),
    ] {
        let result = encoder
            .disposition(
                BlockReadRequest::checked(512, 1, 23).unwrap(),
                15,
                disposition,
            )
            .unwrap();
        let value = validate_canonical_structured_value(result).unwrap();
        assert_eq!(value.variant_tag().unwrap(), tag);
        let payload = value.variant_payload(tag).unwrap().unwrap();
        assert_eq!(
            payload
                .record_field("ordinal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            &15_u64.to_le_bytes()
        );
        assert!(payload.record_field("bytes").unwrap().is_none());
    }
}
