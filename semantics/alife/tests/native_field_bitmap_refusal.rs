use conduit_alife::FieldBitmapRefusal;
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn field_bitmap_refusals_round_trip_through_their_exact_native_type() {
    for refusal in [
        FieldBitmapRefusal::InvalidField,
        FieldBitmapRefusal::InvalidBitmap,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            FieldBitmapRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
