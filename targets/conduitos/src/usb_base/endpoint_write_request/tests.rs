use super::*;
use alloc::vec;
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
fn input(wire: &[u8]) -> alloc::vec::Vec<u8> {
    let contract = EndpointWriteContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("record");
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), wire.to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn empty_and_maximum_wire_are_real_valid_requests() {
    let contract = EndpointWriteContract::prepare().unwrap();
    let decoder = PreparedEndpointWriteRequestDecoder::new(&contract).unwrap();
    let zlp = input(&[]);
    assert!(decoder.decode(&zlp, 2048).unwrap().is_empty());
    let full = input(&[0xa5; 2048]);
    assert_eq!(decoder.decode(&full, 2048).unwrap(), &[0xa5; 2048]);
    assert_eq!(
        decoder.decode(&full, 1024),
        Err(EndpointWriteRequestRefusal::Length)
    );
    assert_eq!(
        decoder.decode(&zlp, 0),
        Err(EndpointWriteRequestRefusal::InvalidEnvelope)
    );
    assert_eq!(
        decoder.decode(&zlp, 2049),
        Err(EndpointWriteRequestRefusal::InvalidEnvelope)
    );
}
#[test]
fn malformed_or_foreign_canonical_request_never_supplies_native_wire() {
    let contract = EndpointWriteContract::prepare().unwrap();
    let decoder = PreparedEndpointWriteRequestDecoder::new(&contract).unwrap();
    assert!(decoder.decode(&[1, 2, 3], 2048).is_err());
    let read = crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = read.request_type().shape() else {
        panic!("record");
    };
    let foreign = StructuredInfoValue::record(
        read.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "length",
                StructuredInfoValue::leaf(
                    fields[0].value_type().clone(),
                    8u64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(decoder.decode(&foreign, 2048).is_err());
}
