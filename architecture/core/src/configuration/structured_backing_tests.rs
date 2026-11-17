//! Ownership accounting observes capacities without changing value semantics.
use super::*;
use alloc::{boxed::Box, string::String};

fn canonical() -> (KindId, Vec<u8>) {
    let value_type = crate::StructuredInfoType::leaf(crate::kind_id(crate::TEXT_INFO_ID)).unwrap();
    let profile = value_type.profile().unwrap().value_kind().clone();
    let value = crate::StructuredInfoValue::leaf(value_type, b"sample".to_vec()).unwrap();
    (profile, value.canonical_bytes().unwrap())
}

#[test]
fn owned_structured_backing_counts_profile_and_spare_payload_capacity() {
    let (profile, bytes) = canonical();
    let mut profile_text = String::with_capacity(profile.0.len() + 73);
    profile_text.push_str(profile.as_str());
    let mut padded = Vec::with_capacity(bytes.len() + 91);
    padded.extend_from_slice(&bytes);
    let expected = profile_text.capacity() + padded.capacity();
    let value = StructuredConfigurationValue::new(KindId(profile_text), padded).unwrap();
    assert_eq!(value.owned_backing_bytes(), Some(expected));
    assert_eq!(value.canonical_value(), bytes);
}

#[test]
fn linked_structured_bytes_have_no_owned_payload_charge() {
    let (profile, bytes) = canonical();
    // The fixture's immortal backing is outside the configuration owner, just
    // as executable-linked storage is. It is not refunded by dropping value.
    let linked = Box::leak(bytes.clone().into_boxed_slice());
    let expected = profile.0.capacity();
    let value = StructuredConfigurationValue::from_static(profile, linked).unwrap();
    let owned = StructuredConfigurationValue::new(value.profile().clone(), bytes).unwrap();
    assert_eq!(value.owned_backing_bytes(), Some(expected));
    assert_eq!(value, owned);
    assert_eq!(value.profile(), owned.profile());
    assert_eq!(value.canonical_value(), owned.canonical_value());
}
