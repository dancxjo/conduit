use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::ThemeColor;

#[test]
fn theme_color_has_exact_native_rgb_and_legacy_const_access() {
    const COLOR: ThemeColor = ThemeColor::from_rgb(0x12, 0x34, 0x56);
    assert_eq!(COLOR.red(), 0x12);
    assert_eq!(COLOR.green(), 0x34);
    assert_eq!(COLOR.blue(), 0x56);
    assert_eq!(COLOR.packed_rgb(), 0x12_34_56);

    let structured = COLOR.into_structured().unwrap();
    assert_eq!(ThemeColor::from_structured(structured).unwrap(), COLOR);
}
