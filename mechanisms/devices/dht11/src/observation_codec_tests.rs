extern crate std;
use super::*;
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as C,
};

fn case<'a>(ty: &'a Type, tag: &str) -> &'a Type {
    let Shape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
}
fn frame(contract: &ObservationContract, temperature: i128, humidity: i128) -> alloc::vec::Vec<u8> {
    let ty = case(&contract.input, "frame");
    let mut t = C::new(field(ty, "temperature").unwrap(), 1024).unwrap();
    let mut h = C::new(field(ty, "humidity").unwrap(), 1024).unwrap();
    t.leaf(&temperature.to_le_bytes()).unwrap();
    h.leaf(&humidity.to_le_bytes()).unwrap();
    let mut payload = C::new(ty, 1024).unwrap();
    payload
        .record(&[
            validate(h.encoded()).unwrap(),
            validate(t.encoded()).unwrap(),
        ])
        .unwrap();
    let mut input = C::new(&contract.input, 1024).unwrap();
    input
        .variant("frame", validate(payload.encoded()).unwrap())
        .unwrap()
        .to_vec()
}
fn unavailable(contract: &ObservationContract, tag: &str, bytes: &[u8]) -> alloc::vec::Vec<u8> {
    let ty = case(&contract.input, "unavailable");
    let mut leaf = C::new(case(ty, tag), 1024).unwrap();
    leaf.leaf(bytes).unwrap();
    let mut payload = C::new(ty, 1024).unwrap();
    payload
        .variant(tag, validate(leaf.encoded()).unwrap())
        .unwrap();
    let mut input = C::new(&contract.input, 1024).unwrap();
    input
        .variant("unavailable", validate(payload.encoded()).unwrap())
        .unwrap()
        .to_vec()
}
#[test]
fn original_byte_parity_preserves_quantities_failures_and_one_cache() {
    let contract = ObservationContract::prepare().unwrap();
    let mut new = PreparedObservationCodec::prepare(&contract).unwrap();
    let mut old = legacy::PreparedObservationCodec::prepare(&contract).unwrap();
    let old_backing = old.retained_backing_bytes();
    let backing = new.retained_backing_bytes();
    std::println!(
        "DHT observation codec backing: old={old_backing}, new={backing}, saved={}",
        old_backing - backing
    );
    assert_eq!(
        backing,
        storage_ceiling::bytes(&contract.output, MAXIMUM_BYTES as usize).unwrap()
    );
    assert!(old_backing > backing);
    let address = new.output.as_ptr();
    for (t, h) in [
        (-32768, 0),
        (-108, 530),
        (0, 0),
        (198, 590),
        (600, 950),
        (32767, 65535),
    ] {
        let input = frame(&contract, t, h);
        assert_eq!(new.convert(&input).unwrap(), old.convert(&input).unwrap());
        assert_eq!(new.output().len(), backing);
        let value = validate(new.output())
            .unwrap()
            .variant_payload("observation")
            .unwrap()
            .unwrap();
        let t_value = value.record_field("temperature").unwrap().unwrap();
        let h_value = value.record_field("humidity").unwrap().unwrap();
        // First-class quantities stay exact semantic types, independently of
        // the legacy encoding oracle and authored coordinate representation.
        assert_eq!(
            t_value.type_bytes(),
            Type::leaf(c::kind_id(c::TEMPERATURE_INFO_ID))
                .unwrap()
                .canonical_bytes()
                .unwrap()
        );
        assert_eq!(
            h_value.type_bytes(),
            Type::leaf(c::kind_id(c::RATIO_INFO_ID))
                .unwrap()
                .canonical_bytes()
                .unwrap()
        );
        assert!(c::Quantity::decode(&t_value.value_node()[5..])
            .unwrap()
            .compare(c::Quantity::from_decimal(t, -1, c::Unit::Celsius).unwrap())
            .unwrap()
            .is_eq());
        assert!(c::Quantity::decode(&h_value.value_node()[5..])
            .unwrap()
            .compare(c::Quantity::from_decimal(h, -1, c::Unit::Percent).unwrap())
            .unwrap()
            .is_eq());
        assert_eq!(new.output.as_ptr(), address);
        assert_eq!(new.retained_backing_bytes(), backing);
    }
    for (tag, bytes) in [
        ("short-frame", &[0][..]),
        ("short-frame", &[255][..]),
        ("malformed-length", &[255][..]),
        ("checksum", &[][..]),
        ("invalid-payload", &[][..]),
    ] {
        let input = unavailable(&contract, tag, bytes);
        assert_eq!(new.convert(&input).unwrap(), old.convert(&input).unwrap());
        assert!(validate(new.output())
            .unwrap()
            .variant_payload("unavailable")
            .unwrap()
            .unwrap()
            .variant_payload(tag)
            .unwrap()
            .is_some());
    }
    for (t, h) in [
        (i128::MIN, 0),
        (32768, 0),
        (0, -1),
        (0, 65536),
        (0, i128::MAX),
    ] {
        let input = frame(&contract, t, h);
        assert_eq!(new.convert(&input), old.convert(&input));
    }
    assert_eq!(new.convert(&[0]), old.convert(&[0]));
}
#[test]
fn foreign_observation_and_output_remain_refused() {
    let mut contract = ObservationContract::prepare().unwrap();
    contract.observation = contract.input.clone();
    assert!(matches!(
        PreparedObservationCodec::prepare(&contract),
        Err(R::WrongType)
    ));
    let mut contract = ObservationContract::prepare().unwrap();
    contract.output = contract.input.clone();
    assert!(matches!(
        PreparedObservationCodec::prepare(&contract),
        Err(R::WrongType)
    ));
}
