use super::{font, profile, typography};
use conduit_presentation::GraphicsTextRole as Role;

#[test]
fn proportional_prose_and_monospace_code_have_distinct_metrics() {
    assert!(profile::advance('i', Role::Body) < profile::advance('W', Role::Body));
    assert_eq!(
        profile::advance('i', Role::Code),
        profile::advance('W', Role::Code)
    );
    assert!(profile::line_height(Role::Title) > profile::line_height(Role::Heading));
    assert!(profile::line_height(Role::Heading) > profile::line_height(Role::Body));
    assert_eq!(
        profile::advance('🦀', Role::Body),
        profile::advance('�', Role::Body)
    );
}

#[test]
fn admitted_unicode_and_secondary_face_have_real_coverage() {
    for character in "Crèche café Ω Ж 中 → ● ○".chars() {
        assert!(
            profile::coverage(character, Role::Body) != profile::GlyphCoverage::Replacement,
            "{character}"
        );
    }
    assert!(profile::glyph('中', Role::Body).is_none());
    assert!(!font::glyph('中').1);
    assert!(font::glyph('🦀').1);
    let ascii = profile::glyph('W', Role::Body).unwrap();
    assert!(
        ascii
            .coverage
            .iter()
            .any(|alpha| *alpha > 0 && *alpha < 255)
    );
}

#[test]
fn measuring_and_drawing_share_word_wrap_and_explicit_lines() {
    let width = "hello "
        .chars()
        .map(|c| profile::advance(c, Role::Body))
        .sum::<u32>() as u16;
    assert_eq!(
        typography::height("hello world", width, Role::Body),
        typography::height("hello\nworld", width, Role::Body)
    );
    assert_eq!(typography::height("x\n", 50, Role::Body), Ok(36));
    assert_eq!(
        typography::height("中", 8, Role::Body),
        Err(super::DisplayError::InvalidExtent)
    );
}

#[test]
fn text_tokens_meet_normal_text_contrast() {
    fn linear(value: u8) -> f64 {
        let s = f64::from(value) / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }
    for token in [
        profile::FOREGROUND,
        profile::ACCENT,
        profile::WARNING,
        profile::MUTED,
        profile::SUCCESS,
    ] {
        assert!((luminance(token) + 0.05) / (luminance(profile::BACKGROUND) + 0.05) >= 4.5);
    }
}

#[test]
fn the_entire_admitted_corpus_never_uses_missing_glyph_replacement() {
    let corpus = include_str!(
        "../../../../products/patchbay/native/assets/unifont/unifont-17.0.04-patchbay.hex"
    );
    for line in corpus.lines() {
        let cp = u32::from_str_radix(line.split_once(':').unwrap().0, 16).unwrap();
        let character = char::from_u32(cp).unwrap();
        for role in [Role::Body, Role::Heading, Role::Title, Role::Code] {
            assert_ne!(
                profile::coverage(character, role),
                profile::GlyphCoverage::Replacement,
                "{character}"
            );
        }
    }
    assert_eq!(
        profile::coverage('中', Role::Body),
        profile::GlyphCoverage::UnicodeFallback
    );
    assert_eq!(
        profile::coverage('🦀', Role::Body),
        profile::GlyphCoverage::Replacement
    );
}
