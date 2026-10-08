//! Ordinary Plan/Play proof of Source diagnostic projection transport only.
use super::*;

#[test]
fn ordinary_source_trace_outputs_retain_exact_types_and_refuse_pressure_and_cancel() {
    let source = super::super::declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../../speech/fargan_epoch_trace.conduit")
        + "\n"
        + include_str!("../../../../speech/fargan_trace_flow.conduit");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let value = |name: &str| {
        super::super::declarations::fixture_value(
            &checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap()
                .value_type,
        )
    };
    let provisional = value("FarganTraceProposal");
    let accepted = value("FarganPcm16EpochResult");
    let anchor = super::super::case_state::field(&accepted, "model");
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let literal = format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let source = source.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {literal}\n"),
    );
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        super::super::prepared_epoch_profiles_with_capacity(true),
        source,
        "speech/flow-fargan-trace-projections",
        true,
        vec![],
    )
    .unwrap();
    let inputs = BTreeMap::from([
        (
            "provisional".into(),
            vec![provisional.canonical_bytes().unwrap()],
        ),
        ("accepted".into(), vec![accepted.canonical_bytes().unwrap()]),
    ]);
    let resources: Resources = BTreeMap::new();
    let result = run_epoch_stream_plan(
        plan.clone(),
        &context,
        &resources,
        inputs.clone(),
        None,
        3,
        ExecutionMode::Normal,
    )
    .unwrap();
    assert_eq!(result.values.len(), 3);
    for name in [
        "FarganFeatureConditionTrace",
        "FarganConditionHistoryTrace",
        "FarganCommittedResultTrace",
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type;
        let value = result
            .values
            .iter()
            .find(|value| value.value_type() == ty)
            .unwrap();
        let full = if name == "FarganCommittedResultTrace" {
            super::super::case_state::field(value, "result")
        } else {
            value
        };
        assert_eq!(super::super::case_state::field(full, "model"), anchor);
        assert_eq!(
            super::super::case_state::field(full, "epoch"),
            super::super::case_state::field(&accepted, "epoch")
        );
    }
    assert!(run_epoch_stream_plan(
        plan.clone(),
        &context,
        &resources,
        inputs.clone(),
        None,
        3,
        ExecutionMode::StoragePressure
    )
    .is_none());
    assert!(run_epoch_stream_plan(
        plan,
        &context,
        &resources,
        inputs,
        None,
        3,
        ExecutionMode::CancelFirstExpression
    )
    .is_none());
}
