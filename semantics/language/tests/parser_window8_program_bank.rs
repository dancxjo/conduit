//! Decode immutable Source programs once; every value still uses native laws.
use conduit_language::{parser_window8::*, *};
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};

fn program(encoded: &str) -> PortableExpressionProgram {
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn run<I: NativeRustBinding, O: NativeRustBinding>(
    program: &PortableExpressionProgram,
    input: I,
) -> O {
    O::decode(&program.evaluate(&input.encode().unwrap()).unwrap()).unwrap()
}
macro_rules! source {
    ($name:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/", $name, ".hex"))
    };
}
struct Bank {
    rank: [PortableExpressionProgram; 5],
    insert: PortableExpressionProgram,
    walk_initialize: PortableExpressionProgram,
    walk_follow: PortableExpressionProgram,
    roots: PortableExpressionProgram,
    context: PortableExpressionProgram,
    class_context: PortableExpressionProgram,
    guards: [PortableExpressionProgram; 5],
    apply: PortableExpressionProgram,
}
impl Bank {
    fn new() -> Self {
        Self {
            rank: [
                program(source!("window8_rank_0_1")),
                program(source!("window8_rank_2_3")),
                program(source!("window8_rank_0_2")),
                program(source!("window8_rank_1_3")),
                program(source!("window8_rank_1_2")),
            ],
            insert: program(source!("window8_rank_insert")),
            walk_initialize: program(source!("window8_walk_initialize")),
            walk_follow: program(source!("window8_walk_follow")),
            roots: program(source!("window8_root_count")),
            context: program(source!("window8_move_context")),
            class_context: program(source!("window8_class_context")),
            guards: [
                program(source!("window8_move_legal_shift")),
                program(source!("window8_move_legal_reduce")),
                program(source!("window8_move_legal_left")),
                program(source!("window8_move_legal_right_root")),
                program(source!("window8_move_legal_right_nonroot")),
            ],
            apply: program(source!("window8_move_apply")),
        }
    }
    fn rank(&self, mut beam: LanguageParserWindow8RawBeam) -> LanguageParserWindow8RawBeam {
        for program in &self.rank {
            beam = run(program, beam);
        }
        beam
    }
    fn merge(
        &self,
        beam: LanguageParserWindow8RawBeam,
        hyp: LanguageParserWindow8RawHypothesis,
    ) -> LanguageParserWindow8RawBeam {
        self.rank(run(
            &self.insert,
            LanguageParserWindow8RawMerge::new(self.rank(beam), hyp).unwrap(),
        ))
    }
    fn proof(
        &self,
        state: &LanguageParserWindow8RawState,
    ) -> Result<LanguageParserWindow8StateProof, conduit_plot::rust_binding::NativeBindingRefusal>
    {
        let mut ancestry = Vec::new();
        for start in 0..8 {
            let mut walk: LanguageParserWindow8RawWalk = run(
                &self.walk_initialize,
                LanguageParserWindow8WalkQuery::new(*state.heads(), start)?,
            );
            for _ in 0..8 {
                walk = run(&self.walk_follow, walk);
            }
            ancestry.push(LanguageParserWindow8Ancestry::new(
                *walk.query().heads(),
                *walk.path(),
                start,
            )?);
        }
        let roots: LanguageParserWindow8RootCount = run(&self.roots, state.clone());
        LanguageParserWindow8StateProof::new(
            ancestry.try_into().unwrap(),
            *roots.count(),
            state.clone(),
        )
    }
    fn context(
        &self,
        prior: &PreparedWindow8State,
        basis: &LanguageParserBasis,
    ) -> LanguageParserWindow8RawContext {
        let top = prior.state().stack()[(*prior.state().depth() - 1) as usize];
        let witness = prior.proof().ancestry()[if top < 8 { top as usize } else { 0 }].clone();
        run(
            &self.context,
            LanguageParserWindow8RawRequest::new(
                LanguageParserAction::RightArc,
                basis.clone(),
                prior.state().relation0().clone(),
                prior.proof().clone(),
                witness,
            )
            .unwrap(),
        )
    }
    fn propose(
        &self,
        seed: &LanguageParserWindow8RawContext,
        class: &LanguageParserWindow8RawClass,
    ) -> LanguageParserWindow8RawResult {
        let mut context: LanguageParserWindow8RawContext = run(
            &self.class_context,
            LanguageParserWindow8RawClassContext::new(class.clone(), seed.clone()).unwrap(),
        );
        for guard in &self.guards {
            context = run(guard, context);
        }
        run(&self.apply, context)
    }
}
fn initial() -> PreparedWindow8State {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/bank-analysis".into()).unwrap(),
        LanguageTextRevisionId::new("window8/bank-r0".into()).unwrap(),
        LanguageTextId::new("window8/bank".into()).unwrap(),
    )
    .unwrap();
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    initialize_window8(&LanguageParserWindow8Begin::new(basis, relation, 1).unwrap()).unwrap()
}
#[test]
fn retained_allocating_program_bank_matches_public_rank_merge_and_full_forest() {
    let bank = Bank::new();
    let prior = initial();
    assert_eq!(&bank.proof(prior.state()).unwrap(), prior.proof());
    let hyp = |identity, score, active| {
        LanguageParserWindow8RawHypothesis::new(
            active,
            [identity; 8],
            identity,
            score,
            0,
            prior.state().clone(),
        )
        .unwrap()
    };
    let beam = LanguageParserWindow8RawBeam::new(
        hyp(9, 20, true),
        hyp(3, 20, true),
        hyp(1, 100, false),
        hyp(7, -30, true),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let reference = window8_merge(beam.clone(), hyp(11, 30, true)).unwrap();
    let reference_ns = started.elapsed().as_nanos();
    let started = std::time::Instant::now();
    let retained = bank.merge(beam.clone(), hyp(11, 30, true));
    let retained_ns = started.elapsed().as_nanos();
    assert_eq!(reference, retained);
    assert_eq!(window8_rank(beam.clone()).unwrap(), bank.rank(beam));
    let root = window8_class(73, prior.state().relation0()).unwrap();
    let next = prepare_window8_step(
        &prior,
        prior.state().basis(),
        *root.action(),
        root.relation(),
    )
    .unwrap();
    assert_eq!(
        &bank.proof(next.next().state()).unwrap(),
        next.next().proof()
    );
    let s = prior.state();
    let mut heads = *s.heads();
    heads[0] = 0;
    let cyclic = LanguageParserWindow8RawState::new(
        s.basis().clone(),
        *s.committed(),
        *s.depth(),
        heads,
        s.relation0().clone(),
        s.relation1().clone(),
        s.relation2().clone(),
        s.relation3().clone(),
        s.relation4().clone(),
        s.relation5().clone(),
        s.relation6().clone(),
        s.relation7().clone(),
        *s.stack(),
        *s.token_count(),
        *s.unread(),
    )
    .unwrap();
    assert!(prepare_window8_state(&cyclic).is_err());
    assert!(bank.proof(&cyclic).is_err());
    eprintln!("exact allocating merge reference_ns={reference_ns} retained_program_ns={retained_ns}; one fixture, excludes bank preparation");
}
#[test]
fn retained_allocating_class_context_and_guards_match_all76_reference_proposals() {
    let bank = Bank::new();
    let prior = initial();
    let current = prepare_window8_context(&prior, prior.state().basis()).unwrap();
    let seed = bank.context(&prior, prior.state().basis());
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/foreign-bank-analysis".into()).unwrap(),
        prior.state().basis().source_revision().clone(),
        prior.state().basis().text().clone(),
    )
    .unwrap();
    let stale = prepare_window8_context(&prior, &foreign).unwrap();
    let stale_seed = bank.context(&prior, &foreign);
    for code in 0..76 {
        let class = window8_class(code, prior.state().relation0()).unwrap();
        assert_eq!(
            current.propose(&class).unwrap().proposal(),
            &bank.propose(&seed, &class)
        );
        let reference = stale.propose(&class).unwrap();
        let cached = bank.propose(&stale_seed, &class);
        assert_eq!(reference.proposal(), &cached);
        assert!(!cached.accepted());
    }
}
