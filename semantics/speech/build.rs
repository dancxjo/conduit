#[path = "build_support/authoring_types.rs"]
mod authoring_types;
#[path = "build_support/graph.rs"]
mod graph;
#[path = "build_support/lower.rs"]
mod lower;
#[path = "build_support/semantic_source.rs"]
mod semantic_source;
use conduit_core::StructuredInfoTypeShape;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=rule_status.conduit");
    println!("cargo:rerun-if-changed=selection.conduit");
    println!("cargo:rerun-if-changed=listening.conduit");
    println!("cargo:rerun-if-changed=translation.conduit");
    println!("cargo:rerun-if-changed=timing.conduit");
    println!("cargo:rerun-if-changed=intent.conduit");
    println!("cargo:rerun-if-changed=inventory.conduit");
    println!("cargo:rerun-if-changed=profile_phones.conduit");
    println!("cargo:rerun-if-changed=voice_profile.conduit");
    println!("cargo:rerun-if-changed=timing_projection.conduit");
    println!("cargo:rerun-if-changed=duration_projection.conduit");
    println!("cargo:rerun-if-changed=duration_render.conduit");
    println!("cargo:rerun-if-changed=control_projection.conduit");
    println!("cargo:rerun-if-changed=context_match.conduit");
    println!("cargo:rerun-if-changed=linguistic_prosody.conduit");
    println!("cargo:rerun-if-changed=pitch_trajectory.conduit");
    println!("cargo:rerun-if-changed=pitch_projection.conduit");
    for path in [
        "ipa.conduit",
        "ipa_syntax.conduit",
        "ipa_inventory.conduit",
        "ipa_constructors.conduit",
        "../language/identity.conduit",
        "build_support/semantic_source.rs",
        "build_support/authoring_types.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let semantic_source = semantic_source::source();
    let mut language_types = conduit_language::identity_types();
    language_types.extend(
        conduit_language::prosody::prosody_types()
            .into_iter()
            .filter(|(name, _)| {
                matches!(
                    *name,
                    "LanguageProsodyChoice"
                        | "LanguageProsodyBoundary"
                        | "LanguageProsodyProminence"
                        | "LanguageProsodyPitch"
                )
            }),
    );
    let mut semantic_catalog = StartupCatalog::new();
    let language_identities =
        semantic_source::language_identities().expect("Language-owned identity declarations check");
    for (name, ty) in &language_types {
        if let Some(checked) = language_identities
            .native_types
            .iter()
            .find(|ty| ty.name == *name)
        {
            assert_eq!(&checked.value_type, ty, "Language owner Type differs");
            semantic_catalog
                .insert_checked_native_type(*name, checked)
                .expect("Language-owned checked Type installs once");
        } else {
            semantic_catalog
                .insert_structured_type(*name, ty.clone())
                .expect("Language-owned Type installs once");
        }
    }
    let semantic =
        check_syntax_document(&parse_syntax_document(&semantic_source), &semantic_catalog)
            .expect("Speaking segment and listening contracts check");
    // Retain the complete checked contracts used by Native generation. Runtime
    // authoring imports these facts rather than checking the compiled source again.
    let authoring_types: Vec<authoring_types::CompiledType> = semantic
        .native_types
        .iter()
        .map(|ty| {
            (
                ty.name.clone(),
                ty.identity.as_str().into(),
                ty.value_type
                    .canonical_bytes()
                    .expect("checked Type encodes"),
                ty.value_contracts
                    .iter()
                    .map(|contract| {
                        (
                            contract.representation_path.clone(),
                            contract.contract.clone(),
                        )
                    })
                    .collect(),
                ty.invariants
                    .iter()
                    .map(|law| law.canonical_bytes().expect("checked law encodes"))
                    .collect(),
            )
        })
        .collect();
    let authoring_bytes =
        postcard::to_allocvec(&(semantic.source_document_id.as_str(), authoring_types))
            .expect("compiled Types encode");
    assert_eq!(
        authoring_types::decode(&authoring_bytes, semantic.source_document_id.as_str())
            .expect("compiled Types decode"),
        semantic.native_types,
        "compiled authoring retains every exact Type contract and law"
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("authoring_types.bin"),
        authoring_bytes,
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &semantic,
        "speech/linguistic-prosody",
        &ProfileCatalog::new(),
    )
    .expect("checked linguistic speech projection expands");
    assert_eq!(expanded.expanded.gears.len(), 1);
    let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one exact projection")
    };
    assert_eq!(entry.key, "program");
    let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
        panic!("portable projection program")
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linguistic_prosody_program.hex"),
        program,
    )
    .expect("retain speech projection");
    let ipa_expanded = expand_canonical_plot_for_authoring(
        &semantic,
        "speech/ipa-supported-unit",
        &ProfileCatalog::new(),
    )
    .expect("checked IPA syntax expands");
    let [entry] = ipa_expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one IPA syntax program")
    };
    let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
        panic!("portable IPA syntax program")
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("ipa_supported_unit_program.hex"),
        program,
    )
    .expect("retain IPA syntax program");
    let identities = language_types
        .iter()
        .map(|(_, ty)| match ty.shape() {
            StructuredInfoTypeShape::Nominal { schema, .. }
            | StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. } => schema.as_str().to_owned(),
            _ => panic!("Language identity has a nominal boundary"),
        })
        .collect::<Vec<_>>();
    let paths = language_types
        .iter()
        .map(|(name, _)| format!("conduit_language::{name}"))
        .collect::<Vec<_>>();
    // IPA profiles retain the exact Language-owned variety Type.
    let bindings = identities
        .iter()
        .zip(&paths)
        .zip(&language_types)
        .filter(|(_, (name, _))| {
            !matches!(
                *name,
                "LanguageTextReferenceMatch"
                    | "LanguageExternalIdentity"
                    | "LanguageProsodyBoundary"
                    | "LanguageProsodyProminence"
            )
        })
        .map(
            |((identity, path), _)| conduit_plot::rust_binding::ExternalNativeRustBinding {
                semantic_identity: identity,
                rust_type_path: path,
            },
        )
        .collect::<Vec<_>>();
    let external_types = language_types
        .iter()
        .map(|(_, ty)| ty.clone())
        .collect::<Vec<_>>();
    let bindings = conduit_plot::rust_binding::generate_rust_bindings_with_external_bindings(
        &semantic.native_types,
        &external_types,
        &bindings,
        &conduit_plot::rust_binding::RustBindingOptions::default(),
    )
    .expect("Speaking bindings consume Language-owned identities");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("semantic_types.rs"),
        format!(
            "pub const IPA_CONSTRUCTOR_SOURCE_ID: &str = {:?};\n{}",
            semantic.source_document_id.as_str(),
            bindings.source
        ),
    )
    .unwrap();
    let path = "voice.conduit";
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-changed=build_support/lower.rs");
    println!("cargo:rerun-if-changed=build_support/graph.rs");
    println!("cargo:rerun-if-changed=pronunciation.conduit");
    println!("cargo:rerun-if-changed=inflection.conduit");
    println!("cargo:rerun-if-changed=trajectory.conduit");
    println!("cargo:rerun-if-changed=connection.conduit");
    println!("cargo:rerun-if-changed=prosody.conduit");
    println!("cargo:rerun-if-changed=onset.conduit");
    println!("cargo:rerun-if-changed=normalization.conduit");
    println!("cargo:rerun-if-changed=glottal.conduit");
    let source = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("profile_phones.conduit"),
        include_str!("rule_status.conduit"),
        include_str!("selection.conduit"),
        fs::read_to_string(path).expect("native speech source"),
        include_str!("pronunciation.conduit"),
        include_str!("inflection.conduit"),
        include_str!("trajectory.conduit"),
        include_str!("connection.conduit"),
        include_str!("prosody.conduit"),
        include_str!("onset.conduit"),
        include_str!("normalization.conduit"),
        include_str!("glottal.conduit"),
        include_str!("timing_projection.conduit"),
        include_str!("duration_projection.conduit"),
        include_str!("duration_render.conduit"),
        include_str!("control_projection.conduit"),
        include_str!("context_match.conduit")
    );
    let source = format!("{}\n{}", source, include_str!("pitch_projection.conduit"));
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked =
        check_syntax_document(&syntax, &StartupCatalog::new()).expect("speech plots check");
    let mut generated = String::from("// Generated from exact checked speech plots.\n");
    generated.push_str(&format!(
        "pub const SOURCE_ID: &str = {:?};\n",
        checked.source_document_id.as_str()
    ));
    let mut types = Vec::new();
    for native in &checked.native_types {
        if let StructuredInfoTypeShape::Record { .. } | StructuredInfoTypeShape::Variant { .. } =
            native.value_type.shape()
        {
            types.push((native.value_type.clone(), native.name.clone()));
        }
    }
    for native in &checked.native_types {
        if let StructuredInfoTypeShape::Record { fields, .. } = native.value_type.shape() {
            let fields = fields
                .iter()
                .map(|field| {
                    format!(
                        "pub {}: {}",
                        field.name().replace('-', "_"),
                        lower::ty(field.value_type(), &types).expect("record field lowering")
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            generated.push_str(&format!(
                "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct {} {{ {fields} }}\n",
                native.name
            ));
        }
        if let StructuredInfoTypeShape::Variant { cases, .. } = native.value_type.shape() {
            generated.push_str(&format!("#[allow(non_camel_case_types)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n#[repr(u8)]\npub enum {} {{ {} }}\n", native.name, cases.iter().map(|case| if matches!(case.payload_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::EMPTY_INFO_ID) { format!("r#{}", case.tag()) } else { format!("r#{}({})", case.tag(), lower::ty(case.payload_type(), &types).expect("variant payload lowering")) }).collect::<Vec<_>>().join(",")));
        }
    }
    let phones = checked
        .native_types
        .iter()
        .find(|native| native.name == "EnglishPhone")
        .unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = phones.value_type.shape() else {
        panic!("EnglishPhone")
    };
    generated.push_str(&format!(
        "#[cfg(test)] pub const PHONES: &[(EnglishPhone, &str)] = &[{}];\n",
        cases
            .iter()
            .map(|case| format!("(EnglishPhone::r#{}, {:?})", case.tag(), case.tag()))
            .collect::<Vec<_>>()
            .join(",")
    ));
    let target = checked
        .native_types
        .iter()
        .find(|native| native.name == "SpeechAcousticTarget")
        .unwrap();
    for (name, constant) in [
        ("EnglishPhoneme", "PHONEMES"),
        ("EnglishStress", "STRESSES"),
        ("EnglishPosition", "POSITIONS"),
        ("EnglishDerivation", "DERIVATIONS"),
    ] {
        let native = checked
            .native_types
            .iter()
            .find(|native| native.name == name)
            .unwrap();
        let StructuredInfoTypeShape::Variant { cases, .. } = native.value_type.shape() else {
            panic!("unit variant")
        };
        generated.push_str(&format!(
            "#[cfg(test)] pub const {constant}: &[({name}, &str)] = &[{}];\n",
            cases
                .iter()
                .map(|case| format!("({name}::r#{}, {:?})", case.tag(), case.tag()))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    let StructuredInfoTypeShape::Record { fields, .. } = target.value_type.shape() else {
        panic!("target")
    };
    generated.push_str(&format!("#[cfg(test)] pub fn target_fields(value: SpeechAcousticTarget) -> [(&'static str, i32); {}] {{ [{}] }}\n", fields.len(), fields.iter().map(|field| format!("({:?}, value.{})", field.name(), field.name())).collect::<Vec<_>>().join(",")));
    // Two generated Rust carriers for the same exact native variant. This is
    // representation conversion, with no phonology, symbol inference or policy.
    generated.push_str("#[cfg(feature = \"semantic-bindings\")] pub fn compact_profile_phone(value: &crate::semantic::EnglishPhone) -> EnglishPhone { match value {\n");
    for case in cases {
        generated.push_str(&format!(
            "crate::semantic::EnglishPhone::{} => EnglishPhone::r#{},\n",
            case.tag()
                .split('_')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<String>(),
            case.tag()
        ));
    }
    generated.push_str("} }\n");
    let mut programs = Vec::new();
    let mut graphs = Vec::new();
    let mut symbols = std::collections::BTreeSet::new();
    for plot in &checked.plots {
        let authored =
            expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
                .unwrap_or_else(|error| panic!("speech plot {} expands: {error:?}", plot.name));
        let name = graph::symbol(&plot.name);
        assert!(
            symbols.insert(name.clone()),
            "checked plot names collide as Rust symbols"
        );
        let lowered = graph::function(&name, &authored, &types).expect("fixed pure graph lowering");
        if plot.name == "speech/profile" {
            let program = PortableExpressionProgram::from_canonical_hex(&lowered.programs[0].1)
                .expect("profile program");
            generated.push_str(&format!(
                "pub const RENDER_PROFILE: SpeechRenderProfile = {};\n",
                lower::constant(&program, &types).expect("literal voice profile")
            ));
        }
        programs.extend(lowered.programs);
        graphs.push((name.clone(), lowered.graph, lowered.result));
        generated.push_str(&format!(
            "pub const {}_ID: &str = {:?};\n",
            name.to_uppercase(),
            plot.checked_plot_id.as_str()
        ));
        generated.push_str(&format!(
            "pub const {}_EXPANDED_ID: &str = {:?};\n",
            name.to_uppercase(),
            authored.expanded.expanded_plot_id.as_str()
        ));
        generated.push_str(&lowered.source);
    }
    generated.push_str(&format!(
        "#[cfg(test)] pub const PROGRAMS: &[(&str, &str)] = &{:?};\n",
        programs
    ));
    generated.push_str("#[cfg(test)] pub struct CompiledGraphProof { pub name: &'static str, pub steps: &'static [(usize, &'static str)], pub result: usize }\n");
    generated.push_str(&format!(
        "#[cfg(test)] pub const GRAPHS: &[CompiledGraphProof] = &[{}];\n",
        graphs
            .iter()
            .map(|(name, graph, result)| format!(
                "CompiledGraphProof {{ name: {name:?}, steps: &{:?}, result: {result} }}",
                graph
            ))
            .collect::<Vec<_>>()
            .join(",")
    ));
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("voice.rs"),
        generated,
    )
    .unwrap();
}
