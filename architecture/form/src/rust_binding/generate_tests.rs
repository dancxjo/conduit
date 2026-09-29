use super::*;
use alloc::vec::Vec;

fn checked_types() -> Vec<crate::CheckedNativeType> {
    let source = r#"type Note = U8 in 0..=127

type Position = {
    x: Distance
    y: Distance
}

type MusicEvent =
    note {
        velocity: U8 in 0..=127
        pitches: sequence Note <= 16
    }
    | rest
"#;
    crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap()
    .native_types
}

#[test]
fn generation_is_deterministic_and_keeps_rust_spelling_out_of_identity() {
    let types = checked_types();
    let plain = generate_rust_bindings(&types, &RustBindingOptions::default()).unwrap();
    assert_eq!(
        plain,
        generate_rust_bindings(&types, &RustBindingOptions::default()).unwrap()
    );
    let prefixed = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            type_prefix: "Fixture".into(),
        },
    )
    .unwrap();
    assert_eq!(plain.semantic_type_bytes, prefixed.semantic_type_bytes);
    assert!(plain.source.contains("pub struct Note(u8);"));
    assert!(plain.source.contains("pub struct Position {"));
    assert!(plain.source.contains("pub enum MusicEvent {"));
    assert!(plain.source.contains("pitches: BoundedSequence<Note, 16>"));
    assert!(plain.source.contains("Note(MusicEventNote)"));
    assert!(plain.source.contains("Rest,"));
    assert!(prefixed.source.contains("pub struct FixtureNote(u8);"));
}

#[test]
fn colliding_or_invalid_rust_spellings_refuse_instead_of_rebinding() {
    let mut types = checked_types();
    types[1].name = "note".into();
    assert!(matches!(
        generate_rust_bindings(&types, &RustBindingOptions::default()),
        Err(RustBindingGenerationError::DuplicateRustIdentifier(name)) if name == "Note"
    ));
    assert_eq!(
        generate_rust_bindings(
            &checked_types(),
            &RustBindingOptions {
                type_prefix: "9".into(),
            },
        ),
        Err(RustBindingGenerationError::InvalidRustIdentifier(
            "9".into()
        ))
    );
}
