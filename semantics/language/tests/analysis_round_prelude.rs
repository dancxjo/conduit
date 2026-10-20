//! Actual Source differential and no-growth admission for a bounded prelude.
//! No full round, model ACK, installed stream, current commitment or consumer claim.
#![cfg(target_has_atomic = "ptr")]
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
#[allow(dead_code)]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{
        fallback_round::{canonical_prelude::*, *},
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
fn no_growth<T>(operation: impl FnOnce() -> T) -> T {
    let (result, allocations) = allocation_probe::observe(operation);
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    result
}
#[test]
fn canonical_prelude_resumes_each_source_program_with_private_lease_and_unchanged_owner() {
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
        producer: text("canonical-prelude-fixture"),
        policy: text("source-prelude-only@1"),
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
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &observed)
        .unwrap();
    let owner = AnalysisEpochOwner::initialize(
        initial,
        &domain,
        context,
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    let prior = owner.beam().clone().encode().unwrap();
    let lexical = owner.lexical().tape().clone().encode().unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let mut cursor = CanonicalRoundPrelude::prepare(&owner, &programs).unwrap();
    let mut foreign = CanonicalRoundPrelude::prepare(&owner, &programs).unwrap();
    assert_eq!(cursor.original_beam_canonical(), prior);
    assert_eq!(cursor.original_lexical_canonical(), lexical);
    assert_eq!(cursor.context()[0], "analysis");
    let capacity = cursor.payload_capacity();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for ordinal in 0..2 {
        let lease = no_growth(|| cursor.issue()).unwrap();
        if ordinal == 0 {
            // Same snapshots/generation still cannot substitute another cursor's lease.
            let _foreign_lease = foreign.issue().unwrap();
            let refusal = match foreign.submit_observed(lease, &[]) {
                Err(failure) => failure,
                Ok(()) => panic!("foreign lease accepted"),
            };
            assert_eq!(refusal.refusal, PreludeRefusal::ForeignLease);
            foreign.cancel();
            assert!(foreign.retained_request(0).is_some());
            assert!(matches!(
                foreign.step_admission(),
                Err(PreludeRefusal::Lifecycle)
            ));
            // The unchanged lease still belongs to the original pending request.
            drive(
                &mut cursor,
                refusal.lease,
                ordinal,
                &mut runtime,
                &mut inputs,
                &mut outputs,
            );
        } else {
            drive(
                &mut cursor,
                lease,
                ordinal,
                &mut runtime,
                &mut inputs,
                &mut outputs,
            );
        }
    }
    assert_eq!(cursor.payload_capacity(), capacity);
    assert_eq!(
        no_growth(|| cursor.step_admission()).unwrap(),
        PreludeProgress::Complete
    );
    assert!(matches!(cursor.issue(), Err(PreludeRefusal::Lifecycle)));
    let receipt = match no_growth(|| cursor.finish()) {
        Ok(receipt) => receipt,
        Err(_) => panic!("incomplete"),
    };
    assert_eq!(receipt.len(), 2);
    for ordinal in 0..2 {
        let (operation, input, output) = receipt.observation(ordinal).unwrap();
        assert_eq!(input, inputs[ordinal]);
        assert_eq!(output, outputs[ordinal]);
        assert_eq!(
            operation,
            if ordinal == 0 {
                "language/analysis-round-empty"
            } else {
                "language-parser-decode-complete"
            }
        );
    }
    assert_eq!(
        inputs[1],
        owner
            .beam()
            .candidate0()
            .parser()
            .state()
            .clone()
            .encode()
            .unwrap()
    );
    assert_eq!(owner.beam().clone().encode().unwrap(), prior);
    assert_eq!(owner.generation(), 0);
    assert_eq!(owner.frontiers().committed, TokenFrontier(0));
    runtime.assert_signs();
    // A captured but incorrect output never becomes Source-admitted evidence.
    let mut bad = CanonicalRoundPrelude::prepare(&owner, &programs).unwrap();
    let lease = bad.issue().unwrap();
    let mut altered = outputs[0].clone();
    altered[0] ^= 1;
    match bad.submit_observed(lease, &altered) {
        Ok(()) => (),
        Err(_) => panic!("bounded observation refused before admission"),
    }
    loop {
        match no_growth(|| bad.step_admission()) {
            Ok(PreludeProgress::Admitting) => (),
            Err(PreludeRefusal::ForeignOutput) => break,
            other => panic!("wrong output admission {other:?}"),
        }
    }
    assert_eq!(bad.observed(0), Some(altered.as_slice()));
    assert!(bad.admitted(0).is_none());
    assert!(bad.retained_request(0).is_some());
    assert!(matches!(bad.issue(), Err(PreludeRefusal::Lifecycle)));
}
fn drive(
    cursor: &mut CanonicalRoundPrelude,
    lease: PreludeSourceLease,
    ordinal: usize,
    runtime: &mut runtime::Runtime,
    inputs: &mut Vec<Vec<u8>>,
    outputs: &mut Vec<Vec<u8>>,
) {
    let request = cursor.request(&lease).unwrap();
    assert_eq!(request.ordinal, ordinal);
    let input = request.canonical_input.to_vec();
    let output = runtime.execute(request.operation.name(), &input);
    assert!(cursor.admitted(ordinal).is_none());
    let oversized = vec![0; cursor.observed_capacity(ordinal).unwrap() + 1];
    let lease = match no_growth(|| cursor.submit_observed(lease, &oversized)) {
        Err(failure) => {
            assert_eq!(failure.refusal, PreludeRefusal::Pressure);
            failure.lease
        }
        Ok(()) => panic!("output capacity pressure was not refused"),
    };
    assert_eq!(cursor.request(&lease).unwrap().canonical_input, input);
    match no_growth(|| cursor.submit_observed(lease, &output)) {
        Ok(()) => (),
        Err(failure) => panic!("actual Source observation refused {:?}", failure.refusal),
    }
    loop {
        match no_growth(|| cursor.step_admission()).unwrap() {
            PreludeProgress::Admitting => (),
            PreludeProgress::SourceAdmitted { ordinal: actual } => {
                assert_eq!(actual, ordinal);
                break;
            }
            other => panic!("unexpected admission progress {other:?}"),
        }
    }
    assert_eq!(cursor.admitted(ordinal), Some(output.as_slice()));
    inputs.push(input);
    outputs.push(output);
}
