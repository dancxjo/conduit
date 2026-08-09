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
fn existing_background_and_accent_palette_roles_are_preserved() {
    let mut buffer = vec![BACKGROUND; 320 * 40];
    draw_document(&mut buffer, 320, 40, &["HOSTS".to_owned()]);
    let rendered_colors = buffer
        .iter()
        .copied()
        .filter(|pixel| *pixel != BACKGROUND)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(rendered_colors.len(), 1);
    assert!(buffer.contains(&BACKGROUND));
}

#[test]
fn tiny_or_inconsistent_buffers_are_clipped_without_panicking() {
    let mut one_pixel = [BACKGROUND];
    draw_document(&mut one_pixel, usize::MAX, usize::MAX, &["中".to_owned()]);
    assert_eq!(one_pixel, [BACKGROUND]);
}
