//! Ordinary composer oracle for the independently checked Source Frame type.
use conduit_core::{
    validate_canonical_structured_value as validate, InfoBool, PreparedStructuredComposer as C,
    StructuredInfoType as T, StructuredInfoTypeShape as S,
};
pub fn encode(input: &T, values: [u8; 8]) -> Vec<u8> {
    let field = |name| {
        let S::Record { fields, .. } = input.shape() else {
            panic!("Frame record")
        };
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
    };
    let registers = field("registers");
    let S::Collection { element, length: 6 } = registers.shape() else {
        panic!("six registers")
    };
    assert_eq!(element, &T::leaf("value/u8".into()).unwrap());
    assert_eq!(field("brightness"), &T::leaf("value/u8".into()).unwrap());
    assert_eq!(
        field("enabled"),
        &T::leaf(conduit_core::BOOL_INFO_ID.into()).unwrap()
    );
    let mut octets = core::array::from_fn::<_, 6, _>(|_| C::new(element, 64).unwrap());
    for (leaf, byte) in octets.iter_mut().zip(values) {
        leaf.leaf(&[byte]).unwrap();
    }
    let mut wire = C::new(registers, 512).unwrap();
    wire.collection(&core::array::from_fn::<_, 6, _>(|n| {
        validate(octets[n].encoded()).unwrap()
    }))
    .unwrap();
    let mut brightness = C::new(field("brightness"), 64).unwrap();
    brightness.leaf(&[values[6]]).unwrap();
    let mut enabled = C::new(field("enabled"), 64).unwrap();
    enabled
        .leaf(&InfoBool::new(values[7] != 0).encode())
        .unwrap();
    let mut frame = C::new(input, 1024).unwrap();
    frame
        .record(&[
            validate(brightness.encoded()).unwrap(),
            validate(enabled.encoded()).unwrap(),
            validate(wire.encoded()).unwrap(),
        ])
        .unwrap();
    frame.encoded().to_vec()
}
