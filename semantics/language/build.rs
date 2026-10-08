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
    println!("cargo:rerun-if-changed=parser.conduit");
    println!("cargo:rerun-if-changed=parser_window8.conduit");
    println!("cargo:rerun-if-changed=parser_window8_search.conduit");
    println!("cargo:rerun-if-changed=parser_window8_facts.conduit");
    println!("cargo:rerun-if-changed=parser_window8_lexical_selection.conduit");
    println!("cargo:rerun-if-changed=parser_beam.conduit");
    println!("cargo:rerun-if-changed=parser_scorer.conduit");
    println!("cargo:rerun-if-changed=parser_mask.conduit");
    println!("cargo:rerun-if-changed=parser_joint.conduit");
    println!("cargo:rerun-if-changed=parser_joint_decode.conduit");
    println!("cargo:rerun-if-changed=parser_available.conduit");
    println!("cargo:rerun-if-changed=parser_revision.conduit");
    println!("cargo:rerun-if-changed=parser_session.conduit");
    println!("cargo:rerun-if-changed=parser_session_policy.conduit");
    println!("cargo:rerun-if-changed=parser_session_facts.conduit");
    println!("cargo:rerun-if-changed=parser_session_dependency.conduit");
    println!("cargo:rerun-if-changed=parser_session_committed_dependency.conduit");
    println!("cargo:rerun-if-changed=parser_session_commit.conduit");
    println!("cargo:rerun-if-changed=parser_session_rebase.conduit");
    println!("cargo:rerun-if-changed=parser_session_branch.conduit");
    println!("cargo:rerun-if-changed=parser_session_protection.conduit");
    println!("cargo:rerun-if-changed=parser_session_protected_set.conduit");
    println!("cargo:rerun-if-changed=parser_session_independent_admission.conduit");
    println!("cargo:rerun-if-changed=parser_session_protected_rebase.conduit");
    println!("cargo:rerun-if-changed=parser_session_protected_forest.conduit");
    println!("cargo:rerun-if-changed=parser_session_protected_branch.conduit");
    println!("cargo:rerun-if-changed=parser_session_protected_mask.conduit");
    println!("cargo:rerun-if-changed=parser_scorer_v2.conduit");
    println!("cargo:rerun-if-changed=discourse.conduit");
    println!("cargo:rerun-if-changed=prosody.conduit");
    println!("cargo:rerun-if-changed=pronunciation_selection.conduit");
    let source = [
        include_str!("types.conduit"),
        include_str!("identity.conduit"),
        include_str!("coverage.conduit"),
        include_str!("syntax.conduit"),
        include_str!("text_revision.conduit"),
        include_str!("revision_lineage.conduit"),
        include_str!("lexical.conduit"),
        include_str!("lexical_proposer.conduit"),
        include_str!("parser.conduit"),
        include_str!("parser_window8.conduit"),
        include_str!("parser_window8_search.conduit"),
        include_str!("parser_window8_facts.conduit"),
        include_str!("parser_window8_lexical_selection.conduit"),
        include_str!("parser_beam.conduit"),
        include_str!("parser_scorer.conduit"),
        include_str!("parser_mask.conduit"),
        include_str!("parser_joint.conduit"),
        include_str!("parser_joint_decode.conduit"),
        include_str!("parser_available.conduit"),
        include_str!("parser_revision.conduit"),
        include_str!("parser_scorer_v2.conduit"),
        include_str!("parser_session.conduit"),
        include_str!("parser_session_policy.conduit"),
        include_str!("parser_session_facts.conduit"),
        include_str!("parser_session_dependency.conduit"),
        include_str!("parser_session_committed_dependency.conduit"),
        include_str!("parser_session_commit.conduit"),
        include_str!("parser_session_rebase.conduit"),
        include_str!("parser_session_branch.conduit"),
        include_str!("parser_session_protection.conduit"),
        include_str!("parser_session_protected_set.conduit"),
        include_str!("parser_session_independent_admission.conduit"),
        include_str!("parser_session_protected_rebase.conduit"),
        include_str!("parser_session_protected_forest.conduit"),
        include_str!("parser_session_protected_branch.conduit"),
        include_str!("parser_session_protected_mask.conduit"),
        include_str!("discourse.conduit"),
        include_str!("prosody.conduit"),
        include_str!("pronunciation_selection.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    for (plot, file) in [
        ("language-window8-initialize", "window8_initialize.hex"),
        (
            "language-window8-stable-lexical-candidate",
            "window8_stable_lexical_candidate.hex",
        ),
        (
            "language-window8-stable-lexical-fact",
            "window8_stable_lexical_fact.hex",
        ),
        (
            "language-window8-walk-initialize",
            "window8_walk_initialize.hex",
        ),
        ("language-window8-walk-follow", "window8_walk_follow.hex"),
        ("language-window8-root-count", "window8_root_count.hex"),
        ("language-window8-move-context", "window8_move_context.hex"),
        (
            "language-window8-move-legal-shift",
            "window8_move_legal_shift.hex",
        ),
        (
            "language-window8-move-legal-reduce",
            "window8_move_legal_reduce.hex",
        ),
        (
            "language-window8-move-legal-left",
            "window8_move_legal_left.hex",
        ),
        (
            "language-window8-move-legal-right-root",
            "window8_move_legal_right_root.hex",
        ),
        (
            "language-window8-move-legal-right-nonroot",
            "window8_move_legal_right_nonroot.hex",
        ),
        ("language-window8-move-apply", "window8_move_apply.hex"),
        (
            "language-lexical-token-proposal",
            "lexical_token_proposal.hex",
        ),
        ("language-window8-complete", "window8_complete.hex"),
        ("language-window8-rank-insert", "window8_rank_insert.hex"),
        (
            "language-window8-choice-frontier",
            "window8_choice_frontier.hex",
        ),
        (
            "language-window8-score-advance",
            "window8_score_advance.hex",
        ),
        ("language-window8-rank-0-1", "window8_rank_0_1.hex"),
        ("language-window8-rank-2-3", "window8_rank_2_3.hex"),
        ("language-window8-rank-0-2", "window8_rank_0_2.hex"),
        ("language-window8-rank-1-3", "window8_rank_1_3.hex"),
        ("language-window8-rank-1-2", "window8_rank_1_2.hex"),
        ("language-window8-class-index", "window8_class_index.hex"),
        (
            "language-window8-class-context",
            "window8_class_context.hex",
        ),
        (
            "language-window8-class-relations",
            "window8_class_relations.hex",
        ),
        (
            "language-window8-class-relation",
            "window8_class_relation.hex",
        ),
        ("language-window8-available", "window8_available.hex"),
        ("language-window8-empty-codes", "window8_empty_codes.hex"),
        ("language-window8-token-codes", "window8_token_codes.hex"),
        (
            "language-window8-feature-context",
            "window8_feature_context.hex",
        ),
        (
            "language-window8-feature-values",
            "window8_feature_values.hex",
        ),
        ("language/vocative-discourse", "discourse_program.hex"),
        ("language/vocative-prosody", "rich_prosody_program.hex"),
        ("language/fallback-prosody", "fallback_prosody_program.hex"),
        (
            "language/pronunciation-pos",
            "pronunciation_pos_program.hex",
        ),
        (
            "language/pronunciation-candidate",
            "pronunciation_candidate_program.hex",
        ),
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            plot,
            &conduit_plot::ProfileCatalog::new(),
        )
        .unwrap_or_else(|failure| panic!("checked {plot} expands: {failure:?}"));
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
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            prepared_family_roots: [
                "LanguageLexicalTokenProposal".into(),
                "LanguageLexicalProposedTape".into(),
                "LanguageParserWindow8StableLexicalFact".into(),
                "LanguageParserWindow8RawState".into(),
                "LanguageParserWindow8RawWalk".into(),
                "LanguageParserWindow8RootCount".into(),
                "LanguageParserWindow8RawClassIndex".into(),
                "LanguageParserWindow8RawClassRelations".into(),
                "LanguageParserWindow8RawClass".into(),
                "LanguageParserWindow8RawBeam".into(),
                "LanguageParserWindow8RawContext".into(),
                "LanguageParserWindow8RawResult".into(),
                "LanguageParserWindow8Completion".into(),
                "LanguageParserWindow8Selected".into(),
                "LanguageParserWindow8RawHypothesis".into(),
                "LanguageParserWindow8RawFeatureContext".into(),
                "LanguageParserWindow8RawModelFeatures".into(),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("language semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    let source = retain_binding_bytes(&generated.source, output.parent().unwrap());
    fs::write(output, source).expect("write generated language bindings");
}

// Keep exact generated Type/program bytes without making rustc parse millions
// of integer-literal AST nodes. Native reconstruction and all laws are unchanged.
fn retain_binding_bytes(source: &str, directory: &std::path::Path) -> String {
    let mut retained = String::with_capacity(source.len());
    let mut index = 0;
    for line in source.lines() {
        if (line.starts_with("pub const ") && line.contains("_SEMANTIC_TYPE: &[u8]"))
            || line.contains("PortableExpressionProgram::from_canonical_bytes(&[")
            // Prepared families and ordinary constructors share these exact
            // ordered encoded law resources; retain each byte array once.
            || line.trim_start().starts_with("&[")
        {
            let start = line.find("&[").expect("generated literal bytes");
            // Constants have an earlier slice Type spelling; their literal starts
            // after the assignment. Invariant programs have only the literal.
            let start = if line.starts_with("pub const ") {
                line.find("= &[").unwrap() + 2
            } else {
                start
            };
            let end = start + line[start..].find(']').unwrap();
            let bytes: Vec<u8> = line[start + 2..end]
                .split(',')
                .filter(|value| !value.trim().is_empty())
                .map(|value| {
                    let value = value.trim();
                    if let Some(hex) = value.strip_prefix("0x") {
                        u8::from_str_radix(hex, 16).expect("generated byte")
                    } else {
                        value.parse().expect("generated byte")
                    }
                })
                .collect();
            let filename = format!("native_binding_bytes_{index}.bin");
            fs::write(directory.join(&filename), &bytes).expect("retain exact binding bytes");
            assert_eq!(fs::read(directory.join(&filename)).unwrap(), bytes);
            retained.push_str(&line[..start]);
            retained.push_str(&format!(
                "include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{filename}\"))"
            ));
            retained.push_str(&line[end + 1..]);
            index += 1;
        } else {
            retained.push_str(line);
        }
        retained.push('\n');
    }
    assert!(index > 0, "generated native byte resources retained");
    retained
}
