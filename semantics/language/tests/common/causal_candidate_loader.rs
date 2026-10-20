//! Explicit unaccepted runtime artifacts; hash custody is not Source re-execution.
use super::parser_model_kernel::ModelDriver;
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use conduit_language::parser_model::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
pub fn text(v: &Value) -> &str {
    v.as_str().unwrap()
}
pub fn number(v: &Value) -> u64 {
    v.as_u64().unwrap()
}
pub fn sha(path: &Path) -> String {
    assert!(path.is_absolute());
    let result = Command::new("sha256sum").arg(path).output().unwrap();
    assert!(result.status.success());
    String::from_utf8(result.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
fn read(path: &Path, maximum: u64) -> Vec<u8> {
    assert!(path.is_absolute());
    assert!(std::fs::metadata(path).unwrap().len() <= maximum);
    std::fs::read(path).unwrap()
}
fn checked(reference: &Value, maximum: u64) -> (Vec<u8>, PathBuf) {
    let path = PathBuf::from(text(&reference["path"]));
    let bytes = read(&path, maximum);
    assert_eq!(sha(&path), text(&reference["sha256"]));
    (bytes, path)
}
fn json_file(reference: &Value) -> Value {
    serde_json::from_slice(&checked(reference, 16 * 1024 * 1024).0).unwrap()
}
pub struct Arm {
    pub name: &'static str,
    pub bytes: Vec<u8>,
    pub artifact: ModelArtifact,
    pub signature: ModelSignature,
    pub identity: Value,
}
impl Arm {
    pub fn model(&self) -> ModelDriver {
        ModelDriver::with_descriptor(
            self.artifact.clone(),
            self.signature.clone(),
            &self.bytes,
            prepare_external,
        )
    }
}
fn prepare_external(
    resource: Option<Arc<AdmittedModelResource>>,
    pool: ResourcePoolId,
    residence: ResourceContentOffer,
) -> Result<PreparedCategoricalStep, ParserScorerRefusal> {
    let resource = resource.ok_or(ParserScorerRefusal::Absent)?;
    let (_, expected) = english_parser_scorer_descriptor();
    if resource.signature() != &expected {
        return Err(ParserScorerRefusal::ForeignSignature);
    }
    let bounds = &residence.contract;
    if bounds.maximum_bytes != 62796
        || bounds.maximum_items != 1
        || bounds.reader_leases == 0
        || bounds.generation_slots != 1
        || bounds.publication_slots != 0
    {
        return Err(ParserScorerRefusal::ResourceBounds);
    }
    let profile = PreparedCategoricalStep::prepare(resource, pool, residence)
        .map_err(|_| ParserScorerRefusal::NumericalContract)?;
    if profile.dimensions() != ENGLISH_PARSER_DIMENSIONS {
        return Err(ParserScorerRefusal::NumericalContract);
    }
    Ok(profile)
}
fn groups() -> Vec<std::ops::Range<usize>> {
    let mut groups = vec![
        0..18,
        18..36,
        36..360,
        360..366,
        366..371,
        371..373,
        373..374,
        374..379,
    ];
    groups.extend((379..413).step_by(2).map(|i| i..i + 2));
    assert_eq!(groups.len(), 25);
    groups
}
fn artifact(reference: &Value, name: &'static str, fit: &Value) -> Arm {
    let (bytes, path) = checked(reference, 62796);
    assert_eq!(bytes.len(), 62796);
    assert_eq!(&bytes[..8], b"CI16SUM1");
    for (offset, expected) in [(8, 413), (12, 76), (16, 25)] {
        assert_eq!(
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            expected
        );
    }
    assert_eq!(reference["sha256"], fit["artifacts"][name]["sha256"]);
    let groups = groups();
    let mut maximum = 0i64;
    for row in bytes[20..].as_chunks::<{ 413 * 2 }>().0 {
        let weights: Vec<_> = row
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| i64::from(i16::from_le_bytes(*v)))
            .collect();
        let minimum: i64 = groups
            .iter()
            .map(|g| g.clone().map(|i| weights[i]).min().unwrap())
            .sum();
        let high: i64 = groups
            .iter()
            .map(|g| g.clone().map(|i| weights[i]).max().unwrap())
            .sum();
        maximum = maximum.max(minimum.abs()).max(high.abs());
    }
    assert!(
        maximum <= 128525,
        "candidate violates unchanged checked Source score bound"
    );
    assert_eq!(
        maximum as u64,
        number(&fit["artifacts"][name]["maximum_group_score_bound"])
    );
    let (mut artifact, signature) = english_parser_scorer_descriptor();
    let content = model_content_digest(&bytes);
    artifact.content.identity = ResourceSemanticIdentity::from_digest(content);
    artifact.content.lifetime.version = ResourceVersionIdentity::from_digest(content);
    artifact.content.access_class = "language/parser-causal-ablation/unaccepted-read@1".into();
    artifact.validate(&signature).unwrap();
    let identity = json!({"classification":"UNACCEPTED_NONBLIND_CANDIDATE","arm":name,"path":path,"sha256":reference["sha256"],"content":artifact.content_identity(),"descriptor":artifact.descriptor_digest(&signature).unwrap(),"signature":signature.semantic_digest().unwrap(),"maximum_group_score_bound":maximum,"resource_reference_encoded_hex":artifact.content.encode().unwrap().iter().map(|b|format!("{b:02x}")).collect::<String>()});
    Arm {
        name,
        bytes,
        artifact,
        signature,
        identity,
    }
}
pub struct Inputs {
    pub manifest: Value,
    pub cases: Vec<Value>,
    pub arms: [Arm; 2],
    pub output: PathBuf,
    pub pins: Value,
}
impl Inputs {
    pub fn load() -> Self {
        let path = PathBuf::from(
            std::env::var_os("CONDUIT_PARSER_CANDIDATE_PAIR_MANIFEST")
                .expect("absolute externally reviewed evaluation manifest"),
        );
        let expected = std::env::var("CONDUIT_PARSER_CANDIDATE_PAIR_MANIFEST_SHA256")
            .expect("externally pinned manifest SHA256");
        assert_eq!(expected.len(), 64);
        assert!(expected
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        let manifest: Value = serde_json::from_slice(&read(&path, 16 * 1024 * 1024)).unwrap();
        assert_eq!(sha(&path), expected);
        assert_eq!(manifest["schema"], "matched-causal-candidate-evaluation@1");
        assert_eq!(
            manifest["classification"],
            "UNACCEPTED_FROZEN_NONBLIND_EVALUATION_ONLY"
        );
        assert_eq!(
            manifest["abi"],
            json!({"features":413,"classes":76,"lookups":25,"bytes":62796,"ordered_groups":groups().iter().map(|g|vec![g.start,g.end]).collect::<Vec<_>>()})
        );
        let fit = json_file(&manifest["fit_receipt"]);
        let cohort = json_file(&manifest["reviewed_cohort"]);
        let crosslink = json_file(&manifest["crosslink"]);
        assert_eq!(
            fit["status"],
            "FITTED_NONBLIND_TRAIN_ABLATION_NOT_DEFAULT_NOT_CALIBRATED"
        );
        assert_eq!(cohort["status"], "REVIEWED_FROZEN_ALIGNED_SOURCE_VALIDATED");
        assert_eq!(cohort["schema"], "reviewed-aligned-causal-train@1");
        assert_eq!(
            cohort["reviewed_exclusion_basis"],
            "whole_sentence_actual_occurrence_segmentation_only"
        );
        assert_eq!(cohort["original_sentences"], 394);
        assert_eq!(cohort["original_pairs"], 1620);
        assert_eq!(cohort["all_other_validation_failures"], 0);
        assert_eq!(cohort["no_augmentation"], true);
        assert_eq!(cohort["previously_fitted_train"], true);
        assert_eq!(
            crosslink["status"],
            "JSON_DIGEST_CROSSLINKS_PASS_NOT_NATIVE_DECODE_OR_FIT_ADMISSION"
        );
        assert_eq!(fit["cohort_sha256"], manifest["reviewed_cohort"]["sha256"]);
        assert_eq!(
            crosslink["inputs"]["cohort"],
            manifest["reviewed_cohort"]["sha256"]
        );
        for (key, receipt_key) in [
            ("trainer", "trainer_sha256"),
            ("trainer_gate", "gate_sha256"),
        ] {
            checked(&manifest[key], 16 * 1024 * 1024);
            assert_eq!(fit[receipt_key], manifest[key]["sha256"]);
        }
        assert_eq!(fit["source_evidence_files"], cohort["files"]);
        for (key, value) in [
            ("seed", 4907),
            ("epochs", 12),
            ("scale", 256),
            ("features", 413),
            ("classes", 76),
            ("lookups", 25),
        ] {
            assert_eq!(number(&fit[key]), value);
        }
        let pins = json_file(&manifest["source_pins"]);
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        for (name, hash) in pins["files"].as_object().unwrap() {
            assert_eq!(sha(&repo.join(name)), text(hash));
        }
        checked(&manifest["ablation_spec"], 16 * 1024 * 1024);
        assert_eq!(pins["spec_sha256"], manifest["ablation_spec"]["sha256"]);
        let profile = json_file(&manifest["lexical_profile"]);
        assert_eq!(
            profile,
            serde_json::from_str::<Value>(ENGLISH_PARSER_LEXICAL_PROFILE).unwrap()
        );
        let cases = json_file(&manifest["cases"]);
        assert_eq!(cases["schema"], "causal-candidate-evaluation-cases@1");
        assert_eq!(cases["status"], "FROZEN_PRE_EVALUATION_NONBLIND_REGRESSION");
        let rows = cases["cases"].as_array().unwrap().clone();
        assert_eq!(rows.len(), 17);
        let mut ids = BTreeSet::new();
        let mut controls = 0;
        for row in &rows {
            assert!(ids.insert(text(&row["id"])));
            assert!(matches!(
                text(&row["partition"]),
                "previously_observed_dev"
                    | "previously_observed_test"
                    | "previously_observed_reserved"
            ));
            controls += usize::from(
                !row["gold_relations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| text(r).split(':').next() == Some("vocative")),
            );
            assert!((1..=8).contains(&row["steps"].as_array().unwrap().len()));
            let n = row["gold_heads"].as_array().unwrap().len();
            assert!((1..=4).contains(&n));
            assert_eq!(row["gold_relations"].as_array().unwrap().len(), n);
            assert_eq!(row["forms"].as_array().unwrap().len(), n);
            assert!((1..=n as u64).contains(&number(&row["pending_consumer_through"])));
            let mut previous = 0;
            for step in row["steps"].as_array().unwrap() {
                let count = number(&step["expected_available_count"]);
                assert!((1..=4).contains(&count) && count >= previous);
                previous = count;
                assert!(
                    number(&step["stable_prefix_scalars"])
                        <= text(&step["text"]).chars().count() as u64
                );
            }
            assert_eq!(previous as usize, n);
        }
        for suffix in ["reserved-initial", "reserved-medial", "reserved-terminal"] {
            assert!(ids.iter().any(|id| id.ends_with(suffix)));
        }
        assert_eq!(
            controls, 9,
            "frozen17 rows include one existing TEST vocative"
        );
        let arms = [
            artifact(&manifest["arms"]["causal"], "causal", &fit),
            artifact(&manifest["arms"]["full"], "full", &fit),
        ];
        let output = PathBuf::from(
            std::env::var_os("CONDUIT_PARSER_CANDIDATE_OUTPUT")
                .expect("fresh absolute evidence directory"),
        );
        assert!(output.is_absolute());
        std::fs::create_dir(&output).unwrap();
        assert_eq!(
            manifest["batching"],
            json!({
                "schema":"sequential-case-arm-batches@1", "arm_order":["causal","full"],
                "case_order":rows.iter().map(|r|r["id"].clone()).collect::<Vec<_>>(),
                "batches":34,"decoder_scope":"one_case_one_arm",
                "trace_scope":"one_prefix_one_case_one_arm","trace_maximum_bytes":67108864,"model_maximum_invocations":4096,
                "artifacts_loaded_once":true,
                "counter_reset_scope":"new_prepared_decoder_per_case_not_installed_capacity"
            })
        );
        Self {
            manifest,
            cases: rows,
            arms,
            output,
            pins,
        }
    }
}
