use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=identity.conduit");
    println!("cargo:rerun-if-changed=coverage.conduit");
    println!("cargo:rerun-if-changed=syntax.conduit");
    println!("cargo:rerun-if-changed=text_revision.conduit");
    println!("cargo:rerun-if-changed=revision_lineage.conduit");
    println!("cargo:rerun-if-changed=lexical.conduit");
    println!("cargo:rerun-if-changed=pronunciation_selection.conduit");
    println!("cargo:rerun-if-changed=lexical_consumer.conduit");
    println!("cargo:rerun-if-changed=parser.conduit");
    println!("cargo:rerun-if-changed=parser_available.conduit");
    println!("cargo:rerun-if-changed=parser_revision_rebase.conduit");
    println!("cargo:rerun-if-changed=parser_beam_commit.conduit");
    println!("cargo:rerun-if-changed=parser_beam.conduit");
    println!("cargo:rerun-if-changed=discourse.conduit");
    println!("cargo:rerun-if-changed=prosody.conduit");
    for file in [
        "parser_scorer_features.conduit",
        "parser_scorer_actions.conduit",
        "parser_scorer_search.conduit",
        "parser_scorer_ranking.conduit",
        "parser_scorer_tournament.conduit",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    let source = [
        include_str!("types.conduit"),
        include_str!("identity.conduit"),
        include_str!("coverage.conduit"),
        include_str!("syntax.conduit"),
        include_str!("text_revision.conduit"),
        include_str!("revision_lineage.conduit"),
        include_str!("lexical.conduit"),
        include_str!("pronunciation_selection.conduit"),
        include_str!("lexical_consumer.conduit"),
        include_str!("parser.conduit"),
        include_str!("parser_beam.conduit"),
        include_str!("parser_available.conduit"),
        include_str!("parser_revision_rebase.conduit"),
        include_str!("parser_beam_commit.conduit"),
        include_str!("parser_scorer_features.conduit"),
        include_str!("parser_scorer_search.conduit"),
        include_str!("parser_scorer_actions.conduit"),
        include_str!("parser_scorer_ranking.conduit"),
        include_str!("parser_scorer_tournament.conduit"),
        include_str!("discourse.conduit"),
        include_str!("prosody.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    let commitment = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "language-parser-beam-commit",
        &conduit_plot::ProfileCatalog::new(),
    )
    .expect("commitment expands");
    assert_eq!(commitment.expanded.gears.len(), 8);
    assert_eq!(commitment.expanded.connections.len(), 7);
    assert_eq!(commitment.input_bindings.len(), 1);
    assert_eq!(commitment.output_bindings.len(), 1);
    let mut id = commitment.input_bindings[0].gear_id.clone();
    let mut ordered = Vec::new();
    let mut seen = Vec::new();
    loop {
        assert!(!seen.contains(&id), "commitment preparation is acyclic");
        seen.push(id.clone());
        let gear = commitment
            .expanded
            .gears
            .iter()
            .find(|g| g.gear_id == id)
            .unwrap();
        assert!(gear.resource_ports.is_empty());
        assert_eq!(gear.configuration.len(), 1);
        let conduit_core::ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
            panic!("commitment expression")
        };
        ordered.push(hex.clone());
        let outgoing: Vec<_> = commitment
            .expanded
            .connections
            .iter()
            .filter(|e| e.source_gear_id == id)
            .collect();
        if outgoing.is_empty() {
            assert_eq!(id, commitment.output_bindings[0].gear_id);
            break;
        }
        assert_eq!(outgoing.len(), 1);
        id = outgoing[0].sink_gear_id.clone();
    }
    assert_eq!(ordered.len(), 8);
    fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("commitment_programs.rs"), format!("pub const COMMITMENT_SOURCE_ID: &str = {:?};\npub(crate) const COMMITMENT_PROGRAMS: [&str; 8] = {:?};\n", checked.source_document_id.as_str(), ordered)).unwrap();
    for (plot, file) in [
        (
            "language-parser-availability",
            "parser_availability_program.hex",
        ),
        ("language/vocative-discourse", "discourse_program.hex"),
        (
            "language/pronunciation-pos",
            "pronunciation_pos_program.hex",
        ),
        (
            "language/pronunciation-candidate",
            "pronunciation_candidate_program.hex",
        ),
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
