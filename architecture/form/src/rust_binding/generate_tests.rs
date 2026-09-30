use super::*;
use alloc::vec::Vec;

fn checked_types() -> Vec<crate::CheckedNativeType> {
    let source = r#"type Note = U8 in 0..=127

type Position = {
    x: Distance
    y: Distance
}

type Observation = {
    note: Note?
    evidence: &Text
}

type MusicEvent =
    note {
        velocity: U8 in 0..=127
        pitches: sequence Note <= 16
    }
    | rest

type Direction =
    north
    | south

type Toggle = {
    enabled: Boolean
}

type Input =
    toggle {
        enabled: Boolean
    }
    | absent
"#;
    crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap()
    .native_types
}

#[test]
fn checked_representation_generates_the_only_rust_discriminant_table() {
    let source =
        "type Outcome =\n    ready\n    | refused\n\nrepresentation test/outcome = Outcome as u8\n";
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings_with_representations(
        &checked.native_types,
        &checked.representations,
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated
        .source
        .contains("pub struct OutcomeRepresentation;"));
    assert!(generated.source.contains("Outcome::Ready => 0"));
    assert!(generated.source.contains("1 => Ok(Outcome::Refused)"));
    assert!(generated.source.contains("MAXIMUM_DECODE_STEPS: usize = 3"));
    assert!(generated
        .source
        .contains("NativeRepresentationRefusal::InvalidTag"));
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
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    let serde_unit_variants = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            derive_serde_for_unit_variants: true,
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert_eq!(plain.semantic_type_bytes, prefixed.semantic_type_bytes);
    assert_eq!(
        plain.semantic_type_bytes,
        serde_unit_variants.semantic_type_bytes
    );
    assert!(plain.source.contains("pub struct Note(u8);"));
    assert!(plain.source.contains("pub const MAXIMUM_BYTES: usize = 1;"));
    assert!(plain.source.contains("pub struct Position {"));
    assert!(plain.source.contains(
        "pub fn new(x: conduit_core::Quantity, y: conduit_core::Quantity) -> Result<Self, NativeBindingRefusal> {\n        Ok(Self { x, y, })"
    ));
    assert!(plain.source.contains("pub struct Observation {"));
    assert!(plain.source.contains("pub enum MusicEvent {"));
    assert!(plain.source.contains("enabled: bool"));
    assert!(plain.source.contains("pitches: BoundedSequence<Note, 16>"));
    assert!(plain.source.contains("Note(MusicEventNote)"));
    assert!(plain.source.contains("Rest,"));
    assert!(plain.source.contains(
        "pub fn toggle(enabled: bool) -> Result<Self, NativeBindingRefusal> { Ok(Self::Toggle(InputToggle { enabled, })) }"
    ));
    assert!(plain.source.contains(
        "pub fn note(pitches: BoundedSequence<Note, 16>, velocity: u8) -> Result<Self, NativeBindingRefusal> { let candidate = Self::Note"
    ));
    assert!(plain.source.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum Direction"
    ));
    assert!(prefixed.source.contains("pub struct FixtureNote(u8);"));
    assert!(serde_unit_variants
        .source
        .contains("Hash, serde::Serialize, serde::Deserialize)]\npub enum Direction"));
    assert!(!serde_unit_variants
        .source
        .contains("serde::Serialize)]\npub struct Position"));
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
                ..RustBindingOptions::default()
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
        fn requires_unit_variant_traits<T: Copy + Ord + core::hash::Hash>() {}
        requires_unit_variant_traits::<Direction>();
        assert!(Direction::North < Direction::South);

        let note = Note::new(60).unwrap();
        assert!(Note::new(200).is_err());
        let encoded = note.clone().encode().unwrap();
        assert_eq!(Note::decode(&encoded).unwrap(), note);

        let mut pitches = BoundedSequence::<Note, 16>::new();
        pitches.push(note).unwrap();
        let event = MusicEvent::note(pitches, 100).unwrap();
        let MusicEvent::Note(payload) = &event else { panic!("note payload") };
        assert_eq!(payload.pitches().len(), 1);
        assert_eq!(*payload.velocity(), 100);
        let encoded = event.clone().encode().unwrap();
        assert_eq!(MusicEvent::decode(&encoded).unwrap(), event);

        let rest = MusicEvent::rest();
        let encoded = rest.clone().encode().unwrap();
        assert_eq!(MusicEvent::decode(&encoded).unwrap(), rest);

        let position = Position::new(
            conduit_core::Quantity::new(3, conduit_core::QuantityUnit::Meter),
            conduit_core::Quantity::new(5, conduit_core::QuantityUnit::Meter),
        ).unwrap();
        let encoded = position.clone().encode().unwrap();
        assert_eq!(Position::decode(&encoded).unwrap(), position);

        let evidence = BoundedBytes::<4096>::new(b"sha256:truth").unwrap();
        let observation = Observation::new(evidence, Some(Note::new(64).unwrap())).unwrap();
        let encoded = observation.clone().encode().unwrap();
        assert_eq!(Observation::decode(&encoded).unwrap(), observation);
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

    let no_std_source = directory.join("bindings-no-std.rs");
    fs::write(&no_std_source, format!("#![no_std]\n{}", generated.source)).unwrap();
    let no_std_library = directory.join("libbindings_no_std.rlib");
    let no_std_output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--crate-type=lib"])
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_form={}", form.display()))
        .arg(&no_std_source)
        .arg("-o")
        .arg(&no_std_library)
        .output()
        .unwrap();
    assert!(
        no_std_output.status.success(),
        "generated no_std Rust failed to compile:\n{}\n{}",
        String::from_utf8_lossy(&no_std_output.stdout),
        String::from_utf8_lossy(&no_std_output.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn locked_package_generation_revalidates_exact_source_without_network_work() {
    let manifest_source = "pack example/music (\n    version = 1.0.0\n) {\n    ship Note\n}\n";
    let parsed = crate::parse_syntax_document(manifest_source);
    let manifest = &parsed.packages[0];
    let members = [crate::PackageMemberSource {
        path: "types",
        source: "type Note = U8 in 0..=127\n",
    }];
    let bundle =
        crate::CheckedPackageBundle::from_sources(manifest_source, manifest, &members).unwrap();
    let lock = crate::resolve_package_lock(
        core::slice::from_ref(&bundle.package),
        &[(bundle.package.path.as_str(), bundle.package.version)],
    )
    .unwrap();
    let root = LockedPackageBindingSource {
        bundle: &bundle,
        manifest_source,
        manifest,
        member_sources: &members,
    };
    let generated = generate_locked_package_rust_bindings(
        LockedPackageRustBindingInput {
            root,
            locked_sources: core::slice::from_ref(&root),
            lock: &lock,
        },
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated.source.contains("pub struct Note(u8);"));

    let unlocked =
        crate::resolve_package_lock(core::slice::from_ref(&bundle.package), &[]).unwrap();
    assert_eq!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: core::slice::from_ref(&root),
                lock: &unlocked,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::SourceNotLocked)
    );
}

#[test]
fn locked_dependency_types_come_only_from_their_exact_source_bundle() {
    let base_manifest_source = "pack example/base (\n    version = 1.0.0\n) {\n    ship Note\n}\n";
    let base_document = crate::parse_syntax_document(base_manifest_source);
    let base_manifest = &base_document.packages[0];
    let base_members = [crate::PackageMemberSource {
        path: "types",
        source: "type Note = U8 in 0..=127\n",
    }];
    let base_bundle = crate::CheckedPackageBundle::from_sources(
        base_manifest_source,
        base_manifest,
        &base_members,
    )
    .unwrap();

    let root_manifest_source = "pack example/music (\n    version = 1.0.0\n) {\n    ship Event\n    need example/base = ^1.0\n}\n";
    let root_document = crate::parse_syntax_document(root_manifest_source);
    let root_manifest = &root_document.packages[0];
    let root_members = [crate::PackageMemberSource {
        path: "types",
        source: "type Event = {\n    pitch: example/base/Note\n}\n",
    }];
    let root_bundle = crate::CheckedPackageBundle::from_sources(
        root_manifest_source,
        root_manifest,
        &root_members,
    )
    .unwrap();
    let package_catalog = [root_bundle.package.clone(), base_bundle.package.clone()];
    let lock = crate::resolve_package_lock(
        &package_catalog,
        &[(
            root_bundle.package.path.as_str(),
            root_bundle.package.version,
        )],
    )
    .unwrap();
    let root = LockedPackageBindingSource {
        bundle: &root_bundle,
        manifest_source: root_manifest_source,
        manifest: root_manifest,
        member_sources: &root_members,
    };
    let base = LockedPackageBindingSource {
        bundle: &base_bundle,
        manifest_source: base_manifest_source,
        manifest: base_manifest,
        member_sources: &base_members,
    };
    let generated = generate_locked_package_rust_bindings(
        LockedPackageRustBindingInput {
            root,
            locked_sources: &[root, base],
            lock: &lock,
        },
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated.source.contains("pub struct Event {"));
    assert!(generated.source.contains("pub struct Note(u8);"));
    assert!(generated.source.contains("pitch: Note"));

    assert_eq!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root],
                lock: &lock,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::MissingLockedSource(
            "example/base".into()
        ))
    );
}
