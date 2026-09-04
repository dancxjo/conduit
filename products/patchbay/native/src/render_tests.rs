use crate::render::{draw_document, BACKGROUND};
use patchbay_model::CONDUIT_APPLICATION_THEME;

#[test]
fn document_renders_unicode_scripts_on_the_software_surface() {
    let mut buffer = vec![BACKGROUND; 640 * 80];
    draw_document(
        &mut buffer,
        640,
        80,
        &["ASCII café Ω Ж ─ → ■ 中".to_owned()],
    );
    assert!(buffer.iter().any(|pixel| *pixel != BACKGROUND));
}

#[test]
fn native_document_uses_every_required_phosphor_palette_role() {
    let mut buffer = vec![BACKGROUND; 400 * 100];
    draw_document(
        &mut buffer,
        400,
        100,
        &[
            "HOSTS 1".into(),
            "ordinary system fact".into(),
            "  secondary detail".into(),
            "> selected exact subject".into(),
        ],
    );
    for color in [
        CONDUIT_APPLICATION_THEME.background,
        CONDUIT_APPLICATION_THEME.structure_primary,
        CONDUIT_APPLICATION_THEME.structure_secondary,
        CONDUIT_APPLICATION_THEME.text_primary,
        CONDUIT_APPLICATION_THEME.text_secondary,
        CONDUIT_APPLICATION_THEME.emphasis,
        CONDUIT_APPLICATION_THEME.focus,
    ] {
        assert!(
            buffer.contains(&color.packed_rgb()),
            "missing color {color:?}"
        );
    }
}

#[test]
fn tiny_or_inconsistent_buffers_are_clipped_without_panicking() {
    let mut one_pixel = [BACKGROUND];
    draw_document(&mut one_pixel, usize::MAX, usize::MAX, &["中".to_owned()]);
    assert_eq!(one_pixel, [BACKGROUND]);

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
    assert!(buffer.contains(&CONDUIT_APPLICATION_THEME.focus.packed_rgb()));
}
