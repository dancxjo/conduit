//! Actual authored three-source composition, not a Rust chart assembly loop.
//! Four distinct typed terminal Fore outputs preserve each operation's origin.
//! No implicit single-terminal arbitration or recovery is claimed.
//! Authored external Fronts currently cannot declare abnormal capability; the
//! first exact latest profile accepts none, and the second accepts only its
//! internal left terminal. Both retain their separate typed output terminals.
use super::tests::Capture;
use crate::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot, Wake};
use conduit_core::{ConnectionTrack, HostAdvertisement, Plan, PortDirection, StructuredInfoValue};
use conduit_data::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use std::collections::BTreeMap;
const SOURCE: &str = r#"
plot inspection/three-series (
    >> blue: MeasurementPlotSeries...| <= SERIES_BOUND_BYTES
    >> green: MeasurementPlotSeries...| <= SERIES_BOUND_BYTES
    >> yellow: MeasurementPlotSeries...| <= SERIES_BOUND_BYTES
    done: Empty...| >>
    pair-failed: conduit/terminal-info@1 <= 34B >>
    latest-failed: conduit/terminal-info@1 <= 34B >>
    chart-failed: conduit/terminal-info@1 <= 34B >>
    failed: conduit/terminal-info@1 <= 34B >>
) {
    pair: FIRST_LATEST
    latest: SECOND_LATEST
    chart: data/measurement-chart/3(labels = ["Blue", "Green", "Yellow"], maximum-updates = 8)
    view: face/measurement-chart-flow("Conditions", 8)
    blue >> pair.left
    green >> pair.right
    pair.latest >> latest.left
    yellow >> latest.right
    latest.latest >> chart.latest
    chart.chart >> view.chart
    view.done >> done
    pair.latest! >> pair-failed
    latest.latest! >> latest-failed
    chart.chart! >> chart-failed
    view.done! >> failed
}
"#;
fn planned_graph(host: &HostAdvertisement, source: &str) -> (BodyPlan, Wake) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    install_measurement_chart_constructor(3, &mut startup, &mut profiles).unwrap();
    install_measurement_chart_flow_catalog(&mut startup, &mut profiles).unwrap();
    let (series, series_bound) = measurement_chart_constructor_input(1).unwrap();
    let (pair, pair_bound) = measurement_chart_constructor_input(2).unwrap();
    let series = conduit_core::CheckedValueContract::new(
        series.profile().unwrap().value_kind().clone(),
        series_bound,
        vec![],
    )
    .unwrap();
    let pair = conduit_core::CheckedValueContract::new(
        pair.profile().unwrap().value_kind().clone(),
        pair_bound,
        vec![],
    )
    .unwrap();
    let first = conduit_semantic_catalog::install_combine_latest_schema_with_input_terminals(
        &series,
        &series,
        [false, false],
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let second = conduit_semantic_catalog::install_combine_latest_schema_with_input_terminals(
        &pair,
        &series,
        [true, false],
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let source = source
        .replace("SERIES_BOUND_BYTES", &format!("{series_bound}B"))
        .replace("FIRST_LATEST", first.as_str())
        .replace("SECOND_LATEST", second.as_str());
    let source = source.as_str();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "inspection/three-series",
        &profiles,
    )
    .unwrap();
    let hosts = [host.clone()];
    let selected =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    // Every actual external binding gets its own finite queue, including
    // terminal-only Fore routes. Nothing increases an internal/global grant.
    let mut boundary = BTreeMap::new();
    let (_, series_bound) = measurement_chart_constructor_input(1).unwrap();
    for (direction, bindings) in [
        (PortDirection::Input, &expanded.input_bindings),
        (PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            let byte_capacity = if direction == PortDirection::Input {
                series_bound
            } else if binding.track == ConnectionTrack::AbnormalTerminal {
                conduit_core::TERMINAL_INFO_ENCODED_LEN as u32
            } else {
                assert_eq!(binding.front_port_id.as_str(), "done");
                1 // Positive queue capacity; Empty still emits no payload.
            };
            assert!(boundary
                .insert(
                    ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity
                    }
                )
                .is_none());
        }
    }
    let plan: Plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &selected,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: measurement_chart_canonical_bytes() as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary,
    )
    .unwrap();
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "proof/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "proof/wake".into()).unwrap().1;
    (
        BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan }]).unwrap(),
        wake,
    )
}

fn input(name: &str, value: i64) -> ExternalForeInput {
    let series = MeasurementPlotSeries::from_projected(
        vec![MeasurementPlotPoint::new(0, 0, value).unwrap()],
        1,
        0,
    )
    .unwrap();
    ExternalForeInput {
        front_port_id: name.into(),
        track: ConnectionTrack::Payload,
        bytes: StructuredInfoValue::leaf(
            measurement_plot_series_type(),
            encode_measurement_plot_series(&series).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap(),
    }
}
struct Outputs;
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        Err(format!(
            "terminal-only graph unexpectedly emitted payload: {:?}",
            value.front_port_id
        ))
    }
}
#[test]
fn authored_three_source_latest_constructor_and_flow_face_share_one_admitted_play() {
    let mut composition = StdHostComposition::minimal();
    composition.data = true;
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "proof/three-series".into(),
            boot_id: "proof/three-series/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        composition,
    );
    let (plan, wake) = planned_graph(host.advertisement(), SOURCE);
    let mut capture = Capture {
        frames: Vec::with_capacity(8),
        pressure: 1,
        final_pressure: 0,
        attempts: 0,
    };
    let inputs = [
        input("blue", 250_000),
        input("green", 500_000),
        input("yellow", 750_000),
        input("blue", 300_000),
    ];
    let report = host
        .run_body_plan_with_fore_and_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            true,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert!(!capture.frames.is_empty());
    for frame in &capture.frames {
        assert_eq!(frame.fragment().properties.len(), 3);
        assert_eq!(frame.scope().body_play(), &report.play);
    }
    // Typed sample-index/normalized semantics only; no physical range/UTC claimed.
    assert!(capture
        .frames
        .iter()
        .all(|frame| frame.scope().is_retired()));
}
