//! Immutable Source Plan and explicit numerical model preparation for the test decoder.
use super::*;

impl Decoder {
    pub fn new() -> Self {
        Self::with_model(ModelDriver::new())
    }
    pub fn with_model(model: ModelDriver) -> Self {
        use conduitos::protocol_source::{ProtocolSpecializationRequest, ProtocolValueReference};
        let transition_source = parser_scorer_transition_source();
        let (imports, plots): (Vec<_>, Vec<_>) = transition_source
            .lines()
            .partition(|line| line.starts_with("with "));
        let package = ProtocolSourcePackage::compile(
            format!(
                "{}\n{}\n{}",
                imports.join("\n"),
                language_source(),
                plots.join("\n")
            ),
            &[ProtocolSpecializationRequest::Zip {
                left: ProtocolValueReference {
                    type_name: "LanguageParserScorerSelected".into(),
                    maximum_bytes: 16384,
                },
                right: ProtocolValueReference {
                    type_name: "LanguageParserResult".into(),
                    maximum_bytes: 16384,
                },
            }],
        )
        .unwrap();
        let artifact = conduit_core::ArtifactId::from(format!(
            "language/parser-proof/package/{}",
            conduit_core::semantic_digest(
                "language/parser-proof/package@1",
                &serde_json::to_vec(&package).unwrap()
            )
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
        ));
        let source = conduitos::protocol_source::PreparedProtocolSource::prepare(package).unwrap();
        // Validate the packaged compound first, before preparing the independent
        // entry Plans, so authoring errors preserve a short diagnostic path.
        source
            .expand("language-parser-scorer-selected-transition")
            .unwrap();
        let graph = |name| SourceGraph::from_source(&source, name, &artifact);
        let mut ranking = graph("language-parser-scorer-rank");
        let mut scan = graph("language-parser-scorer-ranking-chunk");
        // Preserve exact old/new Source output parity before using the faster
        // tournament. Inactive large scores must not outrank negative legal ones.
        for case in 0..3 {
            let mut allowed = [false; 76];
            let mut scores = [i64::MAX; 76];
            if case != 0 {
                allowed[2] = true;
                allowed[75] = true;
                scores[2] = -5;
                scores[75] = if case == 1 { -8 } else { -5 };
            }
            let query = LanguageParserScorerRanking::new(allowed, 0, 0, false, 0, scores).unwrap();
            let mut old = query.clone();
            for _ in 0..5 {
                old = scan.run_native(old);
            }
            let new: LanguageParserScorerRanking = ranking.run_native(query);
            assert_eq!(
                old.encode().unwrap(),
                new.encode().unwrap(),
                "ranking parity case {case}"
            );
        }
        Self {
            initialize: graph("language-parser-initialize"),
            features: graph("language-parser-scorer-model-features"),
            proposal: graph("language-parser-score-proposal"),
            transition: graph("language-parser-transition"),
            branch: graph("language-parser-scorer-branch"),
            selected: graph("language-parser-scorer-selected-transition"),
            merge: graph("language-parser-scorer-merge"),
            ranking,
            legality: graph("language-parser-scorer-legality"),
            agreement: graph("language-parser-admitted-agreement"),
            advance: graph("language-parser-advance-stable"),
            commit: graph("language-parser-beam-commit"),
            next: graph("language-parser-scorer-ranking-next"),
            fallback: graph("language-parser-scorer-fallback"),
            complete: graph("language-parser-decode-complete"),
            rebase: graph("language-parser-revision-rebase"),
            model,
            illegal: 0,
            refused_expansions: 0,
            identity: 0,
        }
    }
}
