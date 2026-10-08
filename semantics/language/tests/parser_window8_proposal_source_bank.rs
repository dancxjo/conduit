//! Exact Source/Native parity, not model selection or corpus accuracy.
use conduit_language::{parser_window8_program_bank::*, *};
use conduit_plot::{rust_binding::*, PortableExpressionProgram};
#[test]
#[ignore = "requires retained original proposer/contrast canonical inputs"]
fn exact_proposal_source_bank_matches_retained_origin_and_feature_frames() {
    let input_directory = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PROPOSAL_FEATURE_INPUTS")
            .expect("retained canonical input directory"),
    );
    let native = PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 64,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_retained_bytes: 512 * 1024 * 1024,
        maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
        maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
    };
    let limits = Window8SourcePreparationLimits {
        maximum_retained_bytes: 512 * 1024 * 1024,
        maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    };
    let bank = Window8ProgramBank::prepare_proposal_native_evaluator(native, limits).unwrap();
    let out = std::path::Path::new(env!("OUT_DIR"));
    let program = |name: &str| {
        PortableExpressionProgram::from_canonical_hex(
            &std::fs::read_to_string(out.join(format!("{name}.hex"))).unwrap(),
        )
        .unwrap()
    };
    let input = std::fs::read(input_directory.join("actual-origin-query.bin")).unwrap();
    let query = LanguageParserProposalWindow8OriginQuery::decode(&input).unwrap();
    let expected = program("proposal_window8_origins")
        .evaluate(&input)
        .unwrap();
    let actual = bank.proposal_origins(query).unwrap();
    assert_eq!(actual.clone().encode().unwrap(), expected);
    let guarded = LanguageParserProposalWindow8Origins::new(actual).unwrap();
    assert_eq!(guarded.raw().origins(), &[0, 1, 1, 2, 2, 2, 2, 2]);
    let mut indices = Vec::new();
    for name in ["vocative", "object"] {
        let input = std::fs::read(input_directory.join(format!("new-feature-{name}.bin"))).unwrap();
        let expected_context = program("proposal_window8_feature_context")
            .evaluate(&input)
            .unwrap();
        let expected = program("proposal_window8_feature_values")
            .evaluate(&expected_context)
            .unwrap();
        let query = LanguageParserProposalWindow8FeatureQuery::decode(&input).unwrap();
        let actual = bank.proposal_features(query).unwrap();
        assert_eq!(actual.clone().encode().unwrap(), expected);
        let guarded = LanguageParserProposalWindow8Features::new(actual).unwrap();
        indices.push(*guarded.raw().indices());
    }
    assert_eq!(&indices[0][..25], &indices[1][..25]);
    assert_ne!(indices[0][25], indices[1][25]);
    assert!(indices.iter().flatten().all(|i| *i < 446));
    println!("PASS original actual revision origin Source + constructed contrasting complete27 feature Source/Native parity {:?}; no model inference/heldout/publicSession claim",bank.prepared_source_receipt().unwrap());
}

#[test]
#[ignore = "requires retained original proposer/contrast canonical inputs"]
fn exact_proposal_v2_source_bank_matches_retained_origin_and_feature_frames() {
    let input_directory = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PROPOSAL_FEATURE_INPUTS")
            .expect("retained canonical input directory"),
    );
    let native = PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 64,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        maximum_retained_bytes: 512 * 1024 * 1024,
        maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
        maximum_conversion_requested_bytes: 2 * 1024 * 1024 * 1024,
    };
    let limits = Window8SourcePreparationLimits {
        maximum_retained_bytes: 512 * 1024 * 1024,
        maximum_preparation_peak_bytes: 1024 * 1024 * 1024,
        maximum_input_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    };
    let bank = Window8ProgramBank::prepare_proposal_v2_native_evaluator(native, limits).unwrap();
    let out = std::path::Path::new(env!("OUT_DIR"));
    let program = |name: &str| {
        PortableExpressionProgram::from_canonical_hex(
            &std::fs::read_to_string(out.join(format!("{name}.hex"))).unwrap(),
        )
        .unwrap()
    };
    let input = std::fs::read(input_directory.join("actual-origin-query.bin")).unwrap();
    let query = LanguageParserProposalWindow8OriginQuery::decode(&input).unwrap();
    let expected = program("proposal_window8_origins")
        .evaluate(&input)
        .unwrap();
    let actual = bank.proposal_origins(query).unwrap();
    assert_eq!(actual.clone().encode().unwrap(), expected);
    let guarded = LanguageParserProposalWindow8Origins::new(actual).unwrap();
    assert_eq!(guarded.raw().origins(), &[0, 1, 1, 2, 2, 2, 2, 2]);
    let mut indices = Vec::new();
    for name in ["vocative", "object"] {
        let input = std::fs::read(input_directory.join(format!("new-feature-{name}.bin"))).unwrap();
        let expected_context = program("proposal_window8_feature_context")
            .evaluate(&input)
            .unwrap();
        let expected = program("proposal_window8_v2_feature_values")
            .evaluate(&expected_context)
            .unwrap();
        let query = LanguageParserProposalWindow8FeatureQuery::decode(&input).unwrap();
        let actual = bank.proposal_v2_features(query).unwrap();
        assert_eq!(actual.clone().encode().unwrap(), expected);
        let guarded = LanguageParserProposalWindow8V2Features::new(actual).unwrap();
        indices.push(*guarded.raw().indices());
    }
    assert_eq!(&indices[0][..25], &indices[1][..25]);
    assert_ne!(indices[0][25], indices[1][25]);
    assert!(indices.iter().flatten().all(|i| *i < 411));
    println!("PASS original actual revision origin Source + constructed contrasting complete411/27 feature Source/Native parity {:?}; no model inference/heldout/publicSession claim",bank.prepared_source_receipt().unwrap());
}
