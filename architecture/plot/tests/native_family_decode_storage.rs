//! A law-free nominal family must admit Type-constructor temporary storage.
use conduit_core::StructuredInfoType;
use conduit_plot::rust_binding::*;
static NOMINAL: NativeFamilyTypeDescriptor = NativeFamilyTypeDescriptor {
    type_bytes: &[
        5, 1, 0, 0, 0, b'n', 0, 9, 0, 0, 0, b'v', b'a', b'l', b'u', b'e', b'/', b'u', b'6', b'4',
    ],
    laws: &[],
    contracts: &[],
    children: &[],
    external_edges: &[],
    conversion_profile: NativeFamilyConversionProfile::Nominal,
    maximum_inline_bytes: 8,
};
#[test]
fn law_free_nominal_preparation_refuses_one_under_complete_decode_bound() {
    let limits = PreparedNativeFamilyLimits {
        maximum_types: 1,
        maximum_laws_per_type: 0,
        maximum_input_bytes: 0,
        maximum_retained_bytes: 1024,
        maximum_preparation_peak_bytes: 1024,
        maximum_conversion_requested_bytes: 1024 * 1024,
    };
    let family = PreparedNativeFamily::prepare(&[&NOMINAL], limits).unwrap();
    let receipt = family.storage_receipt();
    let decoded = StructuredInfoType::from_canonical_bytes(NOMINAL.type_bytes).unwrap();
    assert_eq!(
        StructuredInfoType::canonical_decode_storage_bound(NOMINAL.type_bytes).unwrap(),
        decoded.owned_heap_bytes() + NOMINAL.type_bytes.len(),
    );
    assert_eq!(
        receipt.preparation_peak_heap_bytes_bound,
        receipt.retained_heap_bytes_bound + decoded.owned_heap_bytes() + NOMINAL.type_bytes.len()
    );
    PreparedNativeFamily::prepare(
        &[&NOMINAL],
        PreparedNativeFamilyLimits {
            maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound,
            ..limits
        },
    )
    .unwrap();
    assert!(matches!(
        PreparedNativeFamily::prepare(
            &[&NOMINAL],
            PreparedNativeFamilyLimits {
                maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1,
                ..limits
            }
        ),
        Err(PreparedNativeFamilyRefusal::Capacity)
    ));
    // The old retained-only decode charge would have admitted this ceiling.
    assert!(matches!(
        PreparedNativeFamily::prepare(
            &[&NOMINAL],
            PreparedNativeFamilyLimits {
                maximum_preparation_peak_bytes: receipt.retained_heap_bytes_bound
                    + decoded.owned_heap_bytes(),
                ..limits
            }
        ),
        Err(PreparedNativeFamilyRefusal::Capacity)
    ));
}
