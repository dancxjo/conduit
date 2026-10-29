//! Exact finite Collection codec and incompatible old private wire refusal.
use conduitos_creche_core::{data, wire::*};
#[test]
fn collection_extent_roundtrips_and_overflow_refuses_without_partial_output() {
    let value = "界".repeat(60_000);
    let bytes = data::encode(&value).unwrap();
    assert!(bytes.len() > 128 * 1024);
    assert!(bytes.len() <= MAX_BLOB_BYTES as usize);
    assert_eq!(data::decode::<String>(&bytes).unwrap(), value);
    assert!(data::encode(&"界".repeat(90_000)).is_err());
    assert!(data::decode::<String>(&vec![b'x'; MAX_BLOB_BYTES as usize + 1]).is_err());
    assert!(data::decode::<String>(b"\"unfinished").is_err());
}
#[test]
fn old_private_receiver_version_cannot_be_mistaken_for_collection_admission() {
    let mut exchange = [0; EXCHANGE_BYTES];
    let n = encode_request(
        Envelope {
            instance: 1,
            request: 1,
        },
        Request::Begin {
            kind: InputKind::FaceBasis,
            total: MAX_BLOB_BYTES,
        },
        &mut exchange,
    )
    .unwrap();
    assert!(decode_request(&exchange[..n]).is_ok());
    exchange[4..6].copy_from_slice(&5u16.to_le_bytes());
    assert!(decode_request(&exchange[..n]).is_err());
}
