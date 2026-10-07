#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_joint_flows.rs"]
mod parser_joint_flows;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_joint_v2_runtime.rs"]
mod runtime;
#[path = "common/scorer_model.rs"]
mod scorer_model;
use runtime::evaluate_rows;
#[test]
fn learned_reviewed_vocative_teaching_graphs_run_native_width_four_pipeline() {
    evaluate(Some("reviewed-teaching"));
}
#[test]
#[ignore = "explicit bounded corpus evidence; cold graph preparation and finite Flow storage measured"]
fn learned_heldout_joint_pos_and_attachment_evidence() {
    evaluate(None);
}
#[test]
#[ignore = "explicit varied-name generalization evidence"]
fn learned_reviewed_name_generalization() {
    evaluate(Some("reviewed-generalization"));
}
#[test]
#[ignore = "explicit exact motivating punctuation evaluation; first predictions retained"]
fn learned_exact_hello_travis_punctuation() {
    evaluate(Some("reviewed-punctuation"));
}
fn evaluate(selected: Option<&str>) {
    let rows = if selected == Some("reviewed-teaching") {
        serde_json::from_str::<Vec<joint::Sentence>>(include_str!(
            "../training/ewt_joint_v2/reviewed_teaching.json"
        ))
        .unwrap()
    } else if selected == Some("reviewed-generalization") {
        serde_json::from_str::<Vec<joint::Sentence>>(include_str!(
            "../training/ewt_joint_v2/reviewed_generalization.json"
        ))
        .unwrap()
    } else if selected == Some("reviewed-punctuation") {
        serde_json::from_str::<Vec<joint::Sentence>>(include_str!(
            "../training/ewt_joint_v2/reviewed_punctuation.json"
        ))
        .unwrap()
    } else {
        include_str!("../training/ewt_joint_v2/test_annotations.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<joint::Sentence>(line).unwrap())
            .collect()
    };
    let native_inputs = rows.iter().map(joint::lexical).collect::<Vec<_>>();
    evaluate_rows(selected, rows, native_inputs);
}
