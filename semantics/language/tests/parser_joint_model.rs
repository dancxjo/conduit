use conduit_ai::integer_categorical::IntegerCategoricalModel;
use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_fixture.rs"]
mod joint;
use fixture::replace;
#[path = "common/scorer_model.rs"]
mod scorer_model;
const BYTES: &[u8] = include_bytes!("../training/ewt_joint/ewt_joint.i16");
const MANIFEST: &str = include_str!("../training/ewt_joint/manifest.json");
const PROFILE: &[u8] = include_bytes!("../training/ewt_joint/lexical_profile.json");
fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
fn scorer() -> scorer_model::Scorer {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let joint = hex(semantic_digest(
        "language/parser-joint-encoding@1",
        include_bytes!("../parser_joint.conduit"),
    ));
    let profile = hex(semantic_digest(
        "language/parser-lexical-profile@1",
        PROFILE,
    ));
    assert_eq!(manifest["joint_choice_contract_identity"], joint);
    assert_eq!(manifest["lexical_profile_identity"], profile);
    scorer_model::Scorer::prepare(BYTES, MANIFEST, &format!("joint/{joint}/{profile}"))
}
#[test]
fn wrong_pos_supervised_artifact_has_exact_native_identity_and_numeric_inference() {
    let mut scorer = scorer();
    let model =
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, BYTES).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let tensor = model.weights_tensor().unwrap();
    tensor.validate().unwrap();
    assert_eq!(tensor.dimensions.as_slice(), [76, 374]);
    assert_eq!(
        hex(tensor.content_digest),
        manifest["weights_tensor_content_identity"]
    );
    assert_eq!(hex(model.identity()), manifest["model_content_identity"]);
    assert_ne!(
        scorer.artifact.content_identity(),
        scorer_model::Scorer::new().artifact.content_identity()
    );
    let mut corrupt = BYTES.to_vec();
    corrupt[24] ^= 1;
    assert!(
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, &corrupt).is_err()
    );
    let mut wrong = scorer.artifact.clone();
    wrong.signature_identity[0] ^= 1;
    assert!(IntegerCategoricalModel::prepare(&wrong, &scorer.signature, BYTES).is_err());
    let indices = [17, 25, 349, 361, 366, 371, 373];
    let basis = semantic_digest(
        "fixture/joint-model-input@1",
        b"explicit numeric proof only",
    );
    let (scores, evidence) = scorer.score(&indices, basis);
    assert_eq!(scores.len(), 76);
    assert_eq!(
        evidence.artifact_identity,
        scorer.artifact.content_identity()
    );
    assert_eq!(
        evidence.signature_identity,
        scorer.signature.semantic_digest().unwrap()
    );
    assert_eq!(evidence.input_identities, [basis]);
    for (class, score) in scores.iter().enumerate() {
        let expected: i64 = indices
            .iter()
            .map(|i| {
                let offset = 20 + (class * 374 + *i as usize) * 2;
                i16::from_le_bytes([BYTES[offset], BYTES[offset + 1]]) as i64
            })
            .sum();
        assert_eq!(*score, expected);
    }
    assert_eq!(scorer.adapter.work_units(), 532);
}

#[test]
fn native_choice_prefix_and_prepared_shared_basis_refuse_forgery() {
    let row: joint::Sentence = include_str!("../training/ewt_joint/test_annotations.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<joint::Sentence>(line).unwrap())
        .find(|row| row.id == "email-enronsent32_01-0051")
        .unwrap();
    let lexical = joint::lexical(&row).unwrap();
    let mut f = fixture::Fixture::new();
    let scorer = joint::scorer();
    let initial = joint::initial(&mut f, &lexical, &scorer);
    let hypothesis = LanguageParserJointHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 1, 0, initial.clone()).unwrap(),
    )
    .unwrap();
    assert!(LanguageParserJointRuntimeHypothesis::new(hypothesis.clone(), 2).is_err());
    assert!(
        LanguageParserJointChoiceQuery::new([3, 0, 0, 0], 0, 0, lexical.clone(), initial.clone())
            .is_err()
    );
    let foreign = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("foreign-analysis".into()).unwrap(),
        initial.basis().source_revision().clone(),
        initial.basis().text().clone(),
    )
    .unwrap();
    let state = LanguageParserState::from_structured(replace(
        &initial.clone().into_structured().unwrap(),
        "basis",
        foreign.into_structured().unwrap(),
    ))
    .unwrap();
    let foreign = LanguageParserJointHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 2, 0, state).unwrap(),
    )
    .unwrap();
    assert!(
        LanguageParserJointBeam::new(
            initial.basis().clone(),
            foreign.clone(),
            foreign.clone(),
            foreign.clone(),
            foreign,
            0,
            0,
            lexical
        )
        .is_err()
    );
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let assigned = LanguageParserState::new(
        initial.basis().clone(),
        0,
        2,
        [4, 5, 5, 5, 4],
        root,
        initial.relation1().clone(),
        initial.relation2().clone(),
        initial.relation3().clone(),
        [4, 0, 4, 4, 4],
        1,
        1,
    )
    .unwrap();
    let advanced = LanguageParserJointHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 3, 0, assigned).unwrap(),
    )
    .unwrap();
    assert!(LanguageParserJointRuntimeHypothesis::new(advanced.clone(), 0).is_err());
    assert!(LanguageParserJointRuntimeHypothesis::new(advanced, 1).is_ok());
}
