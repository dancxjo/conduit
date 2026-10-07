use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=identity.conduit");
    println!("cargo:rerun-if-changed=coverage.conduit");
    println!("cargo:rerun-if-changed=syntax.conduit");
    println!("cargo:rerun-if-changed=text_revision.conduit");
    println!("cargo:rerun-if-changed=lexical.conduit");
    println!("cargo:rerun-if-changed=parser.conduit");
    println!("cargo:rerun-if-changed=parser_beam.conduit");
    println!("cargo:rerun-if-changed=parser_scorer.conduit");
    println!("cargo:rerun-if-changed=discourse.conduit");
    println!("cargo:rerun-if-changed=prosody.conduit");
    let source = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("types.conduit"),
        include_str!("identity.conduit"),
        include_str!("coverage.conduit"),
        include_str!("syntax.conduit"),
        include_str!("text_revision.conduit"),
        include_str!("lexical.conduit"),
        include_str!("parser.conduit"),
        include_str!("parser_beam.conduit"),
        include_str!("parser_scorer.conduit"),
        include_str!("discourse.conduit"),
        include_str!("prosody.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    for (plot, file) in [
        ("language/vocative-discourse", "discourse_program.hex"),
        ("language/vocative-prosody", "rich_prosody_program.hex"),
        ("language/fallback-prosody", "fallback_prosody_program.hex"),
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            plot,
            &conduit_plot::ProfileCatalog::new(),
        )
        .expect("checked language Plot expands");
        assert_eq!(
            expanded.expanded.gears.len(),
            1,
            "one finite pure derivation"
        );
        let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
            panic!("one exact expression program")
        };
        assert_eq!(entry.key, "program");
        let conduit_core::ConfigurationValue::Text(program) = &entry.value else {
            panic!("encoded expression program")
        };
        fs::write(
            PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(file),
            program,
        )
        .expect("retain checked language program");
    }
    let generated = generate_rust_bindings(&checked.native_types, &RustBindingOptions::default())
        .expect("language semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated language bindings");
}
