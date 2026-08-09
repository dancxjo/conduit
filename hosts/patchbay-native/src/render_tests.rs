use crate::render::{draw_document, BACKGROUND};
use patchbay_model::PHOSPHOR_THEME;

#[test]
fn native_document_uses_every_required_phosphor_palette_role() {
    let mut buffer = vec![BACKGROUND; 400 * 80];
    draw_document(
        &mut buffer,
        400,
        80,
        &[
            "HOSTS 1".into(),
            "ordinary system fact".into(),
            "  secondary detail".into(),
            "> selected exact subject".into(),
        ],
    );
    for color in [
        PHOSPHOR_THEME.background,
        PHOSPHOR_THEME.structure_primary,
        PHOSPHOR_THEME.structure_secondary,
        PHOSPHOR_THEME.text_primary,
        PHOSPHOR_THEME.text_secondary,
        PHOSPHOR_THEME.emphasis,
        PHOSPHOR_THEME.focus,
    ] {
        assert!(
            buffer.contains(&color.packed_rgb()),
            "missing color {color:?}"
        );
    }
}

#[test]
fn clipping_remains_inside_the_actual_finite_pixel_slice() {
    let guard = 0x00AA_55AA;
    let mut storage = [guard; 18];
    draw_document(&mut storage[1..17], 4, 4, &["HOSTS".into()]);
    assert_eq!(storage[0], guard);
    assert_eq!(storage[17], guard);
}

#[test]
fn selection_keeps_its_text_marker_in_addition_to_gold_color() {
    let selected = "> gear exact-id [1..2] label";
    assert!(selected.starts_with("> "));
    let mut buffer = vec![BACKGROUND; 320 * 40];
    draw_document(&mut buffer, 320, 40, &[selected.into()]);
    assert!(buffer.contains(&PHOSPHOR_THEME.focus.packed_rgb()));
}
