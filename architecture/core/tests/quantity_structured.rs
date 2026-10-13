use conduit_core::{
    MAX_QUANTITY_ENCODED_BYTES, MAXIMUM_STRUCTURED_TRANSPORT_BYTES, QUANTITY_INFO_ID, Quantity,
    StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape, Unit, decode_structured_transport, encode_structured_transport,
    kind_id,
};

#[test]
fn every_quantity_unit_has_one_round_tripping_canonical_tag() {
    let units = [
        Unit::Nanosecond,
        Unit::Microsecond,
        Unit::Millisecond,
        Unit::Second,
        Unit::Millihertz,
        Unit::Hertz,
        Unit::Microvolt,
        Unit::Millivolt,
        Unit::Volt,
        Unit::Micrometer,
        Unit::Millimeter,
        Unit::Centimeter,
        Unit::Meter,
        Unit::Microdegree,
        Unit::Millidegree,
        Unit::Degree,
        Unit::Millionth,
        Unit::Permille,
        Unit::Percent,
        Unit::One,
        Unit::Byte,
        Unit::Kibibyte,
        Unit::Mebibyte,
    ];
    for (index, unit) in units.into_iter().enumerate() {
        let quantity = Quantity::new(index as i64 - 11, unit);
        assert_eq!(Quantity::decode(&quantity.encode()), Ok(quantity.clone()));
        assert_ne!(quantity.semantic_digest(), [0; 32]);
    }
    assert_eq!(QUANTITY_INFO_ID, "value/quantity@2");
}

#[test]
fn malformed_quantity_encoding_refuses_before_structured_use() {
    assert!(Quantity::decode(&[0; 9]).is_err());
    let mut invalid = Quantity::new(1, Unit::Second).encode();
    invalid[0] = 255;
    assert!(Quantity::decode(&invalid).is_err());
    assert!(invalid.len() <= MAX_QUANTITY_ENCODED_BYTES);
}

#[test]
fn structured_record_transport_preserves_quantity_value_and_unit() {
    let quantity_type = StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("measurement/specimen@1"),
        vec![StructuredFieldType::new("measurement", quantity_type.clone()).unwrap()],
    )
    .unwrap();
    let quantity = Quantity::new(3_200, Unit::Millivolt);
    let record = StructuredInfoValue::record(
        record_type.clone(),
        vec![
            StructuredFieldValue::new(
                "measurement",
                StructuredInfoValue::leaf(quantity_type, quantity.encode().to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let maximum = MAXIMUM_STRUCTURED_TRANSPORT_BYTES as u32;
    let encoded = encode_structured_transport(&record, maximum).unwrap();
    let decoded = decode_structured_transport(&record_type, &encoded, maximum).unwrap();
    assert_eq!(decoded, record);
    let StructuredInfoValueShape::Record(fields) = decoded.shape() else {
        panic!("decoded quantity specimen must remain a record");
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
        panic!("measurement must remain a quantity leaf");
    };
    assert_eq!(Quantity::decode(bytes), Ok(quantity.clone()));
}
