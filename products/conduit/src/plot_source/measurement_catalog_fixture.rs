//! Exact canonical test fixture; no host or device authority.
use conduit_core::CheckedValueContract;
fn latest_inputs(count: usize) -> Result<(CheckedValueContract, CheckedValueContract), String> {
    let contract = |arity| {
        let (ty, bytes) = conduit_data::measurement_chart_constructor_input(arity)?;
        CheckedValueContract::new(
            ty.profile()
                .map_err(|_| "measurement tuple profile")?
                .value_kind()
                .clone(),
            bytes,
            vec![],
        )
        .map_err(|_| "measurement tuple contract")
    };
    Ok((
        contract(count - 1).map_err(String::from)?,
        contract(1).map_err(String::from)?,
    ))
}

pub(super) fn source() -> String {
    let (left, right) = latest_inputs(2).unwrap();
    let first = conduit_semantic_catalog::combine_latest_schema_with_input_terminals(
        &left,
        &right,
        [false, false],
    )
    .unwrap();
    let (left, right) = latest_inputs(3).unwrap();
    let second = conduit_semantic_catalog::combine_latest_schema_with_input_terminals(
        &left,
        &right,
        [true, false],
    )
    .unwrap();
    let (_, bound) = conduit_data::measurement_chart_constructor_input(1).unwrap();
    format!(
        r#"
plot inspection/three-series (
    >> first: MeasurementPlotSeries...| <= {bound}B
    >> second: MeasurementPlotSeries...| <= {bound}B
    >> third: MeasurementPlotSeries...| <= {bound}B
    done: Empty...| >>
    pair-failed: conduit/terminal-info@1 <= 34B >>
    latest-failed: conduit/terminal-info@1 <= 34B >>
    chart-failed: conduit/terminal-info@1 <= 34B >>
    failed: conduit/terminal-info@1 <= 34B >>
) {{
    pair: {first}
    latest: {second}
    chart: data/measurement-chart/3(labels = ["First", "Second", "Third"], maximum-updates = 8)
    view: face/measurement-chart-flow("Measurements", 8)
    first >> pair.left
    second >> pair.right
    pair.latest >> latest.left
    third >> latest.right
    latest.latest >> chart.latest
    chart.chart >> view.chart
    view.done >> done
    pair.latest! >> pair-failed
    latest.latest! >> latest-failed
    chart.chart! >> chart-failed
    view.done! >> failed
}}
"#,
        first = first.kind_id.as_str(),
        second = second.kind_id.as_str()
    )
}
