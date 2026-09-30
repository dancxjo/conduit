use conduit_form::rust_binding::NativeRustBinding;
use conduit_language::{offset_basis_type, LinguisticOffsetBasis};

#[test]
fn native_offset_basis_owns_catalog_identity_and_round_trip() {
    assert_eq!(
        offset_basis_type(),
        LinguisticOffsetBasis::semantic_type().unwrap()
    );
    for basis in [
        LinguisticOffsetBasis::unicode_scalar(),
        LinguisticOffsetBasis::utf8_byte(),
    ] {
        let encoded = basis.clone().encode().unwrap();
        assert_eq!(LinguisticOffsetBasis::decode(&encoded).unwrap(), basis);
    }
}
