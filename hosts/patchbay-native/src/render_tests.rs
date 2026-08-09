use crate::render::{draw_document, BACKGROUND};

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
fn existing_background_and_accent_palette_values_are_preserved() {
    let mut buffer = vec![BACKGROUND; 320 * 40];
    draw_document(&mut buffer, 320, 40, &["HOSTS".to_owned()]);
    assert_eq!(BACKGROUND, 0x0015_1820);
    assert!(buffer.contains(&0x006d_d7c7));
    assert!(buffer
        .iter()
        .all(|pixel| *pixel == BACKGROUND || *pixel == 0x006d_d7c7));
}

#[test]
fn tiny_or_inconsistent_buffers_are_clipped_without_panicking() {
    let mut one_pixel = [BACKGROUND];
    draw_document(&mut one_pixel, usize::MAX, usize::MAX, &["中".to_owned()]);
    assert_eq!(one_pixel, [BACKGROUND]);
}
