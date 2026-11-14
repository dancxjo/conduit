//! Ordinary composer oracle for the authored frame schema.
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as C,
    StructuredInfoType as T, StructuredInfoTypeShape as S,
};
pub fn encode(input: &T, address: u8, ram: [u8; 16]) -> Vec<u8> {
    let S::Record { fields, .. } = input.shape() else {
        panic!("frame record")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name(), "address");
    assert_eq!(fields[1].name(), "ram");
    assert_eq!(fields[0].value_type(), &T::leaf("value/u8".into()).unwrap());
    let S::Collection {
        element,
        length: 16,
    } = fields[1].value_type().shape()
    else {
        panic!("sixteen RAM octets")
    };
    assert_eq!(element, &T::leaf("value/u8".into()).unwrap());
    let mut address_value = C::new(fields[0].value_type(), 64).unwrap();
    address_value.leaf(&[address]).unwrap();
    let mut ram_value = C::new(fields[1].value_type(), 1024).unwrap();
    ram_value
        .leaf_collection(ram.iter().map(core::slice::from_ref))
        .unwrap();
    let mut frame = C::new(input, 2048).unwrap();
    frame
        .record(&[
            validate(address_value.encoded()).unwrap(),
            validate(ram_value.encoded()).unwrap(),
        ])
        .unwrap();
    frame.encoded().to_vec()
}
