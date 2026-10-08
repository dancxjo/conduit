//! Decode immutable Source programs once; every value still uses native laws.
extern crate alloc;
use conduit_language::parser_window8::lexical;
use conduit_language::parser_window8_program_bank as owned_bank;
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
    for owned in [
        owned_bank::Window8ProgramBank::prepare().unwrap(),
        owned_bank::Window8ProgramBank::prepare_native(native_limits()).unwrap(),
    ] {
        let bank = Bank::new();

        let prior = initial();
        let owned_prior = owned.admit_state(prior.state()).unwrap();
        assert_eq!(owned_prior.proof(), prior.proof());
        let begin = LanguageParserWindow8Begin::new(
            prior.state().basis().clone(),
            prior.state().relation0().clone(),
            1,
        )
        .unwrap();
        assert_eq!(owned.initialize(&begin).unwrap().proof(), prior.proof());
        assert_eq!(
            owned.complete(&owned_prior).unwrap(),
            window8_complete(&prior).unwrap()
        );
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
        assert_eq!(
            reference,
            owned.merge(beam.clone(), hyp(11, 30, true)).unwrap()
        );
        assert_eq!(owned.rank(beam.clone()).unwrap(), bank.rank(beam.clone()));
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
        assert!(owned.admit_state(&cyclic).is_err());
        eprintln!("exact allocating merge reference_ns={reference_ns} retained_program_ns={retained_ns}; one fixture, excludes bank preparation");
    }
}
#[test]
fn retained_allocating_class_context_and_guards_match_all76_reference_proposals() {
    let bank = Bank::new();
    let prior = initial();
    let owned = owned_bank::Window8ProgramBank::prepare().unwrap();
    let owned_prior = owned.admit_state(prior.state()).unwrap();
    let owned_context = owned.context(&owned_prior, prior.state().basis()).unwrap();
    assert_eq!(owned_context.prior().proof(), prior.proof());
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
    let owned_stale = owned.context(&owned_prior, &foreign).unwrap();
    for code in 0..76 {
        let class = window8_class(code, prior.state().relation0()).unwrap();
        assert_eq!(class, owned.class(code, prior.state().relation0()).unwrap());
        let owned_proposal = owned_context.propose(&class).unwrap();
        assert_eq!(owned_proposal.prior().proof(), prior.proof());
        assert_eq!(
            current.propose(&class).unwrap().proposal(),
            owned_proposal.proposal()
        );
        assert_eq!(
            current.propose(&class).unwrap().proposal(),
            &bank.propose(&seed, &class)
        );
        let reference = stale.propose(&class).unwrap();
        let cached = bank.propose(&stale_seed, &class);
        assert_eq!(reference.proposal(), &cached);
        assert!(!cached.accepted());
        assert_eq!(&cached, owned_stale.propose(&class).unwrap().proposal());
    }
    assert!(owned.class(76, prior.state().relation0()).is_err());
}

#[test]
fn owned_bank_features_choices_and_scores_retain_exact_native_custody() {
    for owned in [
        owned_bank::Window8ProgramBank::prepare().unwrap(),
        owned_bank::Window8ProgramBank::prepare_native(native_limits()).unwrap(),
    ] {
        use conduit_plot::rust_binding::BoundedSequence;

        let prior = initial();
        let state = owned.admit_state(prior.state()).unwrap();
        let provenance = LinguisticDerivationProvenance::deterministic_rule(
            "window8/bank-fixture".into(),
            "profile/3".into(),
        )
        .unwrap();
        let revision = LanguageTextRevision::new(
            LanguageTextFinality::Partial,
            LanguageText::new(
                prior.state().basis().text().clone(),
                LanguageId::new("language/en".into()).unwrap(),
                prior.state().basis().source_revision().clone(),
                "record ".into(),
            )
            .unwrap(),
            None,
            provenance.clone(),
            0,
            Some(6),
        )
        .unwrap();
        let candidate = LanguageLexicalCandidate::new(
            "record".into(),
            BoundedSequence::new(),
            LanguageLexicalPos::Verb,
        )
        .unwrap();
        let entry = LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([candidate]).unwrap(),
            "record".into(),
        )
        .unwrap();
        let profile = LanguageLexicalProfile::new(
            BoundedSequence::try_from_iter([entry]).unwrap(),
            "window8/bank-profile".into(),
            LanguageId::new("language/en".into()).unwrap(),
            provenance,
        )
        .unwrap();
        let tape =
            conduit_language::lexical::prepare_lexical_tape(&revision, &profile, None).unwrap();
        let lexical = lexical::prepare_window8_lexical(&tape).unwrap();
        let features = owned
            .features(&state, &lexical, prior.state().basis(), [0; 8])
            .unwrap();
        let reference =
            lexical::prepare_window8_features(&prior, &lexical, prior.state().basis(), [0; 8])
                .unwrap();
        assert_eq!(features.state().proof(), prior.proof());
        assert_eq!(features.lexical(), lexical.lexical());
        assert_eq!(features.query(), reference.query());
        assert_eq!(features.features(), reference.features());
        let choice =
            LanguageParserWindow8ChoiceQuery::new(features.query().clone(), [0; 8], 0).unwrap();
        assert_eq!(
            owned.choice_frontier(choice.clone()).unwrap(),
            window8_choice_frontier(choice).unwrap()
        );
        let context = owned.context(&state, prior.state().basis()).unwrap();
        let root = owned.class(73, prior.state().relation0()).unwrap();
        let proposal = context.propose(&root).unwrap();
        let advance = LanguageParserWindow8RawAdvance::new(
            [0; 8],
            1,
            proposal.proposal().clone(),
            -12,
            1,
            30,
        )
        .unwrap();
        assert_eq!(
            owned.score_advance(advance.clone()).unwrap(),
            window8_score_advance(advance).unwrap()
        );
    }
}

fn native_limits() -> conduit_plot::rust_binding::PreparedNativeFamilyLimits {
    conduit_plot::rust_binding::PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 64,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_retained_bytes: 256 * 1024 * 1024,
        maximum_preparation_peak_bytes: 512 * 1024 * 1024,
        maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
    }
}

#[test]
fn prepared_native_bank_preserves_complete_output_and_refusal_semantics() {
    let reference = owned_bank::Window8ProgramBank::prepare().unwrap();
    let prepared = owned_bank::Window8ProgramBank::prepare_native(native_limits()).unwrap();
    assert!(reference.native_storage_receipt().is_none());
    let receipt = prepared.native_storage_receipt().unwrap();
    assert_eq!(receipt.types, 53);
    let prior = initial();
    let reference_state = reference.admit_state(prior.state()).unwrap();
    let prepared_state = prepared.admit_state(prior.state()).unwrap();
    assert_eq!(reference_state.proof(), prepared_state.proof());
    assert_eq!(
        reference.complete(&reference_state).unwrap(),
        prepared.complete(&prepared_state).unwrap()
    );
    let reference_context = reference
        .context(&reference_state, prior.state().basis())
        .unwrap();
    let prepared_context = prepared
        .context(&prepared_state, prior.state().basis())
        .unwrap();
    for code in [0, 2, 73] {
        let class = reference.class(code, prior.state().relation0()).unwrap();
        assert_eq!(
            class,
            prepared.class(code, prior.state().relation0()).unwrap()
        );
        assert_eq!(
            reference_context.propose(&class).unwrap().proposal(),
            prepared_context.propose(&class).unwrap().proposal()
        );
    }
    assert_eq!(
        reference
            .class(76, prior.state().relation0())
            .err()
            .map(|error| format!("{error:?}")),
        prepared
            .class(76, prior.state().relation0())
            .err()
            .map(|error| format!("{error:?}"))
    );
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("window8/prepared-foreign".into()).unwrap(),
        prior.state().basis().source_revision().clone(),
        prior.state().basis().text().clone(),
    )
    .unwrap();
    let reference_foreign = reference.context(&reference_state, &foreign).unwrap();
    let prepared_foreign = prepared.context(&prepared_state, &foreign).unwrap();
    let class = reference.class(0, prior.state().relation0()).unwrap();
    let refused = prepared_foreign.propose(&class).unwrap();
    assert!(!refused.proposal().accepted());
    assert_eq!(
        reference_foreign.propose(&class).unwrap().proposal(),
        refused.proposal()
    );
    assert_eq!(prepared.native_storage_receipt(), Some(receipt));
    assert!(owned_bank::Window8ProgramBank::prepare_native(
        conduit_plot::rust_binding::PreparedNativeFamilyLimits {
            maximum_retained_bytes: receipt.retained_heap_bytes_bound - 1,
            ..native_limits()
        }
    )
    .is_err());
}

/// Profile a retained context captured from an actual admitted lexical fact.
/// The input is external evidence; this never substitutes a fixture for a model
/// replay and does not execute numerical inference.
#[test]
#[ignore = "requires CONDUIT_WINDOW8_NATIVE_PROFILE_INPUT actual canonical lexical fact"]
fn actual_retained_context_all76_proposals_match_reference() {
    let path = std::env::var("CONDUIT_WINDOW8_NATIVE_PROFILE_INPUT").unwrap();
    let canonical = std::fs::read(path).unwrap();
    eprintln!(
        "actual profile input_bytes={} semantic_digest={:02x?}",
        canonical.len(),
        conduit_core::semantic_digest("language/window8-native-profile-input@1", &canonical)
    );
    let reference = owned_bank::Window8ProgramBank::prepare().unwrap();
    let start = std::time::Instant::now();
    let prepared = owned_bank::Window8ProgramBank::prepare_native(native_limits()).unwrap();
    let preparation_us = start.elapsed().as_micros();
    let fact = LanguageParserWindow8StableLexicalFact::decode(&canonical).unwrap();
    let snapshot = fact.query().snapshot();
    let raw = snapshot.candidate0().hypothesis().state();
    let reference_state = reference.admit_state(raw).unwrap();
    let prepared_state = prepared.admit_state(raw).unwrap();
    assert_eq!(reference_state.proof(), prepared_state.proof());
    let reference_context = reference
        .context(&reference_state, snapshot.basis())
        .unwrap();
    let prepared_context = prepared.context(&prepared_state, snapshot.basis()).unwrap();
    let classes = (0..76)
        .map(|code| reference.class(code, raw.relation0()).unwrap())
        .collect::<Vec<_>>();
    let start = std::time::Instant::now();
    let expected = classes
        .iter()
        .map(|class| reference_context.propose(class).unwrap())
        .collect::<Vec<_>>();
    let reference_us = start.elapsed().as_micros();
    eprintln!("actual76 Reference batch completed reference_us={reference_us}");
    let start = std::time::Instant::now();
    for (class, expected) in classes.iter().zip(&expected) {
        let observed = prepared_context.propose(class).unwrap();
        assert_eq!(observed.prior().proof(), expected.prior().proof());
        assert_eq!(observed.proposal(), expected.proposal());
    }
    let prepared_us = start.elapsed().as_micros();
    eprintln!(
        "actual retained context76proposal exact Native parity PASS preparation_us={preparation_us} reference_us={reference_us} prepared_us={prepared_us} native_storage={:?}; excludes context/class preparation and numerical model inference; Source evaluation and ordinary input encoding included",
        prepared.native_storage_receipt().unwrap()
    );
}

fn source_limits() -> owned_bank::Window8SourcePreparationLimits {
    owned_bank::Window8SourcePreparationLimits {
        maximum_retained_bytes: 1024 * 1024 * 1024,
        maximum_preparation_peak_bytes: 2 * 1024 * 1024 * 1024,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    }
}
#[test]
fn prepared_source_bank_preserves_refusals_and_exact_resource_ceilings() {
    let reference = owned_bank::Window8ProgramBank::prepare().unwrap();
    let prepared =
        owned_bank::Window8ProgramBank::prepare_native_evaluator(native_limits(), source_limits())
            .unwrap();
    assert!(reference.prepared_source_receipt().is_none());
    let receipt = prepared.prepared_source_receipt().unwrap();
    assert_eq!(receipt.programs, 26);
    assert_eq!(prepared.prepared_program_receipts().count(), 26);
    let prior = initial();
    let r = reference.admit_state(prior.state()).unwrap();
    let p = prepared.admit_state(prior.state()).unwrap();
    assert_eq!(r.proof(), p.proof());
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("prepared-source-foreign".into()).unwrap(),
        prior.state().basis().source_revision().clone(),
        prior.state().basis().text().clone(),
    )
    .unwrap();
    let rc = reference.context(&r, &foreign).unwrap();
    let pc = prepared.context(&p, &foreign).unwrap();
    let class = reference.class(0, prior.state().relation0()).unwrap();
    let expected = rc.propose(&class).unwrap();
    let actual = pc.propose(&class).unwrap();
    assert!(!actual.proposal().accepted());
    assert_eq!(actual.proposal(), expected.proposal());
    assert_eq!(
        format!("{:?}", reference.class(76, prior.state().relation0()).err()),
        format!("{:?}", prepared.class(76, prior.state().relation0()).err())
    );
    assert!(owned_bank::Window8ProgramBank::prepare_native_evaluator(
        native_limits(),
        owned_bank::Window8SourcePreparationLimits {
            maximum_retained_bytes: receipt.retained_heap_bytes_bound - 1,
            ..source_limits()
        }
    )
    .is_err());
    assert!(owned_bank::Window8ProgramBank::prepare_native_evaluator(
        native_limits(),
        owned_bank::Window8SourcePreparationLimits {
            maximum_preparation_peak_bytes: receipt.preparation_peak_heap_bytes_bound - 1,
            ..source_limits()
        }
    )
    .is_err());
    eprintln!("preparedSource exact resource/refusal parity receipt={receipt:?}");
}
#[test]
#[ignore = "requires actual canonical lexical-fact evidence"]
fn actual_retained_context_all76_prepared_source_proposals_match_reference() {
    let canonical =
        std::fs::read(std::env::var("CONDUIT_WINDOW8_NATIVE_PROFILE_INPUT").unwrap()).unwrap();
    let fact = LanguageParserWindow8StableLexicalFact::decode(&canonical).unwrap();
    let snapshot = fact.query().snapshot();
    let raw = snapshot.candidate0().hypothesis().state();
    let reference = owned_bank::Window8ProgramBank::prepare().unwrap();
    let start = std::time::Instant::now();
    let prepared =
        owned_bank::Window8ProgramBank::prepare_native_evaluator(native_limits(), source_limits())
            .unwrap();
    let preparation_us = start.elapsed().as_micros();
    let r = reference.admit_state(raw).unwrap();
    let p = prepared.admit_state(raw).unwrap();
    assert_eq!(r.proof(), p.proof());
    let rc = reference.context(&r, snapshot.basis()).unwrap();
    let pc = prepared.context(&p, snapshot.basis()).unwrap();
    let classes = (0..76)
        .map(|code| reference.class(code, raw.relation0()).unwrap())
        .collect::<Vec<_>>();
    let start = std::time::Instant::now();
    let expected = classes
        .iter()
        .map(|class| rc.propose(class).unwrap())
        .collect::<Vec<_>>();
    let reference_us = start.elapsed().as_micros();
    let start = std::time::Instant::now();
    for (class, expected) in classes.iter().zip(&expected) {
        let actual = pc.propose(class).unwrap();
        assert_eq!(actual.prior().proof(), expected.prior().proof());
        assert_eq!(actual.proposal(), expected.proposal());
    }
    let prepared_us = start.elapsed().as_micros();
    eprintln!("actual76 preparedSource full Native parity preparation_us={preparation_us} reference_us={reference_us} prepared_us={prepared_us} receipt={:?}; excludes context/class preparation and model inference; ordinary input encode included",prepared.prepared_source_receipt().unwrap());
}
