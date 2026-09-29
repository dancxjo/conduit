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

#[test]
fn generated_bindings_compile_as_an_independent_rust_library() {
    use std::ffi::OsStr;
    use std::fs;
    use std::process::Command;
    use std::string::String;

    let generated =
        generate_rust_bindings(&checked_types(), &RustBindingOptions::default()).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let find_rlib = |crate_name: &str| {
        let prefix = format!("lib{crate_name}-");
        fs::read_dir(&dependencies)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension() == Some(OsStr::new("rlib"))
                    && path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| name.starts_with(&prefix))
            })
            .unwrap_or_else(|| panic!("missing {crate_name} rlib in {}", dependencies.display()))
    };
    let form = find_rlib("conduit_form");
    let core = find_rlib("conduit_core");
    let directory =
        std::env::temp_dir().join(format!("conduit-rust-bindings-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("bindings.rs");
    let exercise = r#"
#[cfg(test)]
mod generated_round_trip {
    use super::*;

    #[test]
    fn refinements_and_canonical_round_trip_are_exact() {
        let note = Note::new(60).unwrap();
        assert!(Note::new(200).is_err());
        let encoded = note.clone().encode().unwrap();
        assert_eq!(Note::decode(&encoded).unwrap(), note);

        let mut pitches = BoundedSequence::<Note, 16>::new();
        pitches.push(note).unwrap();
        let event = MusicEvent::note(pitches, 100).unwrap();
        let encoded = event.clone().encode().unwrap();
        assert_eq!(MusicEvent::decode(&encoded).unwrap(), event);

        let rest = MusicEvent::rest();
        let encoded = rest.clone().encode().unwrap();
        assert_eq!(MusicEvent::decode(&encoded).unwrap(), rest);
    }
}
"#;
    fs::write(&source, format!("{}{}", generated.source, exercise)).unwrap();
    let executable = directory.join("bindings-test");
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test"])
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_form={}", form.display()))
        .arg("--extern")
        .arg(format!("conduit_core={}", core.display()))
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated Rust failed to compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let execution = Command::new(&executable).output().unwrap();
    assert!(
        execution.status.success(),
        "generated Rust round trip failed:\n{}\n{}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}
