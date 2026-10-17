//! Numerical artifact evidence only: no dependency edge is supplied to a parser.
use conduit_ai::{integer_categorical::*, *};
use conduit_core::*;
use conduit_data::{TensorAxisRole, TensorElement};
use serde_json::{json, Value};
use std::time::Instant;
const BYTES: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
const SCHEMA: &[u8] = include_bytes!("../training/ewt_joint_v2/schema.json");
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn fixture() -> (ModelArtifact, ModelSignature) {
    let schema = hex(semantic_digest("language/parser-action-schema@1", SCHEMA));
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).unwrap(),
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("language/parser-{name}/{schema}@1")).unwrap(),
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(
                        ModelDimensionConstraint::fixed(count).unwrap(),
                        TensorAxisRole::Feature,
                    )
                    .unwrap()],
                    count * 8,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let signature = ModelSignature::from_parts(
        format!("language/parser-actions/{schema}@1"),
        1,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, 25)],
        vec![port("scores", TensorElement::I64, 76)],
    )
    .unwrap();
    let digest = model_content_digest(BYTES);
    let artifact = ModelArtifact {
        architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: CATEGORICAL_I16_FORMAT.into(),
        precision_profile: CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: CATEGORICAL_I16_FORMAT.into(),
            access_class: "language/parser-model/read@1".into(),
            extent: ResourceExtent {
                bytes: BYTES.len() as u64,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(digest),
                expires_at: None,
            },
        },
    };
    (artifact, signature)
}
#[test]
fn pinned_artifact_ranks_heldout_actions_with_existing_numeric_owner() {
    let manifest: Value =
        serde_json::from_str(include_str!("../training/ewt_joint_v2/manifest.json")).unwrap();
    let (artifact, signature) = fixture();
    assert_eq!(
        hex(artifact.content_identity()),
        manifest["model_content_identity"]
    );
    assert_eq!(
        hex(semantic_digest("language/parser-action-schema@1", SCHEMA)),
        manifest["feature_class_contract_identity"]
    );
    let model = IntegerCategoricalModel::prepare(&artifact, &signature, BYTES).unwrap();
    assert_eq!(model.dimensions(), (413, 76, 25));
    assert_eq!(model.work_units(), 1900);
    assert_eq!(
        model.maximum_score_magnitude(),
        manifest["maximum_absolute_score"].as_u64().unwrap()
    );
    assert_eq!(
        hex(semantic_digest(
            "language/parser-lexical-profile@1",
            include_bytes!("../training/ewt_joint_v2/lexical_profile.json")
        )),
        manifest["lexical_profile_identity"]
    );
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../training/ewt_joint_v2/ranking_cases.json")).unwrap();
    let mut counts = [[0u64; 2]; 2];
    let mut failures = Vec::new();
    let mut invocations = 0u64;
    let mut elapsed_ns = 0u128;
    let mut maximum_ns = 0u128;
    for case in cases {
        let mut best: Option<(i64, usize, Vec<u64>)> = None;
        for alternative in case["alternatives"].as_array().unwrap() {
            let indices: Vec<u64> = alternative
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect();
            let mut scores = [0i64; 76];
            let start = Instant::now();
            model.infer_indices_into(&indices, &mut scores).unwrap();
            let ns = start.elapsed().as_nanos();
            elapsed_ns += ns;
            maximum_ns = maximum_ns.max(ns);
            invocations += 1;
            for (class, score) in scores.into_iter().enumerate() {
                if best.as_ref().is_none_or(|(s, c, f)| {
                    score > *s || (score == *s && (class < *c || (class == *c && indices < *f)))
                }) {
                    best = Some((score, class, indices.clone()));
                }
            }
        }
        let (_, class, indices) = best.unwrap();
        let slot = usize::from(case["split"] == "test");
        if class as u64 != case["target"].as_u64().unwrap() || json!(indices) != case["features"] {
            failures.push(json!({"sentence":case["sentence"], "split":case["split"], "expected_class":case["target"], "selected_class":class, "gold_features":case["features"], "selected_features":indices}));
        }
        counts[slot][1] += 1;
        counts[slot][0] += u64::from(
            class as u64 == case["target"].as_u64().unwrap() && json!(indices) == case["features"],
        );
    }
    for (slot, name) in ["dev", "test"].iter().enumerate() {
        let metrics =
            &manifest["conditional_gold_action_lexical_and_gold_state_joint_diagnostics"][name];
        assert_eq!(counts[slot][0], metrics["joint_context_action_correct"]);
        assert_eq!(counts[slot][1], metrics["oracle_steps"]);
    }
    let memory = std::fs::read_to_string("/proc/self/status").ok().map(|s| {
        s.lines()
            .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
            .collect::<Vec<_>>()
            .join("; ")
    });
    println!(
        "{}",
        json!({"proof":"gold-state joint action ranking; no blind parse", "counts":counts,"failures":failures, "model_invocations":invocations,"numeric_total_ns":elapsed_ns,"numeric_max_ns":maximum_ns,"model_bytes":BYTES.len(),"numeric_working_bytes":model.working_bytes(),"caller_input_bytes":200,"caller_output_bytes":608,"whole_process_memory_ceiling":false,"process_memory_observation":memory})
    );
}
#[test]
fn altered_content_signature_and_invalid_features_refuse_without_output_mutation() {
    let (artifact, mut signature) = fixture();
    let mut changed = BYTES.to_vec();
    changed[20] ^= 1;
    assert!(IntegerCategoricalModel::prepare(&artifact, &signature, &changed).is_err());
    signature.compatibility_version += 1;
    assert!(IntegerCategoricalModel::prepare(&artifact, &signature, BYTES).is_err());
    let (artifact, signature) = fixture();
    let model = IntegerCategoricalModel::prepare(&artifact, &signature, BYTES).unwrap();
    let mut scores = [99i64; 76];
    assert!(model.infer_indices_into(&[413; 25], &mut scores).is_err());
    assert_eq!(scores, [99; 76]);
    assert!(model.infer_indices_into(&[0; 24], &mut scores).is_err());
    assert_eq!(scores, [99; 76]);
}
