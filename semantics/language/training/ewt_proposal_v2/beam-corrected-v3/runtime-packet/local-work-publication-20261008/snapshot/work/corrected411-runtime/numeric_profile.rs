//! Actual original Source projection + retained numeric target execution.
#![cfg(feature = "parser-model-selection")]
#[path="/home/dancxjo/conduit-4907-native-reuse/work/corrected411-runtime/candidate.rs"]
mod candidate;
#[path="/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/tests/common/parser_fixture.rs"]
mod fixture;
#[path="/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/tests/common/parser_model_resource.rs"]
mod model_resource;
#[path="/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/tests/common/parser_planned_runtime.rs"]
#[allow(dead_code)]
mod planned;
use conduit_core::*;
fn timed<T>(name:&str,f:impl FnOnce()->T)->T{let start=std::time::Instant::now();let value=f();eprintln!("phase={name} elapsed_nanos={}",start.elapsed().as_nanos());value}

use conduit_language::{parser_window8_program_bank::*, *};
use conduit_plot::rust_binding::*;
#[test]
#[ignore = "explicit pinned proposer Source/model target execution"]
fn exact_proposal_v2_source_model_outputs_match_full_resource() {
    let inputs = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PROPOSAL_FEATURE_INPUTS").expect("retained complete inputs"),
    );
    let candidate = timed("cold_dictionary_proposer_model_admission",candidate::prepare);
    assert!(std::ptr::eq(
        candidate.selected.categorical(),
        candidate.scorer.as_ref()
    ));
    assert_eq!(
        candidate
            .selected
            .declaration()
            .source_contract()
            .feature_contract,
        candidate.contracts.feature_contract
    );
    eprintln!(
        "full original proposer owner reservation: {:?}",
        candidate.owner.storage_receipt()
    );
    let limits = PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 64,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_retained_bytes: 512 * 1024 * 1024,
        maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
        maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
    };
    let bank = Window8ProgramBank::prepare_proposal_v2_native_evaluator(
        limits,
        Window8SourcePreparationLimits {
            maximum_retained_bytes: 512 * 1024 * 1024,
            maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        },
    )
    .unwrap();
    let source = candidate::numeric_source(&candidate);
    eprintln!("distinct411 original numerical Source/Plan preparation start");
    let mut execution = planned::prepare_proposal_window8_v2_source_with_inference_budget(
        candidate.scorer.clone(),
        source,
        "window8-proposal-v2-learned-model",
        2,
    );
    eprintln!("distinct411 original numerical Source/Plan preparation complete");
    let resource = candidate.scorer.resource();
    let reference = conduit_ai::integer_categorical::IntegerCategoricalModel::prepare(
        resource.artifact(),
        resource.signature(),
        resource.bytes(),
    )
    .unwrap();
    for (sequence, name) in ["vocative", "object"].into_iter().enumerate() {
        let input = std::fs::read(inputs.join(format!("new-feature-{name}.bin"))).unwrap();
        let query = timed("warm_full_native_feature_query_decode",||LanguageParserProposalWindow8FeatureQuery::decode(&input).unwrap());
        let raw = timed("warm_source_context_and_feature_values_with_complete_native_output",||bank.proposal_v2_features(query).unwrap());
        let features = timed("warm_full_native_feature_wrapper",||LanguageParserProposalWindow8V2Features::new(raw).unwrap());
        let mut expected = [0i64; 76];
        reference
            .infer_indices_into(features.raw().indices(), &mut expected)
            .unwrap();
        let ingress=timed("warm_native_feature_clone_into_structured",||features.raw().clone().into_structured().unwrap());
        let actual=timed("warm_actual_source_plan_kernel_and_output",||execution.infer(sequence as u64,&ingress));
        let StructuredInfoValueShape::Collection(values) = actual.shape() else {
            panic!("full76scorecollection")
        };
        assert_eq!(values.len(), 76);
        let observed = values
            .iter()
            .map(|v| {
                let StructuredInfoValueShape::Leaf(b) = v.shape() else {
                    panic!("I64")
                };
                i64::from_le_bytes(b.try_into().unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
        eprintln!("actual original411 Source/model target {name} all76scores exact");
    }
    eprintln!("PASS two exact numeric target calls; pinned fullmodel/dictionary/proposer/Source; no corpus or publicSession claim");
}

#[test]
fn journal_overflow_refuses_before_source_preparation() {
    let candidate = timed("cold_dictionary_proposer_model_admission",candidate::prepare);
    let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        planned::prepare_proposal_window8_v2_source_with_inference_budget(
            candidate.scorer.clone(),
            "deliberately invalid Source must never be parsed".into(),
            "absent-entry",
            16128,
        )
    }));
    let panic = match refusal {
        Ok(_) => panic!("one-over journal capacity accepted"),
        Err(value) => value,
    };
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap();
    assert_eq!(
        message,
        "combined retained journal bound overflows before Source preparation"
    );
}
