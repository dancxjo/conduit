use super::*;
use alloc::string::ToString;
use alloc::vec::Vec;

fn checked_types() -> Vec<crate::CheckedNativeType> {
    let source = r#"type Note = U8 in 0..=127

type Position = {
    x: Distance
    y: Distance
}

type Chord = {
    notes: collection Note = 3
}

type Phrase = {
    notes: sequence Note in 2..=3
}

type Interval = {
    start: U32
    end: U32
    where .start <= .end
}

type EvidenceObservation = {
    note: Note?
    evidence: Bytes
}

type Digest = collection U8 = 32

type OptionalDigest = Digest?

type DigestSet = sequence Digest <= 16

type RawDigestPage = sequence collection U8 = 32 in 1..=2

type Observation = {
    identity: Digest
    source: OptionalDigest
    ancestors: DigestSet
}

type FiniteF32 = F32 finite

type Probability = F32 finite in 0.0..=1.0

type Holder<T> = {
    value: T
    history: sequence T <= 2
}

type TextHolder = Holder<Text <= 16B>

type Nested<T> = {
    value: T
}

type UsesNested = {
    nested: Nested<U16>
}

type FloatEnvelope = {
    value: F32
    finite: FiniteF32
    probability: Probability
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

type Timing =
    estimated {
        uncertainty: Duration
    }
    | exact

type Refusal =
    unavailable

type Outcome =
    completed
    | refused Refusal
"#;
    crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap()
    .native_types
}

#[test]
fn selected_record_can_retain_established_public_fields() {
    let generated = generate_rust_bindings(
        &checked_types(),
        &RustBindingOptions {
            public_record_fields: ["Position".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert!(generated
        .source
        .contains("pub struct Position {\n    pub x:"));
    assert!(generated.source.contains("    pub y:"));
    assert!(generated.source.contains("pub struct Chord {\n    notes:"));
}

#[test]
fn byte_bindings_retain_the_full_checked_structured_capacity() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type JobOutput = Bytes <= 65536B\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();

    assert!(generated
        .source
        .contains("pub struct JobOutput(BoundedBytes<65536>)"));
}

#[test]
fn quantity_unit_fields_use_the_canonical_native_unit_vocabulary() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Axis = {\n unit: QuantityUnit?\n}\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();

    assert!(generated
        .source
        .contains("unit: Option<conduit_core::QuantityUnit>"));
}

#[test]
fn exact_float_law_generates_semantic_wrappers_and_checked_refinements() {
    let generated = generate_rust_bindings(
        &checked_types(),
        &RustBindingOptions {
            copy_nominal_types: ["FiniteF32".into(), "Probability".into()].into(),
            hash_nominal_types: ["FiniteF32".into(), "Probability".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert!(generated
        .source
        .contains("pub struct FiniteF32(conduit_core::IeeeF32)"));
    assert!(generated
        .source
        .contains("conduit_core::ValueConstraint::FloatFinite"));
    assert!(generated
        .source
        .contains("conduit_core::ValueConstraint::FloatRange"));
    assert!(generated.source.contains("value: conduit_core::IeeeF32"));
}

#[test]
fn exact_reference_leaves_use_validated_native_bindings() {
    let source = "type Image = Bytes\ntype References = {\n    text: &Text\n    image: &Image\n    resource: ResourceRef\n}\n";
    let mut catalog = crate::StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let checked =
        crate::check_syntax_document(&crate::parse_syntax_document(source), &catalog).unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();

    assert!(generated
        .source
        .contains("text: conduit_data::DataReference"));
    assert!(generated
        .source
        .contains("image: conduit_data::DataReference"));
    assert!(generated
        .source
        .contains("resource: conduit_core::BoundedResourceRef"));
    assert!(generated
        .source
        .contains("conduit_data::DataReference::decode_for"));
    assert!(generated
        .source
        .contains("conduit_core::BoundedResourceRef::decode(encoded)"));
    assert!(!generated.source.contains("text: BoundedBytes"));
    assert!(!generated.source.contains("image: BoundedBytes"));
    assert!(!generated.source.contains("resource: BoundedBytes"));
}

#[test]
fn checked_form_generates_the_only_rust_discriminant_table() {
    let source = "type Outcome =\n    ready\n    | refused\n\nform test/outcome = Outcome as u8\n";
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated.source.contains("pub struct OutcomeForm;"));
    assert!(generated.source.contains("Outcome::Ready => 0"));
    assert!(generated.source.contains("1 => Ok(Outcome::Refused)"));
    assert!(generated.source.contains("MAXIMUM_DECODE_STEPS: usize = 3"));
    assert!(generated.source.contains("NativeFormRefusal::InvalidTag"));
}

#[test]
fn generated_code_preserves_explicit_iota_origin() {
    let source =
        "type Outcome =\n    ready\n    | refused\n\nform test/outcome = Outcome as u8 from 1\n";
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(source),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated.source.contains("Outcome::Ready => 1"));
    assert!(generated.source.contains("2 => Ok(Outcome::Refused)"));
}

#[test]
fn rust_only_variant_order_preserves_serde_abi_without_changing_type_identity() {
    let types = checked_types();
    let ordinary = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    let ordered = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_orders: [("Direction".into(), vec!["south".into(), "north".into()])]
                .into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert_eq!(ordinary.semantic_type_bytes, ordered.semantic_type_bytes);
    assert!(ordered
        .source
        .contains("pub enum Direction {\n    South,\n    North,"));
    assert!(matches!(
        generate_rust_bindings(
            &types,
            &RustBindingOptions {
                serde_variant_orders: [("Direction".into(), vec!["north".into()])].into(),
                ..RustBindingOptions::default()
            }
        ),
        Err(RustBindingGenerationError::InvalidSemanticType)
    ));
}

#[test]
fn serde_variant_exclusion_preserves_a_non_serde_binding() {
    let types = checked_types();
    let generated = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: [("MusicEvent").into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();

    assert!(generated
        .source
        .contains("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct MusicEventNote"));
    assert!(generated
        .source
        .contains("#[derive(Debug, Clone, PartialEq, Eq)]\npub enum MusicEvent"));
    assert!(generated.source.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]\npub enum Direction"
    ));
}

#[test]
fn selected_record_retains_copy_and_serde_binding_traits() {
    let types = checked_types();
    let generated = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            serde_record_types: ["Toggle".into()].into(),
            copy_record_types: ["Toggle".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();

    assert!(generated.source.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\npub struct Toggle"
    ));
    assert!(generated
        .source
        .contains("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct Position"));
}

#[test]
fn selected_record_can_retain_established_constructor_argument_order() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Pair = {\n    left: U32\n    right: U16\n}\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            record_constructor_orders: [("Pair".into(), vec!["right".into(), "left".into()])]
                .into(),
            copy_record_types: ["Pair".into()].into(),
            copy_record_value_getters: ["Pair".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();

    assert!(generated
        .source
        .contains("pub fn new(right: u16, left: u32)"));
    assert!(generated.source.contains("Ok(Self { left, right, })"));
    assert!(generated
        .source
        .contains("pub const fn left(self) -> u32 { self.left }"));
}

#[test]
fn wide_record_constructor_allows_its_generated_argument_shape() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(
            "type Wide = {\n    a: U8\n    b: U8\n    c: U8\n    d: U8\n    e: U8\n    f: U8\n    g: U8\n    h: U8\n}\n",
        ),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();

    assert!(generated
        .source
        .contains("#[allow(clippy::too_many_arguments)]\n    pub fn new(a: u8, b: u8, c: u8"));
}

#[test]
fn record_constructor_order_must_name_every_field_exactly_once() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Pair = {\n    left: U32\n    right: U16\n}\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    for order in [
        vec!["left".into()],
        vec!["left".into(), "left".into()],
        vec!["left".into(), "missing".into()],
    ] {
        assert_eq!(
            generate_rust_bindings(
                &checked.native_types,
                &RustBindingOptions {
                    record_constructor_orders: [("Pair".into(), order)].into(),
                    ..RustBindingOptions::default()
                },
            ),
            Err(RustBindingGenerationError::InvalidSemanticType)
        );
    }
}

#[test]
fn constrained_copy_record_validation_does_not_clone_the_candidate() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Bounded = {\n    value: U32 in 1..=8\n}\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            copy_record_types: ["Bounded".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert!(generated
        .source
        .contains("let structured = candidate.into_structured()?;"));
    assert!(!generated
        .source
        .contains("let structured = candidate.clone().into_structured()?;"));
}

#[test]
fn selected_serde_record_can_retain_deny_unknown_fields() {
    let types = checked_types();
    let generated = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            serde_record_types: ["Toggle".into()].into(),
            serde_deny_unknown_record_types: ["Toggle".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert!(generated.source.contains(
        "#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct Toggle"
    ));

    assert!(matches!(
        generate_rust_bindings(
            &types,
            &RustBindingOptions {
                serde_deny_unknown_record_types: ["Toggle".into()].into(),
                ..RustBindingOptions::default()
            }
        ),
        Err(RustBindingGenerationError::InvalidSemanticType)
    ));
}

#[test]
fn selected_constrained_record_validates_direct_integer_bounds_without_structured_allocation() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Bounded = {\n    value: U64 in 1..=8\n}\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            direct_checked_record_constructors: ["Bounded".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();

    assert!(generated.source.contains("(1u64..=8u64).contains(&value)"));
    assert!(generated
        .source
        .contains("ValueConstraintRefusal::FixedIntegerRange"));
    assert!(!generated
        .source
        .contains("let structured = candidate.clone().into_structured()?;"));
}

#[test]
fn constrained_integer_nominal_validates_without_structured_allocation() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document("type Positive = U64 in 1..\n"),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();

    assert!(generated.source.contains("value >= 1u64"));
    assert!(generated
        .source
        .contains("ValueConstraintRefusal::FixedIntegerRange"));
    assert!(!generated
        .source
        .contains("let structured = candidate.clone().into_structured()?;"));
}

#[test]
fn generated_contracts_preserve_semantic_openness() {
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(
            "type Positive = Count in 0..\ntype AtMostOne = Scalar in ..=1.000000\n",
        ),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();
    assert!(generated.source.contains("minimum: Some(0), maximum: None"));
    assert!(generated
        .source
        .contains("minimum: None, maximum: Some(1000000)"));
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
            derive_serde_for_variants: true,
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
        "pub fn estimated(uncertainty: conduit_core::Quantity) -> Result<Self, NativeBindingRefusal> { let candidate = Self::Estimated"
    ));
    assert!(plain.source.contains("Refused(Refusal),"));
    assert!(plain
        .source
        .contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Outcome"));
    assert!(plain
        .source
        .contains("pub fn refused(payload: Refusal) -> Result<Self, NativeBindingRefusal>"));
    assert!(!plain.source.contains("struct OutcomeRefused"));
    assert!(plain.source.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum Direction"
    ));
    assert!(prefixed.source.contains("pub struct FixtureNote(u8);"));
    assert!(serde_unit_variants
        .source
        .contains("Hash, serde::Serialize, serde::Deserialize)]\npub enum Direction"));
    assert!(serde_unit_variants.source.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\npub enum Outcome"
    ));
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

    let generated = generate_rust_bindings(
        &checked_types(),
        &RustBindingOptions {
            prepared_family_roots: [
                "Chord".into(),
                "MusicEvent".into(),
                "Observation".into(),
                "Interval".into(),
                "Input".into(),
            ]
            .into(),
            boxed_variant_payloads: ["MusicEvent.note".into()].into(),
            copy_nominal_types: ["Digest".into()].into(),
            hash_nominal_types: ["Digest".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
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
            .filter(|path| {
                path.extension() == Some(OsStr::new("rlib"))
                    && path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| name.starts_with(&prefix))
            })
            .max_by_key(|path| {
                path.metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()
            })
            .unwrap_or_else(|| panic!("missing {crate_name} rlib in {}", dependencies.display()))
    };
    let plot = find_rlib("conduit_plot");
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

        let chord = Chord::new([
            Note::new(60).unwrap(),
            Note::new(64).unwrap(),
            Note::new(67).unwrap(),
        ]).unwrap();
        assert_eq!(chord.notes().len(), 3);
        let encoded = chord.clone().encode().unwrap();
        assert_eq!(Chord::decode(&encoded).unwrap(), chord);

        let underfull = Phrase::new(BoundedSequence::<Note, 3>::new());
        assert!(matches!(
            underfull,
            Err(NativeBindingRefusal::InvalidValue(
                conduit_core::StructuredInfoRefusal::WrongCollectionLength
            ))
        ));
        let mut notes = BoundedSequence::<Note, 3>::new();
        notes.push(Note::new(60).unwrap()).unwrap();
        notes.push(Note::new(64).unwrap()).unwrap();
        let phrase = Phrase::new(notes).unwrap();
        let encoded = phrase.clone().encode().unwrap();
        assert_eq!(Phrase::decode(&encoded).unwrap(), phrase);

        let interval = Interval::new(5, 4).unwrap();
        assert!(Interval::new(4, 5).is_err());
        let encoded = interval.clone().encode().unwrap();
        assert_eq!(Interval::decode(&encoded).unwrap(), interval);

        let evidence = BoundedBytes::<65536>::new(b"sha256:truth").unwrap();
        let observation = EvidenceObservation::new(evidence, Some(Note::new(64).unwrap())).unwrap();
        let encoded = observation.clone().encode().unwrap();
        assert_eq!(EvidenceObservation::decode(&encoded).unwrap(), observation);

        fn requires_digest_traits<T: Copy + Eq + core::hash::Hash>() {}
        requires_digest_traits::<Digest>();
        let identity = Digest::new([7; 32]).unwrap();
        let digest_type = Digest::semantic_type().unwrap();
        let representation = conduit_plot::rust_binding::nominal_representation_type(&digest_type).unwrap();
        assert!(matches!(
            StructuredInfoValue::collection(representation, Vec::new()),
            Err(conduit_core::StructuredInfoRefusal::WrongCollectionLength)
        ));
        let mut ancestors = BoundedSequence::<Digest, 16>::new();
        ancestors.push(Digest::new([8; 32]).unwrap()).unwrap();
        let observation = Observation::new(
            DigestSet::new(ancestors).unwrap(),
            identity,
            OptionalDigest::new(Some(identity)).unwrap(),
        ).unwrap();
        let encoded = observation.clone().encode().unwrap();
        assert_eq!(Observation::decode(&encoded).unwrap(), observation);

        let mut raw_digests = BoundedSequence::<[u8; 32], 2>::new();
        raw_digests.push([9; 32]).unwrap();
        let raw_page = RawDigestPage::new(raw_digests).unwrap();
        let encoded = raw_page.clone().encode().unwrap();
        assert_eq!(RawDigestPage::decode(&encoded).unwrap(), raw_page);

        let mut history = BoundedSequence::<String, 2>::new();
        history.push("prior".into()).unwrap();
        let holder = TextHolder::new(history, "current".into()).unwrap();
        let encoded = holder.clone().encode().unwrap();
        assert_eq!(TextHolder::decode(&encoded).unwrap(), holder);

    }
}

"#;
    let prepared_exercise = r#"
#[test]
fn prepared_recursive_family_matches_existing_entrances() {
    use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeFamilyLimits};
    let limits = PreparedNativeFamilyLimits {
        maximum_types: 64, maximum_laws_per_type: 64,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_retained_bytes: 64 * 1024 * 1024,
        maximum_preparation_peak_bytes: 128 * 1024 * 1024,
        maximum_conversion_requested_bytes: usize::MAX,
    };
    let mut family = PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS, limits).unwrap();
    use conduit_plot::rust_binding::{PreparedNativeRustBinding, NativeFamilyTypeDescriptor};
    let only_chord = PreparedNativeFamily::prepare(&[Chord::PREPARED_DESCRIPTOR], limits).unwrap();
    assert!(only_chord.contains_descriptor(Chord::PREPARED_DESCRIPTOR));
    assert!(only_chord.contains_descriptor(Note::PREPARED_DESCRIPTOR));
    assert!(!only_chord.contains_descriptor(Observation::PREPARED_DESCRIPTOR));
    let original = Chord::PREPARED_DESCRIPTOR;
    let foreign = Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: original.type_bytes, laws: original.laws, contracts: original.contracts,
        children: original.children, conversion_profile: original.conversion_profile,
        maximum_inline_bytes: original.maximum_inline_bytes,
    }));
    assert_eq!(foreign.type_bytes, original.type_bytes);
    assert!(!only_chord.contains_descriptor(foreign));

    let chord = Chord::new([Note::new(3).unwrap(), Note::new(7).unwrap(), Note::new(9).unwrap()]).unwrap();
    let encoded = chord.clone().encode().unwrap();
    assert_eq!(family.decode::<Chord>(&encoded), Chord::decode(&encoded));
    assert!(family.decode::<Note>(&encoded).is_err());
    let chord_type = Chord::semantic_type().unwrap();
    let notes_type = conduit_plot::rust_binding::record_field_type(&chord_type, "notes").unwrap();
    let note_type = Note::semantic_type().unwrap();
    let representation = conduit_plot::rust_binding::nominal_representation_type(&note_type).unwrap();
    let bad_note = StructuredInfoValue::nominal(note_type,
        conduit_plot::rust_binding::primitive_into_structured(representation, &128u8).unwrap()).unwrap();
    let invalid_child = StructuredInfoValue::record(chord_type, vec![StructuredFieldValue::new("notes",
        StructuredInfoValue::collection(notes_type, vec![bad_note, Note::new(7).unwrap().into_structured().unwrap(), Note::new(9).unwrap().into_structured().unwrap()]).unwrap()).unwrap()]).unwrap().canonical_bytes().unwrap();
    assert_eq!(family.decode::<Chord>(&invalid_child), Chord::decode(&invalid_child));
    assert!(family.decode::<Chord>(&invalid_child).is_err());
    // The reference from_structured variant entrance does not call the public
    // case constructor. Preserve that distinction, including its own bounds.
    let music_type = MusicEvent::semantic_type().unwrap();
    let payload_type = conduit_plot::rust_binding::variant_payload_type(&music_type, "note").unwrap();
    let pitches_type = conduit_plot::rust_binding::record_field_type(&payload_type, "pitches").unwrap();
    let velocity_type = conduit_plot::rust_binding::record_field_type(&payload_type, "velocity").unwrap();
    let variant = StructuredInfoValue::variant(music_type, "note", StructuredInfoValue::record(payload_type, vec![
        StructuredFieldValue::new("pitches", StructuredInfoValue::sequence(pitches_type, vec![]).unwrap()).unwrap(),
        StructuredFieldValue::new("velocity", conduit_plot::rust_binding::primitive_into_structured(velocity_type, &128u8).unwrap()).unwrap(),
    ]).unwrap()).unwrap().canonical_bytes().unwrap();
    assert_eq!(family.decode::<MusicEvent>(&variant), MusicEvent::decode(&variant));
    assert!(family.decode::<MusicEvent>(&variant).is_ok());
    let semantic = Interval::semantic_type().unwrap();
    let end = conduit_plot::rust_binding::record_field_type(&semantic, "end").unwrap();
    let start = conduit_plot::rust_binding::record_field_type(&semantic, "start").unwrap();
    let invalid = StructuredInfoValue::record(semantic, vec![
        StructuredFieldValue::new("end", conduit_plot::rust_binding::primitive_into_structured(end, &3u32).unwrap()).unwrap(),
        StructuredFieldValue::new("start", conduit_plot::rust_binding::primitive_into_structured(start, &7u32).unwrap()).unwrap(),
    ]).unwrap().canonical_bytes().unwrap();
    assert_eq!(family.decode::<Interval>(&invalid), Interval::decode(&invalid));
    let mut malformed = encoded.clone(); malformed.pop();
    assert!(family.decode::<Chord>(&malformed).is_err());
    let receipt = family.storage_receipt();
    assert!(PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,
        PreparedNativeFamilyLimits { maximum_retained_bytes: receipt.retained_heap_bytes_bound - 1, ..limits }).is_err());
    assert!(PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,
        PreparedNativeFamilyLimits { maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1, ..limits }).is_err());
}
"#;
    fs::write(
        &source,
        format!("{}{}{}", generated.source, exercise, prepared_exercise),
    )
    .unwrap();
    let executable = directory.join("bindings-test");
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test"])
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_plot={}", plot.display()))
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
        .arg(format!("conduit_plot={}", plot.display()))
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
fn binding_only_boxing_preserves_semantic_identity_and_refuses_invalid_targets() {
    let types = checked_types();
    let plain = generate_rust_bindings(&types, &RustBindingOptions::default()).unwrap();
    let boxed = generate_rust_bindings(
        &types,
        &RustBindingOptions {
            boxed_variant_payloads: ["MusicEvent.note".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    assert_eq!(plain.semantic_type_bytes, boxed.semantic_type_bytes);
    assert!(boxed.source.contains("Note(Box<MusicEventNote>)"));
    for path in ["Missing.note", "MusicEvent.missing", "MusicEvent.rest"] {
        assert_eq!(
            generate_rust_bindings(
                &types,
                &RustBindingOptions {
                    boxed_variant_payloads: [path.into()].into(),
                    ..RustBindingOptions::default()
                },
            ),
            Err(RustBindingGenerationError::InvalidSemanticType)
        );
    }
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
            external_bindings: &[],
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
                external_bindings: &[],
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::SourceNotLocked)
    );
}

#[test]
fn locked_dependency_types_come_only_from_their_exact_source_bundle() {
    let base_manifest_source =
        "pack example/base (\n    version = 1.0.0\n) {\n    ship Note\n    ship Tempo\n}\n";
    let base_document = crate::parse_syntax_document(base_manifest_source);
    let base_manifest = &base_document.packages[0];
    let base_members = [crate::PackageMemberSource {
        path: "types",
        source: "type Note = U8 in 0..=127\ntype Tempo = U16 in 1..=400\n",
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
        source: "type Envelope<T> = {\n    value: T\n    history: sequence T <= 2\n}\n\ntype NoteEnvelope = Envelope<example/base/Note>\n\ntype Event = {\n    pitch: example/base/Note\n    history: sequence example/base/Note <= 4\n    previous: example/base/Note?\n}\n\ntype Choice =\n    selected example/base/Note\n    | absent\n",
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
    let foreign_types = crate::PackageExportCatalog::from_bundle(
        &base_bundle,
        base_manifest_source,
        base_manifest,
        &base_members,
    )
    .unwrap()
    .install_shipped_types(&mut crate::StartupCatalog::new())
    .unwrap();
    let foreign_identity = foreign_types
        .iter()
        .find(|value_type| value_type.name == "Note")
        .unwrap()
        .identity
        .as_str()
        .to_string();
    let unused_identity = foreign_types
        .iter()
        .find(|value_type| value_type.name == "Tempo")
        .unwrap()
        .identity
        .as_str()
        .to_string();
    let external_bindings = [ExternalNativeRustBinding {
        semantic_identity: &foreign_identity,
        rust_type_path: "dependency::Note",
    }];
    assert!(matches!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root, base],
                lock: &lock,
                external_bindings: &[],
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::MissingExternalBinding(identity))
            if identity == foreign_identity
    ));
    let duplicate_bindings = [external_bindings[0], external_bindings[0]];
    assert!(matches!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root, base],
                lock: &lock,
                external_bindings: &duplicate_bindings,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::DuplicateExternalBinding(identity))
            if identity == foreign_identity
    ));
    let invalid_path_bindings = [ExternalNativeRustBinding {
        semantic_identity: &foreign_identity,
        rust_type_path: "dependency::not-a-type",
    }];
    assert!(matches!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root, base],
                lock: &lock,
                external_bindings: &invalid_path_bindings,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::InvalidExternalRustPath(path))
            if path == "dependency::not-a-type"
    ));
    let drifted_bindings = [ExternalNativeRustBinding {
        semantic_identity: "type:stale-dependency-identity",
        rust_type_path: "dependency::Note",
    }];
    assert!(matches!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root, base],
                lock: &lock,
                external_bindings: &drifted_bindings,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::ExternalBindingIdentityDrift(identity))
            if identity == "type:stale-dependency-identity"
    ));
    let unused_bindings = [
        external_bindings[0],
        ExternalNativeRustBinding {
            semantic_identity: &unused_identity,
            rust_type_path: "dependency::Tempo",
        },
    ];
    assert!(matches!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root, base],
                lock: &lock,
                external_bindings: &unused_bindings,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::UnusedExternalBinding(identity))
            if identity == unused_identity
    ));
    let generated = generate_locked_package_rust_bindings(
        LockedPackageRustBindingInput {
            root,
            locked_sources: &[root, base],
            lock: &lock,
            external_bindings: &external_bindings,
        },
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert!(generated.source.contains("pub struct Event {"));
    assert!(!generated.source.contains("pub struct Note(u8);"));
    assert!(generated.source.contains("pitch: dependency::Note"));
    assert!(generated
        .source
        .contains("BoundedSequence<dependency::Note, 4>"));
    assert!(generated.source.contains("Option<dependency::Note>"));
    assert!(generated.source.contains("Selected(dependency::Note)"));
    assert!(generated.source.contains("pub struct NoteEnvelope"));
    assert!(generated.source.contains("value: dependency::Note"));
    assert!(generated
        .source
        .contains("BoundedSequence<dependency::Note, 2>"));
    let renamed_path = generate_locked_package_rust_bindings(
        LockedPackageRustBindingInput {
            root,
            locked_sources: &[root, base],
            lock: &lock,
            external_bindings: &[ExternalNativeRustBinding {
                semantic_identity: &foreign_identity,
                rust_type_path: "other_dependency::RenamedNote",
            }],
        },
        &RustBindingOptions::default(),
    )
    .unwrap();
    assert_eq!(
        generated.semantic_type_bytes,
        renamed_path.semantic_type_bytes
    );

    let dependency =
        generate_rust_bindings(&foreign_types, &RustBindingOptions::default()).unwrap();
    compile_external_binding_round_trip(&dependency.source, &generated.source);

    assert_eq!(
        generate_locked_package_rust_bindings(
            LockedPackageRustBindingInput {
                root,
                locked_sources: &[root],
                lock: &lock,
                external_bindings: &external_bindings,
            },
            &RustBindingOptions::default(),
        ),
        Err(LockedRustBindingGenerationError::MissingLockedSource(
            "example/base".into()
        ))
    );
}

#[test]
fn ordinary_generation_refuses_missing_drifted_and_unused_external_bindings() {
    use super::{
        generate_rust_bindings_with_external_bindings, ExternalRustBindingGenerationError,
    };
    let checked = |source: &str, catalog: &crate::StartupCatalog| {
        crate::check_syntax_document(&crate::parse_syntax_document(source), catalog).unwrap()
    };
    let foreign = checked(
        "type Foreign = {\n    value: U8?\n}\n",
        &crate::StartupCatalog::new(),
    )
    .native_types
    .into_iter()
    .next()
    .unwrap();
    let unrelated = checked(
        "type Unrelated = {\n    value: U16\n}\n",
        &crate::StartupCatalog::new(),
    )
    .native_types
    .into_iter()
    .next()
    .unwrap();
    let mut catalog = crate::StartupCatalog::new();
    catalog
        .insert_structured_type("Foreign", foreign.value_type.clone())
        .unwrap();
    let owned = checked("type Owned = {\n    foreign: Foreign\n}\n", &catalog);
    let binding = ExternalNativeRustBinding {
        semantic_identity: foreign.identity.as_str(),
        rust_type_path: "dependency::Foreign",
    };
    let options = RustBindingOptions::default();
    assert!(matches!(
        generate_rust_bindings_with_external_bindings(
            &owned.native_types,
            core::slice::from_ref(&foreign.value_type),
            &[],
            &options,
        ),
        Err(ExternalRustBindingGenerationError::MissingExternalBinding(identity))
            if identity == foreign.identity.as_str()
    ));
    assert!(matches!(
        generate_rust_bindings_with_external_bindings(
            &owned.native_types,
            core::slice::from_ref(&foreign.value_type),
            &[ExternalNativeRustBinding {
                semantic_identity: "type:drifted",
                rust_type_path: "dependency::Foreign",
            }],
            &options,
        ),
        Err(ExternalRustBindingGenerationError::ExternalBindingIdentityDrift(identity))
            if identity == "type:drifted"
    ));
    assert!(matches!(
        generate_rust_bindings_with_external_bindings(
            &owned.native_types,
            &[foreign.value_type.clone(), unrelated.value_type.clone()],
            &[
                binding,
                ExternalNativeRustBinding {
                    semantic_identity: unrelated.identity.as_str(),
                    rust_type_path: "dependency::Unrelated",
                },
            ],
            &options,
        ),
        Err(ExternalRustBindingGenerationError::UnusedExternalBinding(identity))
            if identity == unrelated.identity.as_str()
    ));
    let generated = generate_rust_bindings_with_external_bindings(
        &owned.native_types,
        &[foreign.value_type],
        &[binding],
        &options,
    )
    .unwrap();
    assert!(generated.source.contains("foreign: dependency::Foreign"));
}

fn compile_external_binding_round_trip(dependency: &str, root: &str) {
    use std::ffi::OsStr;
    use std::fs;
    use std::process::Command;
    use std::string::String;

    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let plot = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension() == Some(OsStr::new("rlib"))
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with("libconduit_plot-"))
        })
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit-external-rust-bindings-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("bindings.rs");
    let exercise = r#"
#[test]
fn external_values_round_trip_through_root_owned_shapes() {
    let note = dependency::Note::new(64).unwrap();
    let mut history = BoundedSequence::<dependency::Note, 4>::new();
    history.push(note.clone()).unwrap();
    let event = Event::new(history, note.clone(), Some(note.clone())).unwrap();
    let encoded = event.clone().encode().unwrap();
    assert_eq!(Event::decode(&encoded).unwrap(), event);
    let choice = Choice::selected(note).unwrap();
    let encoded = choice.clone().encode().unwrap();
    assert_eq!(Choice::decode(&encoded).unwrap(), choice);
}
"#;
    fs::write(
        &source,
        format!("mod dependency {{ {dependency} }}\n{root}\n{exercise}"),
    )
    .unwrap();
    let executable = directory.join("bindings-test");
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--test"])
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_plot={}", plot.display()))
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "external generated Rust failed to compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let execution = Command::new(&executable).output().unwrap();
    assert!(
        execution.status.success(),
        "external generated Rust round trip failed:\n{}\n{}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn shared_generated_descriptor_union_has_separate_runtime_and_generation_ceilings() {
    use alloc::string::String;
    use core::fmt::Write;

    assert_eq!(MAXIMUM_NATIVE_FAMILY_TYPES, 64);
    assert_eq!(MAXIMUM_GENERATED_NATIVE_FAMILY_TYPES, 256);
    // Runtime root-count admission remains independent of the emitted union.
    let bytes = conduit_core::StructuredInfoType::nominal(
        conduit_core::KindId::new("test/finite-root"),
        conduit_core::StructuredInfoType::leaf(conduit_core::KindId::new("value/u8")).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let descriptor: &'static NativeFamilyTypeDescriptor =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(NativeFamilyTypeDescriptor {
            type_bytes: alloc::boxed::Box::leak(bytes.into_boxed_slice()),
            laws: &[],
            contracts: &[],
            children: &[],
            conversion_profile: NativeFamilyConversionProfile::Nominal,
            maximum_inline_bytes: 1,
        }));
    let limits = PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 0,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: 1024 * 1024,
        maximum_preparation_peak_bytes: 2 * 1024 * 1024,
        maximum_conversion_requested_bytes: 1024 * 1024,
    };
    assert!(PreparedNativeFamily::prepare(&[descriptor; 16], limits).is_ok());
    assert!(matches!(
        PreparedNativeFamily::prepare(&[descriptor; 17], limits),
        Err(PreparedNativeFamilyRefusal::Capacity)
    ));

    let mut source = String::new();
    for index in 0..252 {
        writeln!(source, "type Item{index} = U8\n").unwrap();
    }
    for (name, start) in [
        ("FirstRoot", 0),
        ("SecondRoot", 63),
        ("ThirdRoot", 126),
        ("FourthRoot", 189),
    ] {
        writeln!(source, "type {name} = {{").unwrap();
        for index in 0..63 {
            writeln!(source, "field{index}: Item{}", start + index).unwrap();
        }
        writeln!(source, "}}\n").unwrap();
    }
    writeln!(source, "type ExtraRoot = U8\ntype OversizedRoot = {{").unwrap();
    for index in 0..64 {
        writeln!(source, "field{index}: Item{index}").unwrap();
    }
    writeln!(source, "}}\n").unwrap();
    let checked = crate::check_syntax_document(
        &crate::parse_syntax_document(&source),
        &crate::StartupCatalog::new(),
    )
    .unwrap();
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            prepared_family_roots: [
                "FirstRoot".into(),
                "SecondRoot".into(),
                "ThirdRoot".into(),
                "FourthRoot".into(),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .unwrap();
    // Four disjoint complete 64-Type roots share one generated module without
    // authorizing a runtime owner to retain their combined 256-Type union.
    assert_eq!(
        generated
            .source
            .matches("_PREPARED_NATIVE_DESCRIPTOR: ")
            .count(),
        256
    );
    for roots in [
        [
            "FirstRoot",
            "SecondRoot",
            "ThirdRoot",
            "FourthRoot",
            "ExtraRoot",
        ]
        .as_slice(),
        ["OversizedRoot"].as_slice(),
    ] {
        assert_eq!(
            generate_rust_bindings(
                &checked.native_types,
                &RustBindingOptions {
                    prepared_family_roots: roots.iter().map(|name| (*name).into()).collect(),
                    ..RustBindingOptions::default()
                },
            ),
            Err(RustBindingGenerationError::InvalidSemanticType)
        );
    }
    assert!(matches!(
        PreparedNativeFamily::prepare(
            &[],
            PreparedNativeFamilyLimits {
                maximum_types: 65,
                maximum_laws_per_type: 0,
                maximum_input_bytes: 0,
                maximum_retained_bytes: 0,
                maximum_preparation_peak_bytes: 0,
                maximum_conversion_requested_bytes: 0,
            }
        ),
        Err(PreparedNativeFamilyRefusal::Capacity)
    ));
}
