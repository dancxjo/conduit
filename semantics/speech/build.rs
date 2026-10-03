#[path = "build_support/graph.rs"]
mod graph;
#[path = "build_support/lower.rs"]
mod lower;
use conduit_core::StructuredInfoTypeShape;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=listening.conduit");
    println!("cargo:rerun-if-changed=translation.conduit");
    let semantic_source = format!(
        "{}\n{}\n{}",
        include_str!("types.conduit"),
        include_str!("listening.conduit"),
        include_str!("translation.conduit")
    );
    let semantic = check_syntax_document(
        &parse_syntax_document(&semantic_source),
        &StartupCatalog::new(),
    )
    .expect("Speaking segment and listening contracts check");
    let bindings = conduit_plot::rust_binding::generate_rust_bindings(
        &semantic.native_types,
        &conduit_plot::rust_binding::RustBindingOptions::default(),
    )
    .expect("Speaking native bindings");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("semantic_types.rs"),
        bindings.source,
    )
    .unwrap();
    let path = "voice.conduit";
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rerun-if-changed=build_support/lower.rs");
    println!("cargo:rerun-if-changed=build_support/graph.rs");
    println!("cargo:rerun-if-changed=pronunciation.conduit");
    println!("cargo:rerun-if-changed=trajectory.conduit");
    let source = format!(
        "{}\n{}\n{}",
        fs::read_to_string(path).expect("native speech source"),
        include_str!("pronunciation.conduit"),
        include_str!("trajectory.conduit")
    );
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
            generated.push_str(&format!("#[allow(non_camel_case_types)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n#[repr(u8)]\npub enum {} {{ {} }}\n", native.name, cases.iter().map(|case| if matches!(case.payload_type().shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == conduit_core::UNIT_INFO_ID) { format!("r#{}", case.tag()) } else { format!("r#{}({})", case.tag(), lower::ty(case.payload_type(), &types).expect("variant payload lowering")) }).collect::<Vec<_>>().join(",")));
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
    generated.push_str(&format!("#[cfg(test)] pub fn target_fields(value: SpeechAcousticTarget) -> [(&'static str, i64); {}] {{ [{}] }}\n", fields.len(), fields.iter().map(|field| format!("({:?}, value.{})", field.name(), field.name())).collect::<Vec<_>>().join(",")));
    let mut programs = Vec::new();
    let mut graphs = Vec::new();
    for plot in &checked.plots {
        let authored =
            expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
                .unwrap_or_else(|error| panic!("speech plot {} expands: {error:?}", plot.name));
        let name = plot.name.replace(['/', '-'], "_");
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
