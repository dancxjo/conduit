//! Exact Source completion adapter parity; these states establish no Session ancestry.
extern crate alloc;
pub use conduit_language::parser_session_runtime;
#[path = "../src/parser_session_execution.rs"]
mod parser_session_execution;
use conduit_language::*;
use conduit_plot::rust_binding::{
    NativeRustBinding, PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
use parser_session_execution::{
    verification::PreparedSourceVerification, ParserSessionEntry, ParserSessionVerificationLimits,
};

#[test]
fn completion_observation_preserves_original_full_source_decision() {
    let limits = ParserSessionVerificationLimits {
        decoded_program_bytes: 128 * 1024 * 1024,
        preparation_peak_bytes: 512 * 1024 * 1024,
        retained_bytes: 256 * 1024 * 1024,
    };
    let (mut original, _, original_output_type, _) =
        PreparedSourceVerification::prepare(ParserSessionEntry::DecodeComplete, limits).unwrap();
    let (mut observation, _, _, _) =
        PreparedSourceVerification::prepare(ParserSessionEntry::Completion, limits).unwrap();
    assert_eq!(
        original_output_type,
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/bool")).unwrap()
    );
    assert_eq!(
        ParserSessionEntry::Completion.program_hex().lines().count(),
        2
    );
    let mut family = PreparedNativeFamily::prepare(
        &[
            LanguageParserState::PREPARED_DESCRIPTOR,
            LanguageParserCompletionObservation::PREPARED_DESCRIPTOR,
        ],
        PreparedNativeFamilyLimits {
            maximum_types: 64,
            maximum_laws_per_type: 128,
            maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_retained_bytes: 512 * 1024 * 1024,
            maximum_preparation_peak_bytes: 512 * 1024 * 1024,
            maximum_conversion_requested_bytes: 1024 * 1024 * 1024,
        },
    )
    .unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        LanguageTextRevisionId::new("source/1".into()).unwrap(),
        LanguageTextId::new("text/1".into()).unwrap(),
    )
    .unwrap();
    let dep = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    for complete in [false, true] {
        let state = LanguageParserState::new(
            basis.clone(),
            0,
            1,
            if complete {
                [4, 5, 5, 5, 4]
            } else {
                [5, 5, 5, 5, 4]
            },
            if complete { root.clone() } else { dep.clone() },
            dep.clone(),
            dep.clone(),
            dep.clone(),
            [4; 5],
            1,
            u64::from(complete),
        )
        .unwrap();
        let input = state.clone().encode().unwrap();
        assert_eq!(family.decode::<LanguageParserState>(&input).unwrap(), state);
        let legacy = original.evaluate(&input).unwrap();
        // Primitive pure-expression wires carry the fixed Type's payload;
        // the adapter's returned record carries the complete canonical frame.
        assert_eq!(legacy, &[u8::from(complete)]);
        let actual = observation.evaluate(&input).unwrap();
        let expected = LanguageParserCompletionObservation::new(complete)
            .unwrap()
            .encode()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            *family
                .decode::<LanguageParserCompletionObservation>(actual)
                .unwrap()
                .complete(),
            complete
        );
    }
}
