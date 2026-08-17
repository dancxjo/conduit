use conduit_core::{
    decode_structured_transport, encode_structured_transport, kind_id, Quantity,
    QuantityDecodeRefusal, QuantityUnit, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
    MAXIMUM_STRUCTURED_TRANSPORT_BYTES, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID,
};

#[test]
fn every_quantity_unit_has_one_round_tripping_canonical_tag() {
    let units = [
        QuantityUnit::Nanosecond,
        QuantityUnit::Microsecond,
        QuantityUnit::Millisecond,
        QuantityUnit::Second,
        QuantityUnit::Millihertz,
        QuantityUnit::Hertz,
        QuantityUnit::Microvolt,
        QuantityUnit::Millivolt,
        QuantityUnit::Volt,
        QuantityUnit::Micrometer,
        QuantityUnit::Millimeter,
        QuantityUnit::Centimeter,
        QuantityUnit::Meter,
        QuantityUnit::Microdegree,
        QuantityUnit::Millidegree,
        QuantityUnit::Degree,
        QuantityUnit::Millionth,
        QuantityUnit::Permille,
        QuantityUnit::Percent,
        QuantityUnit::One,
        QuantityUnit::Byte,
        QuantityUnit::Kibibyte,
        QuantityUnit::Mebibyte,
    ];
    for (index, unit) in units.into_iter().enumerate() {
        let quantity = Quantity::new(index as i64 - 11, unit);
        assert_eq!(Quantity::decode(&quantity.encode()), Ok(quantity));
        assert_ne!(quantity.semantic_digest(), [0; 32]);
    }
    assert_eq!(QUANTITY_INFO_ID, "value/quantity@1");
}

#[test]
fn malformed_quantity_encoding_refuses_before_structured_use() {
    assert_eq!(
        Quantity::decode(&[0; QUANTITY_ENCODED_LEN - 1]),
        Err(QuantityDecodeRefusal::WrongLength {
            expected: QUANTITY_ENCODED_LEN,
            actual: QUANTITY_ENCODED_LEN - 1,
        })
    );
    let mut invalid = Quantity::new(1, QuantityUnit::Second).encode();
    invalid[0] = 255;
    assert_eq!(
        Quantity::decode(&invalid),
        Err(QuantityDecodeRefusal::UnknownUnitTag(255))
    );
}

#[test]
fn structured_record_transport_preserves_quantity_value_and_unit() {
    let quantity_type = StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("measurement/specimen@1"),
        vec![StructuredFieldType::new("measurement", quantity_type.clone()).unwrap()],
    )
    .unwrap();
    let quantity = Quantity::new(3_200, QuantityUnit::Millivolt);
    let record = StructuredInfoValue::record(
        record_type.clone(),
        vec![StructuredFieldValue::new(
            "measurement",
            StructuredInfoValue::leaf(quantity_type, quantity.encode().to_vec()).unwrap(),
        )
        .unwrap()],
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
    assert_eq!(Quantity::decode(bytes), Ok(quantity));
}
