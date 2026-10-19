//! Actual checked Source plus production numerical channel, bounded hosted prep.
//! This is not resumable installed-stream execution or model accuracy acceptance.
#![cfg(all(
    feature = "parser-model",
    target_has_atomic = "ptr",
    target_has_atomic = "8"
))]
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/parser_round_model_channel.rs"]
mod numeric;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{
        fallback_round::{learned::*, *},
        *,
    },
    lexical::prepare_lexical_tape,
    revision::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
#[test]
fn actual_source_features_and_owned_numeric_ack_feed_single_round_and_refuse_foreign_context() {
    let mut runtime = runtime::Runtime::new();
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let context = RevisionContext {
        stream: text("analysis"),
        subject: text("utterance"),
        epoch: text("1"),
        producer: text("learned-round-fixture"),
        policy: text("learned-hosted-preparation@1"),
    };
    let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, fixture::dep(), 1).unwrap();
    let observed = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let program = PreparedAnalysisInitializationProgram::prepare().unwrap();
    let initial = program
        .replay(
            prepare_lexical_tape(&source, &fixture::profile(), None).unwrap(),
            &begin,
            &observed,
        )
        .unwrap();
    assert_eq!(runtime.source_id.as_str(), initial.source_id());
    let foreign_initial = program
        .replay(
            prepare_lexical_tape(&source, &fixture::profile(), None).unwrap(),
            &begin,
            &observed,
        )
        .unwrap();
    let failed_initial = program.replay(lexical, &begin, &observed).unwrap();
    let limits = RevisionLimits {
        history_events: 16,
        revisable_units: 4,
    };
    let mut owner = AnalysisEpochOwner::initialize(initial, &domain, context, limits).unwrap();
    let failed_owner =
        AnalysisEpochOwner::initialize(failed_initial, &domain, context, limits).unwrap();
    let mut foreign = AnalysisEpochOwner::initialize(
        foreign_initial,
        &domain,
        RevisionContext {
            stream: text("foreign-stream"),
            ..context
        },
        limits,
    )
    .unwrap();
    let profile = numeric::profile();
    let (document, catalog, definition) = numeric::definition(&profile);
    assert!(matches!(
        PreparedLearnedRoundChannel::prepare_canonical(
            &document,
            "parser-round-numeric",
            &catalog,
            definition.clone(),
            profile.clone(),
            ParserRoundModel::UnacceptedPunctuationV3,
            128
        ),
        Err(FallbackRoundRefusal::Model)
    ));
    let mut channel = PreparedLearnedRoundChannel::prepare_canonical(
        &document,
        "parser-round-numeric",
        &catalog,
        definition,
        profile,
        ParserRoundModel::AcceptedEnglishV2,
        128,
    )
    .unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let prepared = match owner.prepare_learned_round_with(&programs, &mut runtime, &mut channel) {
        Ok(value) => value,
        Err(failure) => panic!(
            "learned preparation: {:?}; retained {}; pending {}; kernel signs {:?}",
            failure.refusal,
            failure.learned.len(),
            channel.pending().is_some(),
            channel.numerical().signs()
        ),
    };
    assert_eq!(prepared.mode(), AnalysisRoundMode::AuthenticatedCategorical);
    assert!(!prepared.learned_evidence().is_empty());
    assert!(prepared.learned_evidence().len() <= MAX_LEARNED_REQUESTS_PER_ROUND);
    for (ordinal, evidence) in prepared.learned_evidence().iter().enumerate() {
        let request = evidence.request();
        assert_eq!(request.request_ordinal(), ordinal);
        assert_eq!(request.generation(), owner.generation());
        assert_eq!(request.context()[0], "analysis");
        assert_eq!(request.query().epoch(), source.sequence());
        assert_eq!(
            request.features().invocation(),
            request.query().invocation()
        );
        assert_eq!(
            evidence.numerical().invocation(),
            *request.query().invocation()
        );
        let mut exact = vec![0; evidence.numerical().input_len()];
        evidence.numerical().copy_input_into(&mut exact).unwrap();
        assert_eq!(exact, request.input_canonical());
        assert_eq!(evidence.selection(), ParserRoundModel::AcceptedEnglishV2);
        assert!(!evidence.numerical().active_plays().is_empty());
        assert_eq!(evidence.source_id(), prepared.source_id());
    }
    let before = foreign.beam().clone().encode().unwrap();
    let rejected = match foreign.apply_fallback_round(prepared) {
        Err(failure) => failure,
        Ok(()) => panic!("foreign stream accepted a prepared round"),
    };
    assert!(matches!(rejected.refusal, FallbackRoundRefusal::Stale));
    assert_eq!(foreign.beam().clone().encode().unwrap(), before);
    match owner.apply_fallback_round(rejected.prepared) {
        Ok(()) => (),
        Err(failure) => panic!("rightful apply: {:?}", failure.refusal),
    }
    assert_eq!(owner.generation(), 1);
    assert!(channel.pending().is_none());
    assert!(!channel.numerical().signs().is_empty());
    // A provider transport failure at ranking occurs AFTER a real numerical ACK.
    // It must preserve that receipt and prevent another partial-round attempt.
    let failure = match failed_owner.prepare_learned_round_with(
        &programs,
        &mut RefuseRanking(&mut runtime),
        &mut channel,
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("injected Source transport failure accepted"),
    };
    assert!(matches!(failure.refusal, FallbackRoundRefusal::Evaluation));
    assert!(!failure.learned.is_empty());
    assert!(channel.is_stopped());
    assert_eq!(failed_owner.generation(), 0);
    let operations = runtime.operations;
    let refused =
        match failed_owner.prepare_learned_round_with(&programs, &mut runtime, &mut channel) {
            Err(failure) => failure,
            Ok(_) => panic!("terminal partial round replayed"),
        };
    assert!(matches!(
        refused.refusal,
        FallbackRoundRefusal::PendingNumerical
    ));
    assert!(refused.learned.is_empty());
    assert!(refused.observations.is_empty());
    assert_eq!(runtime.operations, operations);
    runtime.assert_signs();
}

struct RefuseRanking<'a>(&'a mut runtime::Runtime);
impl FallbackObservationProvider for RefuseRanking<'_> {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal> {
        if operation == "language-parser-scorer-rank" {
            return Err(FallbackRoundRefusal::Evaluation);
        }
        self.0.observe(operation, input)
    }
}
