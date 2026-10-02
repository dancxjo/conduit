use conduit_human::{KeymapDisposition, KeymapRefusal, TextFragment};
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_copy<T: Copy>() {}

#[test]
fn keymap_output_is_copy_and_round_trips_every_payload_shape() {
    assert_copy::<TextFragment>();
    assert_copy::<KeymapDisposition>();

    let values = [
        KeymapDisposition::NoText,
        KeymapDisposition::Text(TextFragment::from_char('a')),
        KeymapDisposition::Text(TextFragment::from_char('€')),
        KeymapDisposition::Text(TextFragment::from_char('𐀀')),
        KeymapDisposition::Cancelled,
        KeymapDisposition::Refused(KeymapRefusal::UnknownComposeSequence),
    ];
    for value in values {
        let structured = value.into_structured().unwrap();
        assert_eq!(
            KeymapDisposition::from_structured(structured).unwrap(),
            value
        );
    }
}

#[test]
fn text_fragment_owns_the_exact_unicode_scalar_domain() {
    for value in [0, 55_295] {
        let fragment = TextFragment::basic(value).unwrap();
        assert_eq!(
            TextFragment::from_structured(fragment.into_structured().unwrap()).unwrap(),
            fragment
        );
    }
    for value in [57_344, 1_114_111] {
        let fragment = TextFragment::supplementary(value).unwrap();
        assert_eq!(
            TextFragment::from_structured(fragment.into_structured().unwrap()).unwrap(),
            fragment
        );
    }
    assert!(TextFragment::basic(55_296).is_err());
    assert!(TextFragment::supplementary(57_343).is_err());
    assert!(TextFragment::supplementary(1_114_112).is_err());

    let fragment = TextFragment::from_char('🦀');
    let mut bytes = [0; 4];
    assert_eq!(fragment.encode_utf8(&mut bytes), "🦀".as_bytes());
}

#[test]
fn keymap_output_has_no_handwritten_semantic_duplicate() {
    let source = include_str!("../src/input_keymap.rs");
    assert!(!source.contains("pub struct TextFragment"));
    assert!(!source.contains("pub enum KeymapDisposition"));
}
