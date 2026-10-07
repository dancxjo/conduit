use conduit_ai::integer_categorical::IntegerCategoricalModel;
use conduit_core::*;
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
