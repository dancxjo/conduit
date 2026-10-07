#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
//! Measure retained remote lifecycle evidence for the actual admitted model Plan.
extern crate alloc;
pub use conduit_language::{
    LanguageId, LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos,
    LanguageLexicalProfile, LinguisticDerivationProvenance,
};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_planned_runtime.rs"]
mod planned;
use conduit_core::*;
use conduit_language::parser_model_selection as selection;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn bounded_remote_lifecycle_journal_cost_is_measured_without_reset_or_eviction() {
    replay(None, 70);
}
#[test]
fn declared_batch_retains_evidence_and_refuses_excess_before_ingress() {
    replay(Some(80), 80);
}
#[test]
fn complete_corpus_bound_prepares_both_remote_and_main_retained_journals() {
    let prepared = model_resource::categorical(
        include_bytes!("../training/ewt_joint_v2/ewt_joint.i16").to_vec(),
        selection::pinned_v2_model_signature().unwrap(),
        1,
    );
    let _execution = planned::prepare_with_inference_budget(prepared, 4096);
}
fn replay(budget: Option<u16>, first_refusal: u64) {
    let bytes = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
    let prepared = model_resource::categorical(
        bytes.to_vec(),
        selection::pinned_v2_model_signature().unwrap(),
        1,
    );
    let selected = selection::PreparedParserModelSelection::prepare(
        prepared.clone(),
        &selection::pinned_v2_lexical_profile().unwrap(),
    )
    .unwrap();
    assert_eq!(
        selected.expected_lexical_profile(),
        &selection::pinned_v2_lexical_profile().unwrap()
    );
    assert_eq!(selected.compatibility().lookups, 25);
    let mut execution = match budget {
        Some(maximum) => {
            planned::prepare_with_inference_budget(selected.prepared_categorical().clone(), maximum)
        }
        None => planned::prepare(selected.prepared_categorical().clone()),
    };
    drop(selected);
    drop(prepared);
    let mut exhausted = None;
    for frame in 0..257u64 {
        let indices: Vec<_> = (0..25u64).map(|i| (i * 13 + frame) % 413).collect();
        let basis = conduit_language::LanguageParserBasis::new(
            conduit_language::LanguageAnalysisRevisionId::new(format!("analysis/{frame}")).unwrap(),
            conduit_language::LanguageTextRevisionId::new(format!("source/{frame}")).unwrap(),
            conduit_language::LanguageTextId::new("utterance".into()).unwrap(),
        )
        .unwrap();
        let features = conduit_language::LanguageParserV2ModelFeatures::new(
            basis,
            indices.clone().try_into().unwrap(),
        )
        .unwrap();
        let value = features.into_structured().unwrap();
        let trial = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execution.infer(frame, &value)
        }));
        let score_value = match trial {
            Ok(scores) => scores,
            Err(error) => {
                let message = error
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| error.downcast_ref::<&str>().copied())
                    .unwrap_or("");
                assert!(
                    message.contains(if budget.is_some() {
                        "declared inference batch exhausted before ingress"
                    } else {
                        "RemoteItemCapacityExceeded"
                    }),
                    "unexpected failure: {message}"
                );
                exhausted = Some(frame);
                break;
            }
        };
        if frame < 3 {
            for (host, signs) in execution.signs_snapshot() {
                let remote = signs
                    .iter()
                    .filter(|event| {
                        matches!(
                            event.kind,
                            conduit_kernel::KernelEventKind::RemoteValueOffered
                                | conduit_kernel::KernelEventKind::RemoteValueAccepted
                                | conduit_kernel::KernelEventKind::RemoteValueDelivered
                                | conduit_kernel::KernelEventKind::RemoteOutputClosed
                                | conduit_kernel::KernelEventKind::RemoteInputAdmitted
                                | conduit_kernel::KernelEventKind::RemoteInputClosed
                        )
                    })
                    .count();
                assert_eq!(remote, 4 * (frame as usize + 1));
                eprintln!(
                    "completed inference={frame} child={host:?} retained_remote_signs={remote}"
                );
            }
        }
        let StructuredInfoValueShape::Collection(scores) = score_value.shape() else {
            panic!("bare I64 scores")
        };
        assert_eq!(scores.len(), 76);
        for (class, score) in scores.iter().enumerate() {
            let StructuredInfoValueShape::Leaf(raw) = score.shape() else {
                panic!("I64 leaf")
            };
            let actual = i64::from_le_bytes(raw.try_into().unwrap());
            let expected: i64 = indices
                .iter()
                .map(|index| {
                    let offset = 20 + 2 * (class * 413 + *index as usize);
                    i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as i64
                })
                .sum();
            assert_eq!(actual, expected);
        }
    }
    assert_eq!(
        exhausted,
        Some(first_refusal),
        "the current finite journal capacity is enforced"
    );
    eprintln!("first journal admission failure={exhausted:?}; no model reset or evidence eviction");
}
