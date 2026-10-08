use conduit_data::{TensorAxisRole, TensorElement};
use conduit_plot::rust_binding::{
    NativeRustBinding, PreparedNativeFamily, PreparedNativeFamilyLimits,
    PreparedNativeFamilyRefusal, PreparedNativeRustBinding,
};

fn limits() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 4,
        maximum_laws_per_type: 8,
        maximum_input_bytes: 4096,
        maximum_retained_bytes: 1024 * 1024,
        maximum_preparation_peak_bytes: 2 * 1024 * 1024,
        maximum_conversion_requested_bytes: 4 * 1024 * 1024,
    }
}

fn family() -> PreparedNativeFamily {
    PreparedNativeFamily::prepare(
        &[
            TensorElement::PREPARED_DESCRIPTOR,
            TensorAxisRole::PREPARED_DESCRIPTOR,
        ],
        limits(),
    )
    .unwrap()
}

fn parity<T: PreparedNativeRustBinding + Clone + PartialEq + core::fmt::Debug>(
    family: &mut PreparedNativeFamily,
    value: T,
) {
    let bytes = value.clone().encode().unwrap();
    assert_eq!(T::decode(&bytes).unwrap(), value);
    let prepared = family.decode::<T>(&bytes).unwrap();
    assert_eq!(prepared, value);
    assert_eq!(prepared.encode().unwrap(), bytes);
    assert_eq!(
        T::semantic_type().unwrap().canonical_bytes().unwrap(),
        T::PREPARED_DESCRIPTOR.type_bytes,
    );
}

#[test]
fn data_owned_prepared_variants_preserve_every_tensor_tag_and_payload() {
    let mut family = family();
    for value in [
        TensorElement::I8,
        TensorElement::U8,
        TensorElement::I16,
        TensorElement::I24,
        TensorElement::U16,
        TensorElement::I32,
        TensorElement::U32,
        TensorElement::I64,
        TensorElement::U64,
        TensorElement::F32,
        TensorElement::F64,
    ] {
        parity(&mut family, value);
    }
    for value in [
        TensorAxisRole::Batch,
        TensorAxisRole::Time,
        TensorAxisRole::Feature,
        TensorAxisRole::Sensor,
        TensorAxisRole::SpatialCoordinate,
        TensorAxisRole::Frequency,
        TensorAxisRole::Channel,
        TensorAxisRole::other("axis".into()).unwrap(),
        TensorAxisRole::other("λ".repeat(32)).unwrap(),
    ] {
        parity(&mut family, value);
    }
}

#[test]
fn prepared_tensor_variants_refuse_foreign_types_and_empty_other_identity() {
    let mut family = family();
    let element = TensorElement::F32.encode().unwrap();
    assert!(family.decode::<TensorAxisRole>(&element).is_err());

    let mut empty = TensorAxisRole::other("axis".into())
        .unwrap()
        .encode()
        .unwrap();
    // Replace only the terminal canonical text leaf, retaining the complete
    // original variant Type and all its contracts.
    assert!(empty.ends_with(&[0, 4, 0, 0, 0, b'a', b'x', b'i', b's']));
    empty.truncate(empty.len() - 9);
    empty.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert!(TensorAxisRole::decode(&empty).is_err());
    assert!(family.decode::<TensorAxisRole>(&empty).is_err());

    let mut element_only =
        PreparedNativeFamily::prepare(&[TensorElement::PREPARED_DESCRIPTOR], limits()).unwrap();
    assert!(element_only
        .decode::<TensorAxisRole>(&TensorAxisRole::Time.encode().unwrap(),)
        .is_err());
}

#[test]
fn tensor_descriptor_family_refuses_one_under_preparation_and_retained_limits() {
    let receipt = family().storage_receipt();
    for bounded in [
        PreparedNativeFamilyLimits {
            maximum_retained_bytes: receipt.retained_heap_bytes_bound - 1,
            ..limits()
        },
        PreparedNativeFamilyLimits {
            maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1,
            ..limits()
        },
    ] {
        assert!(matches!(
            PreparedNativeFamily::prepare(
                &[
                    TensorElement::PREPARED_DESCRIPTOR,
                    TensorAxisRole::PREPARED_DESCRIPTOR
                ],
                bounded,
            ),
            Err(PreparedNativeFamilyRefusal::Capacity)
        ));
    }
}
