//! Synthetic structural fixtures only. No model inference or lexical evidence.
use super::{parser_fixture as fixture, parser_selection_io::SourceGraph};
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub struct Structural {
    parser: fixture::Fixture,
    merge: SourceGraph,
    agreement: SourceGraph,
    advance: SourceGraph,
    commit: SourceGraph,
}
impl Structural {
    pub fn new() -> Self {
        let source = super::parser_selection_io::language_source();
        eprintln!(
            "model-free structural checked Source digest: {:02x?}",
            conduit_core::semantic_digest(
                "proof/parser-structural-consensus/source@1",
                source.as_bytes()
            )
        );
        Self {
            parser: fixture::Fixture::new(),
            merge: SourceGraph::language("language-parser-scorer-merge"),
            agreement: SourceGraph::language("language-parser-admitted-agreement"),
            advance: SourceGraph::language("language-parser-advance-stable"),
            commit: SourceGraph::language("language-parser-commit"),
        }
    }
    /// Construct a complete chain by accepted Source actions, with one declared
    /// relation variant. Labels are structural alternatives, not lexical claims.
    pub fn tree(&mut self, count: u64, changed: usize, relation: &str) -> LanguageParserState {
        let mut state = self.parser.initial(count);
        for ordinal in 0..count as usize {
            let label = if ordinal == 0 {
                "root"
            } else if ordinal == changed {
                relation
            } else {
                "dep"
            };
            let result = self.parser.step(&state, "right_arc", label, "analysis/1");
            assert!(fixture::accepted(&result), "legal right arc at {ordinal}");
            state = fixture::field(&result, "state").clone();
        }
        for _ in 0..count {
            let result = self.parser.step(&state, "reduce", "dep", "analysis/1");
            assert!(fixture::accepted(&result), "legal reduction");
            state = fixture::field(&result, "state").clone();
        }
        let raw = LanguageParserNumericState::from_structured(state).unwrap();
        assert_eq!(*raw.unread(), count);
        assert_eq!(*raw.depth(), 1);
        assert!(raw.heads()[..count as usize].iter().all(|head| *head < 5));
        LanguageParserState::new(
            raw.basis().clone(),
            *raw.committed(),
            *raw.depth(),
            *raw.heads(),
            raw.relation0().clone(),
            raw.relation1().clone(),
            raw.relation2().clone(),
            raw.relation3().clone(),
            *raw.stack(),
            *raw.token_count(),
            *raw.unread(),
        )
        .unwrap()
    }
    pub fn hypothesis(
        state: &LanguageParserState,
        identity: u64,
        score: i64,
        active: bool,
    ) -> LanguageParserScorerHypothesis {
        LanguageParserScorerHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(active, identity, score, state.clone()).unwrap(),
            *state.token_count(),
        )
        .unwrap()
    }
    pub fn rank(
        &mut self,
        candidates: &[LanguageParserScorerHypothesis; 4],
        order: [usize; 4],
    ) -> LanguageParserScorerBeam {
        let inactive = Self::hypothesis(candidates[0].parser().state(), 99, -1_000_000, false);
        let mut beam = LanguageParserScorerBeam::new(
            inactive.clone(),
            inactive.clone(),
            inactive.clone(),
            inactive,
        )
        .unwrap();
        for index in order {
            beam = self.merge.run_native(
                LanguageParserScorerMerge::new(beam, candidates[index].clone()).unwrap(),
            );
        }
        beam
    }
    pub fn admitted(beam: &LanguageParserScorerBeam) -> LanguageParserAdmittedBeam {
        LanguageParserAdmittedBeam::new(
            beam.candidate0().parser().state().basis().clone(),
            beam.candidate0().parser().clone(),
            beam.candidate1().parser().clone(),
            beam.candidate2().parser().clone(),
            beam.candidate3().parser().clone(),
        )
        .unwrap()
    }
    pub fn agreement(
        &mut self,
        beam: &LanguageParserAdmittedBeam,
        dependent: u64,
    ) -> LanguageParserAgreement {
        self.agreement
            .run_native(LanguageParserAdmittedAgreementQuery::new(beam.clone(), dependent).unwrap())
    }
    pub fn frontier(
        &mut self,
        beam: &LanguageParserAdmittedBeam,
        count: u64,
    ) -> LanguageParserRawFrontier {
        let mut frontier = LanguageParserRawFrontier::new(beam.basis().clone(), 0, 0).unwrap();
        for dependent in 0..count {
            let agreement = self.agreement(beam, dependent);
            let result: LanguageParserFrontierResult = self.advance.run_native(
                LanguageParserAdvanceRequest::new(agreement, false, frontier.clone(), false)
                    .unwrap(),
            );
            if !*result.accepted() {
                break;
            }
            frontier = result.frontier().clone();
        }
        frontier
    }
    pub fn commit(
        &mut self,
        frontier: &LanguageParserRawFrontier,
        through: u64,
    ) -> LanguageParserFrontierResult {
        self.commit.run_native(
            LanguageParserCommitRequest::new(false, frontier.clone(), false, through).unwrap(),
        )
    }
}
