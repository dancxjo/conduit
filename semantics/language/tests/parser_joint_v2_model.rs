use conduit_ai::integer_categorical::IntegerCategoricalModel;
use conduit_core::*;
#[path = "common/scorer_model.rs"]
mod scorer_model;
const BYTES: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
const MANIFEST: &str = include_str!("../training/ewt_joint_v2/manifest.json");
const PROFILE: &[u8] = include_bytes!("../training/ewt_joint_v2/lexical_profile.json");
fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn availability_aware_artifact_has_exact_identity_and_25_lookup_inference() {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let profile = hex(semantic_digest(
        "language/parser-lexical-profile@1",
        PROFILE,
    ));
    assert_eq!(manifest["lexical_profile_identity"], profile);
    let available = hex(semantic_digest(
        "language/parser-available-contract@1",
        include_bytes!("../parser_available.conduit"),
    ));
    assert_eq!(manifest["availability_contract_identity"], available);
    let classes = hex(semantic_digest(
        "language/parser-scorer-encoding@1",
        include_bytes!("../parser_scorer.conduit"),
    ));
    assert_eq!(manifest["action_class_contract_identity"], classes);
    let mut scorer =
        scorer_model::Scorer::prepare_v2(BYTES, MANIFEST, &format!("v2/{available}/{profile}"));
    let model =
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, BYTES).unwrap();
    let tensor = model.weights_tensor().unwrap();
    tensor.validate().unwrap();
    assert_eq!(tensor.dimensions.as_slice(), [76, 413]);
    assert_eq!(hex(model.identity()), manifest["model_content_identity"]);
    assert_eq!(
        hex(tensor.content_digest),
        manifest["weights_tensor_content_identity"]
    );
    let maximum = BYTES[20..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| i16::from_le_bytes(*bytes).unsigned_abs() as u64)
        .max()
        .unwrap();
    assert!(
        maximum * 25 <= 229376,
        "pinned artifact fits existing Source expansion score bound"
    );
    let mut corrupt = BYTES.to_vec();
    corrupt[24] ^= 1;
    assert!(
        IntegerCategoricalModel::prepare(&scorer.artifact, &scorer.signature, &corrupt).is_err()
    );
    let mut wrong = scorer.artifact.clone();
    wrong.signature_identity[0] ^= 1;
    assert!(IntegerCategoricalModel::prepare(&wrong, &scorer.signature, BYTES).is_err());
    // Explicit numeric fixture; Source candidate-set extraction is proved separately.
    let mut indices = vec![17, 25, 349, 361, 366, 371, 373, 376];
    indices.extend((0..17).map(|code| 379 + 2 * code));
    let (scores, evidence) = scorer.score(
        &indices,
        semantic_digest("fixture/v2-input@1", b"numeric fixture only"),
    );
    assert_eq!(
        evidence.artifact_identity,
        scorer.artifact.content_identity()
    );
    assert_eq!(scores.len(), 76);
    for (class, actual) in scores.iter().enumerate() {
        let expected: i64 = indices
            .iter()
            .map(|index| {
                let offset = 20 + 2 * (class * 413 + *index as usize);
                i16::from_le_bytes(BYTES[offset..offset + 2].try_into().unwrap()) as i64
            })
            .sum();
        assert_eq!(*actual, expected);
    }
    assert_eq!(scorer.adapter.work_units(), 1900);
}
