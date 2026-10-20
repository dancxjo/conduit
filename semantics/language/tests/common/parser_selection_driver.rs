//! Finite test orchestration. Source owns branches, rankings and transitions;
//! this driver transports admitted frames and preserves all refused proposals.
#![allow(dead_code)]
use super::parser_model_kernel::ModelDriver;
use super::parser_selection_io::{language_source, SourceGraph};
use super::parser_selection_values::{admit_hyp, admit_state, dep};
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduitos::protocol_source::ProtocolSourcePackage;

pub struct Decoder {
    initialize: SourceGraph,
    features: SourceGraph,
    proposal: SourceGraph,
    transition: SourceGraph,
    branch: SourceGraph,
    selected: SourceGraph,
    merge: SourceGraph,
    ranking: SourceGraph,
    legality: SourceGraph,
    pub agreement: SourceGraph,
    pub advance: SourceGraph,
    pub commit: SourceGraph,
    next: SourceGraph,
    fallback: SourceGraph,
    complete: SourceGraph,
    pub rebase: SourceGraph,
    pub model: ModelDriver,
    pub illegal: u64,
    pub refused_expansions: u64,
    identity: u64,
}
pub struct Decoded {
    pub beam: LanguageParserScorerBeam,
    pub complete: bool,
    pub steps: u64,
    pub elapsed_ns: u128,
    pub invocations: u64,
}
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
    fn id(&mut self) -> u64 {
        self.identity += 1;
        self.identity
    }
    pub fn initial(
        &mut self,
        lexical: &LanguageParserAvailableLexical,
        analysis: &str,
    ) -> LanguageParserState {
        let basis = LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(analysis.into()).unwrap(),
            lexical.tape().source().material().revision().clone(),
            lexical.tape().source().material().identity().clone(),
        )
        .unwrap();
        let begin = LanguageParserBegin::new(basis, dep(), *lexical.token_count()).unwrap();
        admit_state(self.initialize.run_native(begin))
    }
    fn is_complete(&mut self, state: &LanguageParserState) -> bool {
        self.complete.run_bool(state.clone())
    }

    fn numeric(state: &LanguageParserState) -> LanguageParserNumericState {
        LanguageParserNumericState::new(
            state.basis().clone(),
            *state.committed(),
            *state.depth(),
            *state.heads(),
            state.relation0().clone(),
            state.relation1().clone(),
            state.relation2().clone(),
            state.relation3().clone(),
            *state.stack(),
            *state.token_count(),
            *state.unread(),
        )
        .unwrap()
    }

    fn propose(
        &mut self,
        state: &LanguageParserState,
        class: u64,
        score: i64,
    ) -> LanguageParserScoredProposal {
        self.proposal.run_native(
            LanguageParserScoredClass::new(
                state.basis().clone(),
                class,
                dep(),
                score,
                Self::numeric(state),
            )
            .unwrap(),
        )
    }
    pub fn illegal_scored_fixture(&mut self, state: &LanguageParserState) -> serde_json::Value {
        // Adversarial score fixture, explicitly not a learned inference receipt.
        let proposal = self.propose(state, 2, 128525);
        let result: LanguageParserResult = self.transition.run_native(proposal.request().clone());
        assert!(!*result.accepted());
        assert_eq!(result.state(), &Self::numeric(state));
        serde_json::json!({"mode":"adversarial numerical proposal fixture","class":2,"score":128525,"accepted":result.accepted(),"refusal":format!("{:?}",result.refusal()),"state_preserved":true})
    }
    fn insert(
        &mut self,
        beam: LanguageParserScorerBeam,
        candidate: LanguageParserScorerHypothesis,
    ) -> LanguageParserScorerBeam {
        self.merge
            .run_native(LanguageParserScorerMerge::new(beam, candidate).unwrap())
    }
    fn slots(beam: &LanguageParserScorerBeam) -> [LanguageParserScorerHypothesis; 4] {
        [
            beam.candidate0().clone(),
            beam.candidate1().clone(),
            beam.candidate2().clone(),
            beam.candidate3().clone(),
        ]
    }
    /// Rebase all active hypotheses, preserve committed lexical choices, reset
    /// scores, and coalesce only exact admitted state+choice duplicates. Source
    /// merge owns ordering/admission of every distinct retained seed.
    pub fn rebase_fresh_revision(
        &mut self,
        previous: &LanguageParserScorerBeam,
        previous_lexical: &lexical::PreparedLexicalTape,
        source: &LanguageTextRevision,
        analysis: &LanguageAnalysisRevisionId,
    ) -> (
        lexical::PreparedLexicalTape,
        LanguageParserScorerBeam,
        usize,
    ) {
        let lexical = lexical::prepare_lexical_tape(
            source,
            previous_lexical.tape().profile(),
            Some(previous_lexical),
        )
        .unwrap();
        let mut retained = Vec::new();
        let mut coalesced = 0;
        for prior in Self::slots(previous) {
            if !*prior.parser().active() {
                continue;
            }
            let rebase = PreparedParserRevisionRebase::prepare(
                prior.parser().state(),
                previous_lexical,
                source,
                analysis.clone(),
                dep(),
            )
            .unwrap();
            assert_eq!(rebase.lexical().tape(), lexical.tape());
            let raw = self.rebase.run_native(rebase.request().clone());
            let state = admit_state(raw);
            let mut choices = [0; 4];
            let committed = *state.committed() as usize;
            choices[..committed].copy_from_slice(&prior.choices()[..committed]);
            retained.push(
                LanguageParserScorerHypothesis::new(
                    choices,
                    LanguageParserHypothesis::new(
                        true,
                        *prior.parser().identity(),
                        0,
                        state.clone(),
                    )
                    .unwrap(),
                    *state.unread(),
                )
                .unwrap(),
            );
        }
        let first = retained.first().expect("active retained beam");
        let inactive = LanguageParserScorerHypothesis::new(
            *first.choices(),
            LanguageParserHypothesis::new(false, self.id(), 0, first.parser().state().clone())
                .unwrap(),
            *first.selected(),
        )
        .unwrap();
        let mut beam = LanguageParserScorerBeam::new(
            inactive.clone(),
            inactive.clone(),
            inactive.clone(),
            inactive.clone(),
        )
        .unwrap();
        for candidate in retained {
            beam = self.insert(beam, candidate);
        }
        // Source ranks all at most four score-zero seeds before exact value
        // coalescing, so the retained duplicate identity follows Source ties.
        let mut unique = Vec::new();
        for candidate in Self::slots(&beam) {
            if !*candidate.parser().active() {
                continue;
            }
            if unique.iter().any(|prior: &LanguageParserScorerHypothesis| {
                prior.parser().state() == candidate.parser().state()
                    && prior.choices() == candidate.choices()
            }) {
                coalesced += 1;
            } else {
                unique.push(candidate);
            }
        }
        let mut beam = LanguageParserScorerBeam::new(
            inactive.clone(),
            inactive.clone(),
            inactive.clone(),
            inactive,
        )
        .unwrap();
        for candidate in unique {
            beam = self.insert(beam, candidate);
        }
        (lexical, beam, coalesced)
    }
    pub fn decode(
        &mut self,
        lexical: &LanguageParserAvailableLexical,
        initial: LanguageParserState,
        learned: bool,
    ) -> Decoded {
        let inactive = LanguageParserScorerHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(false, self.id(), 0, initial.clone()).unwrap(),
            *initial.unread(),
        )
        .unwrap();
        let seed = LanguageParserScorerHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(true, self.id(), 0, initial).unwrap(),
            *inactive.selected(),
        )
        .unwrap();
        let beam = LanguageParserScorerBeam::new(
            seed,
            inactive.clone(),
            inactive.clone(),
            inactive.clone(),
        )
        .unwrap();
        self.decode_seeded(lexical, beam, learned, inactive)
    }
    /// Explicit fresh-revision profile. Scores describe only this revision's
    /// continuations; callers rebase every survivor before entering here.
    pub fn decode_from_beam(
        &mut self,
        lexical: &LanguageParserAvailableLexical,
        beam: LanguageParserScorerBeam,
        learned: bool,
    ) -> Decoded {
        let inactive = LanguageParserScorerHypothesis::new(
            *beam.candidate0().choices(),
            LanguageParserHypothesis::new(
                false,
                self.id(),
                0,
                beam.candidate0().parser().state().clone(),
            )
            .unwrap(),
            *beam.candidate0().selected(),
        )
        .unwrap();
        self.decode_seeded(lexical, beam, learned, inactive)
    }
    fn decode_seeded(
        &mut self,
        lexical: &LanguageParserAvailableLexical,
        mut beam: LanguageParserScorerBeam,
        learned: bool,
        inactive: LanguageParserScorerHypothesis,
    ) -> Decoded {
        let start = std::time::Instant::now();
        let before = self.model.invocations();
        let mut steps = 0;
        // Each accepted arc-eager action reads or pops a token: at most2n.
        for _ in 0..8 {
            let mut next_beam = LanguageParserScorerBeam::new(
                inactive.clone(),
                inactive.clone(),
                inactive.clone(),
                inactive.clone(),
            )
            .unwrap();
            let mut expanded = false;
            for prior in Self::slots(&beam) {
                if !*prior.parser().active() {
                    continue;
                }
                if self.is_complete(prior.parser().state()) {
                    next_beam = self.insert(next_beam, prior);
                    continue;
                }
                expanded = true;
                let mut representatives = Vec::with_capacity(5);
                for class in [0, 1, 2, 39, 73] {
                    let proposal = self.propose(prior.parser().state(), class, 0);
                    let result: LanguageParserResult =
                        self.transition.run_native(proposal.request().clone());
                    if !*result.accepted() {
                        self.illegal += 1;
                    }
                    representatives.push(result);
                }
                let mask: LanguageParserScorerRanking = self.legality.run_native(
                    LanguageParserScorerLegality::new(
                        prior.parser().state().basis().clone(),
                        representatives[2].clone(),
                        representatives[1].clone(),
                        representatives[3].clone(),
                        representatives[4].clone(),
                        representatives[0].clone(),
                    )
                    .unwrap(),
                );
                let allowed = *mask.allowed();
                let choices = if prior.parser().state().unread() < lexical.token_count() {
                    4
                } else {
                    1
                };
                for choice in 0..choices {
                    let branch = if prior.parser().state().unread() < lexical.token_count() {
                        let result: LanguageParserScorerBranchResult = self.branch.run_native(
                            LanguageParserScorerBranch::new(choice, lexical.clone(), prior.clone())
                                .unwrap(),
                        );
                        if !*result.accepted() {
                            continue;
                        }
                        admit_hyp(result.candidate().clone())
                    } else {
                        prior.clone()
                    };
                    let query = LanguageParserScorerChoiceQuery::new(
                        *branch.choices(),
                        *lexical.tape().source().sequence(),
                        self.model.invocations() + 1,
                        lexical.clone(),
                        branch.parser().state().clone(),
                    )
                    .unwrap();
                    let features: LanguageParserScorerFeatures = self.features.run_native(query);
                    assert_eq!(features.basis(), branch.parser().state().basis());
                    assert_eq!(*features.epoch(), *lexical.tape().source().sequence());
                    let scores = if learned {
                        let scores = self.model.score(*features.indices());
                        assert_eq!(
                            *features.invocation(),
                            self.model.last_receipt().unwrap().invocation
                        );
                        scores
                    } else {
                        [0; 76]
                    };
                    let mut rank =
                        LanguageParserScorerRanking::new(allowed, 0, 0, false, 0, scores).unwrap();
                    if !learned {
                        rank = self.fallback.run_native(rank);
                    }
                    // At most4 descendants of one branch can survive beam4.
                    for _ in 0..4 {
                        rank = self.ranking.run_native(rank);
                        if !*rank.found() {
                            break;
                        }
                        let scored = LanguageParserScoredClass::new(
                            branch.parser().state().basis().clone(),
                            *rank.class(),
                            dep(),
                            *rank.score(),
                            Self::numeric(branch.parser().state()),
                        )
                        .unwrap();
                        let id = self.id();
                        let raw: LanguageParserScorerRawHypothesis = self.selected.run_native(
                            LanguageParserScorerSelected::new(id, branch.clone(), scored).unwrap(),
                        );
                        if !*raw.parser().active() {
                            self.refused_expansions += 1;
                            rank = self.next.run_native(rank);
                            continue;
                        }
                        let candidate = admit_hyp(raw);
                        next_beam = self.insert(next_beam, candidate);
                        rank = self.next.run_native(rank);
                    }
                }
            }
            if !expanded {
                break;
            }
            beam = next_beam;
            steps += 1;
        }
        let complete = *beam.candidate0().parser().active()
            && self.is_complete(beam.candidate0().parser().state());
        Decoded {
            beam,
            complete,
            steps,
            elapsed_ns: start.elapsed().as_nanos(),
            invocations: self.model.invocations() - before,
        }
    }
}
