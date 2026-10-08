#[path = "build_session_chain.rs"]
mod session_chain;
use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build_session_chain.rs");
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
    println!("cargo:rerun-if-changed=parser_session_seed.conduit");
    println!("cargo:rerun-if-changed=parser_session_completion.conduit");
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
    println!("cargo:rerun-if-changed=parser_session_custody.conduit");
    println!("cargo:rerun-if-changed=parser_session_custody_origins.conduit");
    println!("cargo:rerun-if-changed=parser_session_custody_frontier.conduit");
    println!("cargo:rerun-if-changed=parser_session_custody_initialize.conduit");
    println!("cargo:rerun-if-changed=parser_session_independent_commit.conduit");
    println!("cargo:rerun-if-changed=parser_session_independent_commit_rebase.conduit");
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
        include_str!("parser.conduit"),
        include_str!("parser_window8.conduit"),
        include_str!("parser_window8_search.conduit"),
        include_str!("parser_window8_facts.conduit"),
        include_str!("parser_window8_lexical_selection.conduit"),
        include_str!("parser_beam.conduit"),
        include_str!("parser_scorer.conduit"),
        include_str!("parser_mask.conduit"),
        include_str!("parser_joint.conduit"),
        include_str!("parser_session_seed.conduit"),
        include_str!("parser_session_completion.conduit"),
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
        include_str!("parser_session_custody.conduit"),
        include_str!("parser_session_custody_origins.conduit"),
        include_str!("parser_session_custody_frontier.conduit"),
        include_str!("parser_session_custody_initialize.conduit"),
        include_str!("parser_session_independent_commit.conduit"),
        include_str!("parser_session_independent_commit_rebase.conduit"),
        include_str!("discourse.conduit"),
        include_str!("prosody.conduit"),
        include_str!("pronunciation_selection.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    for (plot, file) in [
        ("language-parser-availability", "parser_availability.hex"),
        (
            "language-parser-decode-complete",
            "parser_decode_complete.hex",
        ),
        (
            "language-parser-independent-branch",
            "parser_independent_branch.hex",
        ),
        (
            "language-parser-independent-commit",
            "parser_independent_commit.hex",
        ),
        (
            "language-parser-independent-commit-rebase",
            "parser_independent_commit_rebase.hex",
        ),
        (
            "language-parser-independent-commit-rebase-sets",
            "parser_independent_commit_rebase_sets.hex",
        ),
        (
            "language-parser-independent-mask",
            "parser_independent_mask.hex",
        ),
        ("language-parser-joint-branch", "parser_joint_branch.hex"),
        ("language-parser-joint-commit", "parser_joint_commit.hex"),
        (
            "language-parser-joint-consensus",
            "parser_joint_consensus.hex",
        ),
        (
            "language-parser-joint-expansion",
            "parser_joint_expansion.hex",
        ),
        ("language-parser-joint-rebase", "parser_joint_rebase.hex"),
        (
            "language-parser-joint-runtime-merge",
            "parser_joint_runtime_merge.hex",
        ),
        (
            "language-parser-joint-score-band-1000",
            "parser_joint_score_band_1000.hex",
        ),
        (
            "language-parser-joint-stable-fact",
            "parser_joint_stable_fact.hex",
        ),
        ("language-parser-legal-mask", "parser_legal_mask.hex"),
        (
            "language-parser-protected-origin-edge",
            "parser_protected_origin_edge.hex",
        ),
        (
            "language-parser-protected-set-initialize",
            "parser_protected_set_initialize.hex",
        ),
        (
            "language-parser-protected-set-insert",
            "parser_protected_set_insert.hex",
        ),
        (
            "language-parser-protected-set-rebase",
            "parser_protected_set_rebase.hex",
        ),
        (
            "language-parser-protection-forest-projection",
            "parser_protection_forest_projection.hex",
        ),
        (
            "language-parser-retained-commit-anchor",
            "parser_retained_commit_anchor.hex",
        ),
        (
            "language-parser-revision-reset",
            "parser_revision_reset.hex",
        ),
        (
            "language-parser-score-proposal",
            "parser_score_proposal.hex",
        ),
        ("language-parser-session-seed", "parser_session_seed.hex"),
        (
            "language-parser-session-complete",
            "parser_session_complete.hex",
        ),
        ("language-parser-transition", "parser_transition.hex"),
        (
            "language-parser-v2-model-features",
            "parser_v2_model_features.hex",
        ),
        ("language-parser-v2-pos", "parser_v2_pos.hex"),
        ("language-parser-wait-state", "parser_wait_state.hex"),
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            plot,
            &conduit_plot::ProfileCatalog::new(),
        )
        .expect("fixed Source chain expands");
        session_chain::retain(
            &expanded,
            &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(file),
        );
    }
    for (plot, file) in [
        (
            "language-parser-protected-set-initialize",
            "parser_protected_set_initialize.hex",
        ),
        (
            "language-parser-independent-commit",
            "parser_independent_commit.hex",
        ),
        (
            "language-parser-independent-commit-rebase-sets",
            "parser_independent_commit_rebase_sets.hex",
        ),
        (
            "language-parser-independent-commit-rebase",
            "parser_independent_commit_rebase.hex",
        ),
        (
            "language-parser-retained-commit-anchor",
            "parser_retained_commit_anchor.hex",
        ),
        (
            "language-parser-protected-origin-edge",
            "parser_protected_origin_edge.hex",
        ),
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
                "LanguageAnalysisTokenRef",
                "LanguageDependencyArc",
                "LanguageDependencyHead",
                "LanguageDependencyRelation",
                "LanguageParserAvailability",
                "LanguageParserAvailableLexical",
                "LanguageParserAvailableState",
                "LanguageParserHypothesis",
                "LanguageParserIndependentBranchContext",
                "LanguageParserIndependentCommitRebaseRequest",
                "LanguageParserIndependentCommitRebaseSets",
                "LanguageParserIndependentCommitRequest",
                "LanguageParserIndependentCommitSet",
                "LanguageParserIndependentCommitSetProposal",
                "LanguageParserIndependentMaskQuery",
                "LanguageParserIndependentProtectedAdmission",
                "LanguageParserJointBeam",
                "LanguageParserJointBranchQuery",
                "LanguageParserJointBranchResult",
                "LanguageParserJointCommitProposal",
                "LanguageParserJointCommitQuery",
                "LanguageParserJointConsensusObservation",
                "LanguageParserJointConsensusQuery",
                "LanguageParserJointExpansion",
                "LanguageParserJointHypothesis",
                "LanguageParserJointProtectedBranchQuery",
                "LanguageParserJointRebaseContext",
                "LanguageParserJointRebaseProposal",
                "LanguageParserJointRuntimeHypothesis",
                "LanguageParserJointRuntimeMerge",
                "LanguageParserJointRuntimeRawBeam",
                "LanguageParserJointRuntimeRawHypothesis",
                "LanguageParserJointScoreBandProposal",
                "LanguageParserJointScoreBandQuery",
                "LanguageParserJointStableFactProposal",
                "LanguageParserLegalMask",
                "LanguageParserMaskQuery",
                "LanguageParserProtectedEdgeProposal",
                "LanguageParserProtectedInitialReceipt",
                "LanguageParserProtectedInitializationReceipt",
                "LanguageParserProtectedInsertContext",
                "LanguageParserProtectedInsertReceipt",
                "LanguageParserProtectedOriginCorrelation",
                "LanguageParserProtectedRebaseReceipt",
                "LanguageParserProtectedSetProposal",
                "LanguageParserProtectedSetRebaseContext",
                "LanguageParserProtectionForestProposal",
                "LanguageParserProtectionForestQuery",
                "LanguageParserRawAvailability",
                "LanguageParserRawJointHypothesis",
                "LanguageParserRawWaitState",
                "LanguageParserRequest",
                "LanguageParserResult",
                "LanguageParserRetainedCommitAnchor",
                "LanguageParserRetainedCommitAnchorProposal",
                "LanguageParserRetainedCommitReceipt",
                "LanguageParserRetainedFactReceipt",
                "LanguageParserRetainedSnapshotReceipt",
                "LanguageParserRevisionContext",
                "LanguageParserRevisionResult",
                "LanguageParserScoredClass",
                "LanguageParserScoredProposal",
                "LanguageParserSessionSeedProposal",
                "LanguageParserCompletionObservation",
                "LanguageParserProtectedHypothesisCompatibility",
                "LanguageParserSessionSeedRequest",
                "LanguageParserStableDependencyAdmission",
                "LanguageParserState",
                "LanguageParserV2ChoiceQuery",
                "LanguageParserV2ModelFeatures",
                "LanguageParserV2PosContext",
                "LanguageParserWaitState",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
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
            // Preserve each shared ordered law array as exact binary bytes.
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
