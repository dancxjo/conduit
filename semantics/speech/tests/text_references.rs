#![cfg(feature = "semantic-bindings")]
use conduit_speech::{semantic::*, text_admission::*};

fn material(text: &str) -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("text".into()).unwrap(),
        LanguageId::new("es".into()).unwrap(),
        LanguageTextRevisionId::new("revision".into()).unwrap(),
        text.into(),
    )
    .unwrap()
}
fn reference(start: u32, end: u32, id: &str, revision: &str, language: &str) -> LanguageSegmentRef {
    LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        LanguageId::new(language.into()).unwrap(),
        LanguageTextRange::new(end, start).unwrap(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        LanguageTextId::new(id.into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn resolves_unicode_scalars_and_preserves_borrowed_material() {
    let snapshot = material("¡Qué bonita! 👋e\u{301}");
    for (start, end, expected) in [
        (1, 4, "Qué"),
        (5, 11, "bonita"),
        (13, 14, "👋"),
        (14, 16, "e\u{301}"),
        (16, 16, ""),
    ] {
        let reference = reference(start, end, "text", "revision", "es");
        let resolved = resolve_text(&reference, &snapshot).unwrap();
        assert_eq!(resolved.text(), expected);
        assert!(core::ptr::eq(resolved.material(), &snapshot));
        assert!(core::ptr::eq(resolved.reference(), &reference));
        assert_eq!(*resolved.checked().scalar_count(), 16);
    }
}
#[test]
fn identical_spelling_cannot_resolve_foreign_basis_or_out_of_range() {
    let snapshot = material("hola");
    for (id, revision, language, end) in [
        ("foreign", "revision", "es", 4),
        ("text", "stale", "es", 4),
        ("text", "revision", "en", 4),
        ("text", "revision", "es", 5),
    ] {
        let reference = reference(0, end, id, revision, language);
        assert!(matches!(
            resolve_text(&reference, &snapshot),
            Err(TextReferenceRefusal::Native(_))
        ));
    }
    assert!(LanguageTextRange::new(0, 1).is_err());
}
#[test]
fn empty_material_and_maximum_scalar_extent_are_exact() {
    for text in [String::new(), "a".repeat(4096)] {
        let snapshot = material(&text);
        let count = text.chars().count() as u32;
        let reference = reference(0, count, "text", "revision", "es");
        assert_eq!(resolve_text(&reference, &snapshot).unwrap().text(), text);
    }
    let snapshot = material("");
    let phone = LanguageSegmentRef::phone(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("es".into()).unwrap(),
        0,
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        resolve_text(&phone, &snapshot),
        Err(TextReferenceRefusal::ReferenceKind)
    ));
}
