use conduit_core::{
    encode_count, kind_id, StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
};

#[test]
fn structured_configuration_requires_matching_profile_and_canonical_value() {
    let value_type = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let value = StructuredInfoValue::leaf(value_type.clone(), encode_count(7).to_vec()).unwrap();
    let canonical = value.canonical_bytes().unwrap();
    let profile = value_type.profile().unwrap().value_kind().clone();

    assert!(StructuredConfigurationValue::new(profile, canonical.clone()).is_some());
    assert!(
        StructuredConfigurationValue::new(kind_id("structured-info/wrong@1"), canonical).is_none()
    );
    assert!(StructuredConfigurationValue::new(
        value_type.profile().unwrap().value_kind().clone(),
        vec![0xff],
    )
    .is_none());
}

fn accepted_before(profile: &conduit_core::KindId, bytes: &[u8]) -> bool {
    if profile.as_str().is_empty()
        || bytes.is_empty()
        || bytes.len() > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return false;
    }
    StructuredInfoValue::from_canonical_bytes(bytes)
        .ok()
        .and_then(|v| v.value_type().profile().ok())
        .is_some_and(|p| p.value_kind() == profile)
}

#[test]
fn borrowed_configuration_validation_preserves_owned_acceptance() {
    use conduit_core::StructuredFieldValue;
    let leaf = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let value = StructuredInfoValue::leaf(leaf.clone(), encode_count(7).to_vec()).unwrap();
    let record = StructuredInfoType::record(
        kind_id("test/configuration@1"),
        vec![
            conduit_core::StructuredFieldType::new("a", leaf.clone()).unwrap(),
            conduit_core::StructuredFieldType::new("z", leaf.clone()).unwrap(),
        ],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        record.clone(),
        vec![
            StructuredFieldValue::new("a", value.clone()).unwrap(),
            StructuredFieldValue::new("z", value).unwrap(),
        ],
    )
    .unwrap();
    let encoded = value.canonical_bytes().unwrap();
    let prefix = record.canonical_bytes().unwrap();
    let field_bytes = 4 + 1 + leaf.canonical_bytes().unwrap().len();
    let fields_at = prefix.len() - 2 * field_bytes;
    let mut unsorted = prefix[..fields_at].to_vec();
    unsorted.extend_from_slice(&prefix[fields_at + field_bytes..]);
    unsorted.extend_from_slice(&prefix[fields_at..fields_at + field_bytes]);
    unsorted.extend_from_slice(&encoded[prefix.len()..]);
    let profile = record.profile().unwrap().value_kind().clone();
    assert!(accepted_before(&profile, &unsorted));
    for canonical in [&encoded, &unsorted] {
        assert!(StructuredConfigurationValue::new(profile.clone(), canonical.clone()).is_some());
        for end in 0..canonical.len() {
            let bytes = &canonical[..end];
            assert_eq!(
                StructuredConfigurationValue::new(profile.clone(), bytes.to_vec()).is_some(),
                accepted_before(&profile, bytes),
                "truncated at {end}"
            );
        }
        for index in 0..canonical.len() {
            let mut bytes = canonical.clone();
            bytes[index] ^= 0xff;
            assert_eq!(
                StructuredConfigurationValue::new(profile.clone(), bytes.clone()).is_some(),
                accepted_before(&profile, &bytes),
                "altered at {index}"
            );
        }
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert_eq!(
            StructuredConfigurationValue::new(profile.clone(), trailing.clone()).is_some(),
            accepted_before(&profile, &trailing)
        );
    }
}

#[test]
fn configuration_nominal_values_keep_the_owned_aggregate_node_boundary() {
    let leaf = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let nominal = StructuredInfoType::nominal(kind_id("test/nominal-bool@1"), leaf).unwrap();
    let inner = StructuredInfoType::bounded_sequence(nominal, 1, 1024).unwrap();
    let outer = StructuredInfoType::bounded_sequence(inner, 16, 16).unwrap();
    let profile = outer.profile().unwrap().value_kind().clone();
    for (last, expected) in [(1006usize, true), (1007, true), (1008, false)] {
        let mut encoded = outer.canonical_bytes().unwrap();
        encoded.push(1);
        encoded.extend_from_slice(&16u32.to_le_bytes());
        for group in 0..16 {
            let count = if group == 15 { last } else { 1024 };
            encoded.push(1);
            encoded.extend_from_slice(&(count as u32).to_le_bytes());
            for _ in 0..count {
                encoded.push(0);
                encoded.extend_from_slice(&1u32.to_le_bytes());
                encoded.push(1);
            }
        }
        assert_eq!(accepted_before(&profile, &encoded), expected);
        assert_eq!(
            StructuredConfigurationValue::new(profile.clone(), encoded).is_some(),
            expected
        );
    }
}
