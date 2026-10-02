use conduit_language::{offset_basis_type, LinguisticOffsetBasis};
use conduit_plot::rust_binding::NativeRustBinding;

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
        let encoded = basis.encode().unwrap();
        assert_eq!(LinguisticOffsetBasis::decode(&encoded).unwrap(), basis);
    }
}
