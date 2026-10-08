//! Ordinary Plan/Play proof of Source diagnostic projection transport only.
use super::*;

#[test]
fn ordinary_source_trace_outputs_retain_exact_types_and_refuse_pressure_and_cancel() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check(false))
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn ordinary_source_trace_and_primary_pcm_sink_allocate_nothing_during_play() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| check(true))
        .unwrap()
        .join()
        .unwrap();
}
fn check(primary: bool) {
    let mut source = super::super::declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../../speech/fargan_epoch_trace.conduit")
        + "\n"
        + include_str!("../../../../speech/fargan_trace_flow.conduit");
    let entry = if primary {
        source.push_str("\nplot speech/flow-fargan-diagnostic-forward (\n    >> value: FarganPcm16EpochResult...|\n    result: FarganPcm16EpochResult...| >>\n) = .\n");
        let original = include_str!("../../../../speech/fargan_trace_flow.conduit");
        let extra = &original[original
            .find("plot speech/flow-fargan-trace-projections (")
            .unwrap()..];
        source.push_str(&extra.replace("speech/flow-fargan-trace-projections (", "speech/flow-fargan-trace-and-primary (")
            .replace("    pcm_trace: FarganCommittedResultTrace...| >>", "    pcm_trace: FarganCommittedResultTrace...| >>\n    result: FarganPcm16EpochResult...| >>")
            .replace("    provisional >> features.value", "    forward: speech/flow-fargan-diagnostic-forward\n    provisional >> features.value")
            .replace("    accepted >> pcm.value", "    accepted >> pcm.value\n    accepted >> forward.value\n    forward.result >> result"));
        "speech/flow-fargan-trace-and-primary"
    } else {
        "speech/flow-fargan-trace-projections"
    };
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
    let source = source
        .lines()
        .filter(|line| !line.starts_with("type Numeric"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = source.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {literal}\n"),
    );
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        super::super::prepared_epoch_profiles_with_capacity(true),
        source,
        entry,
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
    let types: Vec<_> = [
        ("feature_trace", "FarganFeatureConditionTrace"),
        ("history_trace", "FarganConditionHistoryTrace"),
        ("pcm_trace", "FarganCommittedResultTrace"),
    ]
    .into_iter()
    .map(|(port, name)| {
        (
            port.to_owned(),
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap()
                .value_type
                .clone(),
        )
    })
    .collect();
    let traces = trace_hooks::TraceSinks::prepare(&types, anchor, 7, 1).unwrap();
    let result = run_epoch_stream_plan_with_trace(
        plan.clone(),
        &context,
        &resources,
        inputs.clone(),
        None,
        StreamRun {
            expected: usize::from(primary),
            mode: ExecutionMode::Normal,
            trace: Some(&traces),
            service: ServiceBudget::legacy(),
        },
    )
    .unwrap();
    assert!(result.drained);
    assert_eq!(result.scheduler_step_allocations, 0);
    assert_eq!(result.prepared_expression_allocations, 0);
    assert_eq!(result.values.len(), usize::from(primary));
    if primary {
        assert_eq!(result.values[0], accepted);
    }
    assert!(traces.finished());
    let values: Vec<_> = types
        .iter()
        .flat_map(|(port, _)| traces.rows(port))
        .map(|bytes| StructuredInfoValue::from_canonical_bytes(&bytes).unwrap())
        .collect();
    assert_eq!(values.len(), 3);
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
        let value = values
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
    for mode in [
        ExecutionMode::StoragePressure,
        ExecutionMode::CancelFirstExpression,
    ] {
        let refused = trace_hooks::TraceSinks::prepare(&types, anchor, 7, 1).unwrap();
        assert!(run_epoch_stream_plan_with_trace(
            plan.clone(),
            &context,
            &resources,
            inputs.clone(),
            None,
            StreamRun {
                expected: usize::from(primary),
                mode,
                trace: Some(&refused),
                service: ServiceBudget::legacy(),
            }
        )
        .is_none());
        assert!(!refused.finished());
    }
}
