//! Synthetic ASR input construction and the existing common Language kernel.
//! No recognition model, ASR parser, publication policy or second revision log.
#![allow(dead_code)]
#[path = "../../../language/tests/common/parser_kernel.rs"]
mod kernel;
use conduit_core::ValuePayload;
use conduit_language::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;

pub fn seq<T, const N: usize>(items: impl IntoIterator<Item = T>) -> BoundedSequence<T, N> {
    BoundedSequence::try_from_iter(items).ok().unwrap()
}
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "synthetic/asr-common-consumer".into(),
        "1".into(),
    )
    .unwrap()
}
pub fn profile() -> LanguageLexicalProfile {
    LanguageLexicalProfile::new(
        seq([
            ("you", LanguageLexicalPos::Pronoun),
            ("would", LanguageLexicalPos::Auxiliary),
        ]
        .map(|(word, pos)| {
            LanguageLexicalEntry::new(
                seq([LanguageLexicalCandidate::new(word.into(), seq([]), pos).unwrap()]),
                word.into(),
            )
            .unwrap()
        })),
        "synthetic/asr-common-consumer".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
pub fn revision(value: &str) -> LanguageTextRevisionId {
    LanguageTextRevisionId::new(value.into()).unwrap()
}
pub fn envelope(sequence: u64, event: AsrRecognitionEvent) -> AsrRecognitionEnvelope {
    let time = ListeningEventTime::new(
        i64::try_from(sequence).unwrap(),
        ListeningClockOrigin::StreamStart,
    )
    .unwrap();
    AsrRecognitionEnvelope::new(
        event,
        ListeningEventId::new(format!("synthetic/event/{sequence}")).unwrap(),
        ListeningProvenance::new(
            seq([]),
            ListeningProvenanceKind::Derived,
            None,
            Some("synthetic-envelope-fixture-no-ASR-model".into()),
            seq([]),
        )
        .unwrap(),
        sequence,
        ListeningStreamId::new("synthetic/stream".into()).unwrap(),
        ListeningEventTimes::new(time.clone(), time).unwrap(),
    )
    .unwrap()
}
pub fn partial(sequence: u64, text: &str) -> AsrRecognitionEnvelope {
    envelope(
        sequence,
        AsrRecognitionEvent::partial_hypothesis(
            None,
            ListeningTextRole::Recognition,
            ListeningSegmentId::new("segment".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
    )
}
pub fn revised(sequence: u64, start: u32, end: u32, text: &str) -> AsrRecognitionEnvelope {
    envelope(
        sequence,
        AsrRecognitionEvent::revised_hypothesis(
            None,
            LanguageTextRange::new(end, start).unwrap(),
            ListeningTextRole::Recognition,
            ListeningSegmentId::new("segment".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
    )
}
pub fn committed(sequence: u64, text: &str) -> AsrRecognitionEnvelope {
    envelope(
        sequence,
        AsrRecognitionEvent::committed_segment(
            None,
            None,
            ListeningTextRole::Recognition,
            ListeningSegmentId::new("segment".into()).unwrap(),
            None,
            text.into(),
            seq([]),
        )
        .unwrap(),
    )
}
pub fn cancelled(sequence: u64) -> AsrRecognitionEnvelope {
    envelope(
        sequence,
        AsrRecognitionEvent::hypothesis_cancelled(
            "synthetic cancellation".into(),
            ListeningTextRole::Recognition,
            ListeningSegmentId::new("segment".into()).unwrap(),
        )
        .unwrap(),
    )
}
pub fn source() -> String {
    [
        include_str!("../../../language/types.conduit"),
        include_str!("../../../language/identity.conduit"),
        include_str!("../../../language/coverage.conduit"),
        include_str!("../../../language/syntax.conduit"),
        include_str!("../../../language/text_revision.conduit"),
        include_str!("../../../language/revision_lineage.conduit"),
        include_str!("../../../language/lexical.conduit"),
        include_str!("../../../language/parser.conduit"),
        include_str!("../../../language/parser_beam.conduit"),
        include_str!("../../../language/parser_available.conduit"),
        include_str!("../../../language/parser_revision_rebase.conduit"),
        PARSER_BEAM_COMMIT_SOURCE,
    ]
    .join("\n")
}
/// Existing checked Source expands, selects a Plan and runs through normal Play.
pub fn play<I: NativeRustBinding, O: NativeRustBinding>(entry: &str, input: I) -> O {
    let mut run = kernel::Execution::prepare(source(), entry);
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let packet = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: input.encode().unwrap(),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(65536),
    };
    run.kernel.start().unwrap();
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &packet)
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    assert!((0..4000).any(|_| {
        run.step();
        run.kernel
            .output_into(&output_port.port_id, &mut output)
            .unwrap()
            == Some(0)
    }));
    run.kernel.complete_output(&output_port.port_id, 0).unwrap();
    run.kernel.close_input(&input_port.port_id).unwrap();
    assert!((0..4000).any(|_| run.step() == conduit_composite::KernelCompositeStatus::Complete));
    O::decode(&output.encoded).unwrap()
}
pub fn relation(base: LanguageUniversalDependencyRelation) -> LanguageParserRelation {
    LanguageParserRelation::new(base, LanguageParserSubtype::new(String::new()).unwrap()).unwrap()
}
pub fn native(raw: &LanguageParserNumericState) -> LanguageParserState {
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
pub fn commit_request(
    state: &LanguageParserState,
    cancelled: bool,
    pressure: bool,
) -> LanguageParserBeamCommitRequest {
    let hypothesis =
        |active, id| LanguageParserHypothesis::new(active, id, 0, state.clone()).unwrap();
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            state.basis().clone(),
            hypothesis(true, 1),
            hypothesis(false, 2),
            hypothesis(false, 3),
            hypothesis(false, 4),
        )
        .unwrap(),
        cancelled,
        LanguageParserRawFrontier::new(
            state.basis().clone(),
            *state.committed(),
            *state.token_count(),
        )
        .unwrap(),
        pressure,
        *state.token_count(),
    )
    .unwrap()
}
