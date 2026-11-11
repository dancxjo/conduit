//! Authored Plot -> actual StdHost Body admission -> ordinary kernel Host Call.
use super::*;
use crate::{
    ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, StdHostConfig,
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, BodyWorkset, ResidentPlot};
use conduit_core::{
    ConnectionTrack, HostAdvertisement, Plan, PortDirection, SignId, StructuredInfoValue,
    TerminalDisposition, port_id,
};
use conduit_data::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use std::collections::BTreeMap;
fn plan(host: &HostAdvertisement) -> Result<Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_measurement_window_catalog(&mut startup, &mut profiles).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    let source = r#"plot projection (
 window: MeasurementWindow >> series: MeasurementPlotSeries
) {
 project: data/measurement-plot(points = 2, when-full = "evenly-spaced")
 window >> project.window
 project.series >> series
}"#;
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .map_err(|e| format!("{e:?}"))?;
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "projection", &profiles)
            .map_err(|e| e.to_string())?;
    let hosts = [host.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded.expanded, &hosts)
        .map_err(|e| e.to_string())?;
    let (input, output) = measurement_plot_value_bounds();
    let boundary = BTreeMap::from([
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("window"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("series"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: output,
            },
        ),
    ]);
    conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: input.max(output),
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary,
    )
    .map_err(|e| e.to_string())
}
fn input() -> Vec<u8> {
    use conduit_core::{Quantity, TemporalInstant, TemporalScale, Unit};
    let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
        capacity: 4,
        range: MeasurementRange {
            minimum: Quantity::new(0, Unit::Millivolt),
            maximum: Quantity::new(100, Unit::Millivolt),
        },
        clock_basis: "source/boot".into(),
        full_policy: FullWindowPolicy::Reject,
    })
    .unwrap();
    for index in 0..4 {
        window
            .push(MeasurementSample {
                value: Quantity::new(index * 25, Unit::Millivolt),
                observed_at: TemporalInstant {
                    ticks: (index + 1) as u64,
                    scale: TemporalScale::Milliseconds,
                    clock_basis: "source/boot".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                }
                .try_into()
                .unwrap(),
                uncertainty: None,
            })
            .unwrap();
    }
    StructuredInfoValue::leaf(
        measurement_window_type(),
        encode_measurement_window(&window).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Output {
    series: Option<MeasurementPlotSeries>,
}
impl BodyForeOutputAdapter for Output {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        assert_eq!(output.front_port_id, port_id("series"));
        assert_eq!(output.track, ConnectionTrack::Payload);
        assert_eq!(
            output.value_kind,
            measurement_plot_series_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
        let value = conduit_core::validate_canonical_structured_value(&output.bytes).unwrap();
        self.series = Some(
            decode_measurement_plot_series(
                value
                    .primitive_bytes(MEASUREMENT_PLOT_SERIES_INFO_ID)
                    .unwrap(),
            )
            .unwrap(),
        );
        Ok(())
    }
}
#[test]
fn authored_measurement_plot_runs_through_admitted_body_and_standard_host_dispatch() {
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/measurement".into(),
        boot_id: "boot/measurement".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let plan = plan(host.advertisement()).unwrap();
    let gear = &plan.fragments[0].placements[0];
    let workspace = measurement_plot_workspace_bytes();
    assert_eq!(gear.resources.len(), 1);
    assert_eq!(gear.resources[0].units, workspace);
    assert_eq!(
        gear.resources[0].class_id.as_str(),
        conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS
    );
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        BodyWorkset::from_plots([plot.clone()]).unwrap(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/wake")).unwrap().1;
    let body_plan = BodyPlan::seal(&wake, vec![BodyPlotPlan { plot, plan }]).unwrap();
    let inputs = [ExternalForeInput {
        front_port_id: port_id("window"),
        track: ConnectionTrack::Payload,
        bytes: input(),
    }];
    let mut output = Output { series: None };
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            false,
            &mut output,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
        )
        .unwrap();
    assert_eq!(
        report.terminal,
        TerminalDisposition::Completed,
        "{:?}",
        report.failure
    );
    assert_eq!(report.requests.len(), 1);
    let series = output.series.unwrap();
    assert_eq!(series.points().len(), 2);
    assert_eq!(series.source_samples(), 4);
    assert_eq!(series.omitted_samples(), 2);
    assert_eq!(*series.point(0).unwrap().source_index(), 0);
    assert_eq!(*series.point(1).unwrap().source_index(), 3);
}
#[test]
fn insufficient_actual_workspace_pool_refuses_projection_planning() {
    let host = StdHost::new();
    let admitted_plan = plan(host.advertisement()).unwrap();
    let mut advertisement = host.advertisement().clone();
    let pool = advertisement
        .resources
        .iter_mut()
        .find(|r| r.class_id.as_str() == conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS)
        .unwrap();
    pool.capacity_units = measurement_plot_workspace_bytes() - 1;
    assert!(plan(&advertisement).is_err());
    let mut actual_ledger =
        crate::kernel_preparation::KernelResourceLedger::new(&advertisement).unwrap();
    assert!(
        actual_ledger
            .prepare_and_reserve_plans(&advertisement, &[(&admitted_plan, false)])
            .is_err()
    );
}
