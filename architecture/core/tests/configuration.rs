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

#[test]
fn linked_structured_bytes_validate_without_copying_and_preserve_owned_wire() {
    use serde::Serialize;
    #[derive(Serialize)]
    struct Original {
        profile: conduit_core::KindId,
        canonical_value: Vec<u8>,
    }
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), encode_count(7).to_vec()).unwrap();
    // Hosted test owns this one permanent buffer. Make's compiled-expression
    // conformance test separately exercises literal bytes linked in read-only data.
    let bytes: &'static [u8] = Box::leak(value.canonical_bytes().unwrap().into_boxed_slice());
    let profile = ty.profile().unwrap().value_kind().clone();
    let linked = StructuredConfigurationValue::from_static(profile.clone(), bytes).unwrap();
    let owned = StructuredConfigurationValue::new(profile.clone(), bytes.to_vec()).unwrap();
    assert_eq!(linked.canonical_value().as_ptr(), bytes.as_ptr());
    assert_eq!(linked.clone().canonical_value().as_ptr(), bytes.as_ptr());
    assert_eq!(linked, owned);
    let original = Original {
        profile: profile.clone(),
        canonical_value: bytes.to_vec(),
    };
    let json = serde_json::to_vec(&linked).unwrap();
    assert_eq!(json, serde_json::to_vec(&original).unwrap());
    let binary = postcard::to_allocvec(&linked).unwrap();
    assert_eq!(binary, postcard::to_allocvec(&original).unwrap());
    assert_eq!(
        serde_json::from_slice::<StructuredConfigurationValue>(&json).unwrap(),
        linked
    );
    assert_eq!(
        postcard::from_bytes::<StructuredConfigurationValue>(&binary).unwrap(),
        linked
    );
    assert!(
        StructuredConfigurationValue::from_static(kind_id("structured-info/wrong@1"), bytes)
            .is_none()
    );
    for invalid in [&[][..], &[0xff][..], &bytes[..bytes.len() - 1]] {
        assert!(StructuredConfigurationValue::from_static(profile.clone(), invalid).is_none());
    }
}

#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn canonical_shape_fixtures() -> Vec<StructuredInfoValue> {
    use conduit_core::{StructuredFieldType, StructuredFieldValue, StructuredVariantCase};
    let leaf_type = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let leaf = StructuredInfoValue::leaf(leaf_type.clone(), encode_count(7).to_vec()).unwrap();
    let collection_type = StructuredInfoType::collection(leaf_type.clone(), Some(2)).unwrap();
    let collection =
        StructuredInfoValue::collection(collection_type, vec![leaf.clone(), leaf.clone()]).unwrap();
    let sequence_type = StructuredInfoType::bounded_sequence(leaf_type.clone(), 1, 3).unwrap();
    let sequence = StructuredInfoValue::sequence(sequence_type, vec![leaf.clone()]).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("test/fast-configuration@1"),
        vec![
            StructuredFieldType::new("a", leaf_type.clone()).unwrap(),
            StructuredFieldType::new("z", collection.value_type().clone()).unwrap(),
        ],
    )
    .unwrap();
    let record = StructuredInfoValue::record(
        record_type,
        vec![
            StructuredFieldValue::new("a", leaf.clone()).unwrap(),
            StructuredFieldValue::new("z", collection.clone()).unwrap(),
        ],
    )
    .unwrap();
    let variant_type = StructuredInfoType::variant(
        kind_id("test/fast-choice@1"),
        vec![
            StructuredVariantCase::new("count", leaf_type).unwrap(),
            StructuredVariantCase::new("record", record.value_type().clone()).unwrap(),
        ],
    )
    .unwrap();
    let variant = StructuredInfoValue::variant(variant_type, "record", record.clone()).unwrap();
    let nominal_type =
        StructuredInfoType::nominal(kind_id("test/fast-nominal@1"), variant.value_type().clone())
            .unwrap();
    let nominal = StructuredInfoValue::nominal(nominal_type, variant.clone()).unwrap();
    vec![leaf, collection, sequence, record, variant, nominal]
}

fn require_storage_acceptance(profile: &conduit_core::KindId, bytes: &[u8]) {
    let expected = accepted_before(profile, bytes);
    assert_eq!(
        StructuredConfigurationValue::new(profile.clone(), bytes.to_vec()).is_some(),
        expected
    );
    // Hosted fixture owns this immutable input before constructor observation.
    let linked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
    assert_eq!(
        StructuredConfigurationValue::from_static(profile.clone(), linked).is_some(),
        expected
    );
}

#[test]
fn canonical_configuration_all_shapes_preserve_owned_acceptance_and_type_identity() {
    for value in canonical_shape_fixtures() {
        let bytes = value.canonical_bytes().unwrap();
        let prefix = value.value_type().canonical_bytes().unwrap();
        let decoded_type = StructuredInfoType::from_canonical_bytes(&prefix).unwrap();
        let borrowed = conduit_core::validate_canonical_structured_value(&bytes).unwrap();
        assert_eq!(borrowed.type_bytes(), prefix);
        assert_eq!(
            borrowed.type_semantic_digest(),
            decoded_type.semantic_digest().unwrap()
        );
        let profile = decoded_type.profile().unwrap().value_kind().clone();
        require_storage_acceptance(&profile, &bytes);
        require_canonical_wire_parity(&profile, &bytes);
        for end in 0..bytes.len() {
            require_storage_acceptance(&profile, &bytes[..end]);
        }
        for index in 0..bytes.len() {
            let mut altered = bytes.clone();
            altered[index] ^= 0xff;
            require_storage_acceptance(&profile, &altered);
        }
        let mut trailing = bytes;
        trailing.push(0);
        require_storage_acceptance(&profile, &trailing);
    }
}

#[test]
fn canonical_configuration_profile_requires_every_exact_digest_nibble_and_framing_byte() {
    for value in canonical_shape_fixtures() {
        let bytes = value.canonical_bytes().unwrap();
        let profile = value.value_type().profile().unwrap().value_kind().clone();
        let text = profile.as_str();
        let prefix = "structured-info/profile-".len();
        for index in prefix..prefix + 64 {
            let mut changed = text.as_bytes().to_vec();
            changed[index] = if changed[index] == b'0' { b'1' } else { b'0' };
            require_storage_acceptance(&kind_id(core::str::from_utf8(&changed).unwrap()), &bytes);
        }
        for malformed in [
            "".to_string(),
            text[..prefix].to_string(),
            text[..text.len() - 1].to_string(),
            format!("{text}x"),
            format!("x{text}"),
            text.replacen("profile-", "profiles-", 1),
            text.replace("@1", "@2"),
            text.to_uppercase(),
            format!(
                "{}{}@1",
                &text[..prefix],
                text[prefix..prefix + 64].to_uppercase()
            ),
            format!("{}g{}", &text[..prefix], &text[prefix + 1..]),
            format!("{}@1", &text[..prefix + 63]),
            format!("{}0@1", &text[..prefix + 64]),
        ] {
            assert!(!accepted_before(&kind_id(&malformed), &bytes));
            require_storage_acceptance(&kind_id(&malformed), &bytes);
        }
    }
}

#[test]
fn canonical_configuration_checks_primitive_and_quantity_role_laws() {
    let bool_type = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    let mut invalid = bool_type.canonical_bytes().unwrap();
    invalid.push(0);
    invalid.extend_from_slice(&1u32.to_le_bytes());
    invalid.push(2);
    let profile = bool_type.profile().unwrap().value_kind().clone();
    assert!(!accepted_before(&profile, &invalid));
    require_storage_acceptance(&profile, &invalid);
    // An intrinsic role leaf must reject a payload with another role/family;
    // use a known valid primitive payload rather than malformed framing.
    let role = conduit_core::quantity_role_info_id(
        conduit_core::builtin_temperature_family(),
        conduit_core::QuantityRole::Point,
    )
    .unwrap();
    let role_type = StructuredInfoType::leaf(kind_id(&role)).unwrap();
    let role_profile = role_type.profile().unwrap().value_kind().clone();
    for (literal, expected) in [
        ("TemperatureDelta(1, °F)", false),
        ("1m", false),
        ("1°F", true),
    ] {
        let quantity = conduit_core::Quantity::parse_plot_literal(literal)
            .unwrap()
            .encode();
        let mut encoded = role_type.canonical_bytes().unwrap();
        encoded.push(0);
        encoded.extend_from_slice(&(quantity.len() as u32).to_le_bytes());
        encoded.extend_from_slice(&quantity);
        assert_eq!(accepted_before(&role_profile, &encoded), expected);
        require_storage_acceptance(&role_profile, &encoded);
    }
}

// Reconstruct only the TYPE and profile on known-valid canonical input. This is
// the allocation work the actual old configuration_profile performed; the full
// value-tree acceptance oracle above is not used as its allocation baseline.
fn old_type_profile_preparation(
    profile: conduit_core::KindId,
    bytes: &'static [u8],
    prefix: usize,
) -> Option<(conduit_core::KindId, &'static [u8])> {
    let value_type = StructuredInfoType::from_canonical_bytes(&bytes[..prefix]).ok()?;
    value_type.validate_canonical_node(&bytes[prefix..]).ok()?;
    let actual = value_type.profile().ok()?;
    (actual.value_kind() == &profile).then_some((profile, bytes))
}

#[test]
fn canonical_linked_configuration_skips_old_type_and_profile_ownership() {
    for value in canonical_shape_fixtures() {
        let prefix = value.value_type().canonical_bytes().unwrap().len();
        let profile = value.value_type().profile().unwrap().value_kind().clone();
        let old_profile = profile.clone();
        let bytes: &'static [u8] = Box::leak(value.canonical_bytes().unwrap().into_boxed_slice());
        let (old, old_cost) =
            allocation_probe::observe(|| old_type_profile_preparation(old_profile, bytes, prefix));
        let (new, new_cost) =
            allocation_probe::observe(|| StructuredConfigurationValue::from_static(profile, bytes));
        assert!(old.is_some());
        let new = new.unwrap();
        assert_eq!(new.canonical_value().as_ptr(), bytes.as_ptr());
        assert!(old_cost.allocations > 0, "{old_cost:?}");
        assert_eq!(new_cost.allocations, 0, "{new_cost:?}");
        assert_eq!(new_cost.peak_bytes, 0, "{new_cost:?}");
    }
}

fn require_canonical_wire_parity(profile: &conduit_core::KindId, bytes: &[u8]) {
    #[derive(serde::Serialize)]
    struct Original<'a> {
        profile: &'a conduit_core::KindId,
        canonical_value: &'a [u8],
    }
    let original = Original {
        profile,
        canonical_value: bytes,
    };
    let owned = StructuredConfigurationValue::new(profile.clone(), bytes.to_vec()).unwrap();
    let linked = StructuredConfigurationValue::from_static(
        profile.clone(),
        Box::leak(bytes.to_vec().into_boxed_slice()),
    )
    .unwrap();
    assert_eq!(owned, linked);
    let json = serde_json::to_vec(&original).unwrap();
    let binary = postcard::to_allocvec(&original).unwrap();
    for value in [&owned, &linked] {
        assert_eq!(serde_json::to_vec(value).unwrap(), json);
        assert_eq!(postcard::to_allocvec(value).unwrap(), binary);
    }
    assert_eq!(
        serde_json::from_slice::<StructuredConfigurationValue>(&json).unwrap(),
        owned
    );
    assert_eq!(
        postcard::from_bytes::<StructuredConfigurationValue>(&binary).unwrap(),
        owned
    );
}
