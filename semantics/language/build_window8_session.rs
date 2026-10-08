//! Fixed complete Source selection for the proposal-v2 Window8 Session.
//! This build-time selection grants no runtime snapshot or commit authority.
pub const SOURCE_ADDITIONS: &[(&str, &str)] = &[
    (
        "parser_window8_continuation.conduit",
        include_str!("parser_window8_continuation.conduit"),
    ),
    (
        "parser_window8_dependency_custody.conduit",
        include_str!("parser_window8_dependency_custody.conduit"),
    ),
    (
        "parser_window8_dependency_facts.conduit",
        include_str!("parser_window8_dependency_facts.conduit"),
    ),
    (
        "parser_window8_dependency_forest.conduit",
        include_str!("parser_window8_dependency_forest.conduit"),
    ),
    (
        "parser_window8_empty_grow.conduit",
        include_str!("parser_window8_empty_grow.conduit"),
    ),
    (
        "parser_window8_independent_commit.conduit",
        include_str!("parser_window8_independent_commit.conduit"),
    ),
    (
        "parser_window8_lexical_protection.conduit",
        include_str!("parser_window8_lexical_protection.conduit"),
    ),
    (
        "parser_window8_proposal_numeric_custody.conduit",
        include_str!("parser_window8_proposal_numeric_custody.conduit"),
    ),
    (
        "parser_window8_qualified_choice_protection.conduit",
        include_str!("parser_window8_qualified_choice_protection.conduit"),
    ),
    (
        "parser_window8_qualified_dependency.conduit",
        include_str!("parser_window8_qualified_dependency.conduit"),
    ),
    (
        "parser_window8_qualified_dependency_rebase.conduit",
        include_str!("parser_window8_qualified_dependency_rebase.conduit"),
    ),
    (
        "parser_window8_qualified_independent_commit.conduit",
        include_str!("parser_window8_qualified_independent_commit.conduit"),
    ),
    (
        "parser_window8_qualified_lexical.conduit",
        include_str!("parser_window8_qualified_lexical.conduit"),
    ),
    (
        "parser_window8_qualified_lexical_rebase.conduit",
        include_str!("parser_window8_qualified_lexical_rebase.conduit"),
    ),
    (
        "parser_window8_reanalysis.conduit",
        include_str!("parser_window8_reanalysis.conduit"),
    ),
    (
        "parser_window8_session_seed.conduit",
        include_str!("parser_window8_session_seed.conduit"),
    ),
];
pub const ROOTS: &[&str] = &[
    "LanguageParserProposalWindow8FeatureContext",
    "LanguageParserProposalWindow8OriginQuery",
    "LanguageParserProposalWindow8V2Features",
    "LanguageParserProposalWindow8V2ModelScores",
    "LanguageParserWindow8Available",
    "LanguageParserWindow8Begin",
    "LanguageParserWindow8ChoiceQuery",
    "LanguageParserWindow8ClassQuery",
    "LanguageParserWindow8CodeQuery",
    "LanguageParserWindow8Completion",
    "LanguageParserWindow8DependencyCompatibilityDecision",
    "LanguageParserWindow8EmptyGrowContext",
    "LanguageParserWindow8EmptySeedProposal",
    "LanguageParserWindow8EmptySeedRequest",
    "LanguageParserWindow8GrowContext",
    "LanguageParserWindow8GrowProposal",
    "LanguageParserWindow8IndependentCommitInitialize",
    "LanguageParserWindow8IndependentCommitRebase",
    "LanguageParserWindow8IndependentCommitSetProposal",
    "LanguageParserWindow8Ordinal",
    "LanguageParserWindow8ProtectedChoiceDecision",
    "LanguageParserWindow8QualifiedChoiceBranch",
    "LanguageParserWindow8QualifiedDependencyProposal",
    "LanguageParserWindow8QualifiedDependencyRebase",
    "LanguageParserWindow8QualifiedIndependentCommitRequest",
    "LanguageParserWindow8QualifiedLexicalAnalysis",
    "LanguageParserWindow8QualifiedLexicalAnchorContext",
    "LanguageParserWindow8QualifiedLexicalProposal",
    "LanguageParserWindow8QualifiedLexicalRebase",
    "LanguageParserWindow8RawAdvance",
    "LanguageParserWindow8RawClassContext",
    "LanguageParserWindow8RawClassRelations",
    "LanguageParserWindow8RawFeatureContext",
    "LanguageParserWindow8RawMerge",
    "LanguageParserWindow8RawModelFeatures",
    "LanguageParserWindow8RawRequest",
    "LanguageParserWindow8RawWalk",
    "LanguageParserWindow8ReanalysisContext",
    "LanguageParserWindow8ReanalysisProposal",
    "LanguageParserWindow8RootCount",
    "LanguageParserWindow8Selected",
    "LanguageParserWindow8SessionSeedContext",
    "LanguageParserWindow8WaitProposal",
];
pub const PORTS: &[(&str, &str)] = &[
    (
        "language-proposal-window8-feature-context",
        "window8_session_00.hex",
    ),
    (
        "language-proposal-window8-origins",
        "window8_session_01.hex",
    ),
    (
        "language-proposal-window8-v2-feature-indices",
        "window8_session_02.hex",
    ),
    (
        "language-proposal-window8-v2-feature-values",
        "window8_session_03.hex",
    ),
    (
        "language-proposal-window8-v2-score-observation",
        "window8_session_04.hex",
    ),
    ("language-window8-available", "window8_session_05.hex"),
    ("language-window8-choice-frontier", "window8_session_06.hex"),
    ("language-window8-class-context", "window8_session_07.hex"),
    ("language-window8-class-index", "window8_session_08.hex"),
    ("language-window8-class-relation", "window8_session_09.hex"),
    ("language-window8-class-relations", "window8_session_10.hex"),
    ("language-window8-complete", "window8_session_11.hex"),
    (
        "language-window8-dependency-compatible",
        "window8_session_12.hex",
    ),
    ("language-window8-empty-codes", "window8_session_13.hex"),
    ("language-window8-empty-grow", "window8_session_14.hex"),
    ("language-window8-feature-context", "window8_session_15.hex"),
    ("language-window8-feature-values", "window8_session_16.hex"),
    ("language-window8-grow", "window8_session_17.hex"),
    (
        "language-window8-independent-commit-initialize",
        "window8_session_18.hex",
    ),
    (
        "language-window8-independent-commit-rebase",
        "window8_session_19.hex",
    ),
    ("language-window8-initialize", "window8_session_20.hex"),
    ("language-window8-move-apply", "window8_session_21.hex"),
    ("language-window8-move-context", "window8_session_22.hex"),
    ("language-window8-move-legal-left", "window8_session_23.hex"),
    (
        "language-window8-move-legal-reduce",
        "window8_session_24.hex",
    ),
    (
        "language-window8-move-legal-right-nonroot",
        "window8_session_25.hex",
    ),
    (
        "language-window8-move-legal-right-root",
        "window8_session_26.hex",
    ),
    (
        "language-window8-move-legal-shift",
        "window8_session_27.hex",
    ),
    (
        "language-window8-qualified-dependency-analysis",
        "window8_session_28.hex",
    ),
    (
        "language-window8-qualified-dependency-anchor",
        "window8_session_29.hex",
    ),
    (
        "language-window8-qualified-dependency-rebase",
        "window8_session_30.hex",
    ),
    (
        "language-window8-qualified-independent-commit",
        "window8_session_31.hex",
    ),
    (
        "language-window8-qualified-lexical-anchor",
        "window8_session_32.hex",
    ),
    (
        "language-window8-qualified-lexical-anchor-scalars",
        "window8_session_33.hex",
    ),
    (
        "language-window8-qualified-lexical-origin",
        "window8_session_34.hex",
    ),
    (
        "language-window8-qualified-lexical-proposal",
        "window8_session_35.hex",
    ),
    (
        "language-window8-qualified-lexical-rebase-origin",
        "window8_session_36.hex",
    ),
    (
        "language-window8-qualified-lexical-rebase-scalars",
        "window8_session_37.hex",
    ),
    (
        "language-window8-qualified-lexical-rebase-token",
        "window8_session_38.hex",
    ),
    (
        "language-window8-qualified-lexical-token",
        "window8_session_39.hex",
    ),
    (
        "language-window8-qualified-protected-choice",
        "window8_session_40.hex",
    ),
    ("language-window8-rank-0-1", "window8_session_41.hex"),
    ("language-window8-rank-0-2", "window8_session_42.hex"),
    ("language-window8-rank-1-2", "window8_session_43.hex"),
    ("language-window8-rank-1-3", "window8_session_44.hex"),
    ("language-window8-rank-2-3", "window8_session_45.hex"),
    ("language-window8-rank-insert", "window8_session_46.hex"),
    ("language-window8-reanalysis", "window8_session_47.hex"),
    ("language-window8-root-count", "window8_session_48.hex"),
    ("language-window8-score-advance", "window8_session_49.hex"),
    (
        "language-window8-session-empty-seed",
        "window8_session_50.hex",
    ),
    ("language-window8-session-seed", "window8_session_51.hex"),
    ("language-window8-token-codes", "window8_session_52.hex"),
    ("language-window8-wait", "window8_session_53.hex"),
    ("language-window8-walk-follow", "window8_session_54.hex"),
    ("language-window8-walk-initialize", "window8_session_55.hex"),
];
