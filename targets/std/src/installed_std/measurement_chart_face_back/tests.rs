//! Authored bounded Flow -> admitted ordinary Body -> actual kernel commit ->
//! opaque renderer-neutral contribution. No physical producer or Mask is claimed.
use super::*;
use crate::{
    body_execution::{
        BodyFaceContributionObserver, BodyForeOutputAdapter, BodyRunRequest,
        CommittedFaceContribution, FaceContributionRefusal, ReturnedFaceContribution,
    },
    ExternalForeDelivery, ExternalForeInput, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot, Wake};
use conduit_core::{ConnectionTrack, HostAdvertisement, Plan, PortDirection, StructuredInfoValue};
use conduit_data::*;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use std::collections::BTreeMap;
const SOURCE: &str = concat!(
    include_str!("../../../../../plots/measurement-inspection/main.conduit"),
    r#"
plot inspection/charts (
    >> chart: MeasurementChart...|
) {
    view: measurement-inspection("Conditions", 2)
    chart >> view.chart
}"#,
);
pub(super) fn planned(host: &HostAdvertisement) -> (BodyPlan, Wake) {
    planned_source(host, SOURCE)
}
pub(super) fn planned_for_update_bound(host: &HostAdvertisement, maximum: u32) -> (BodyPlan, Wake) {
    assert!((1..=4096).contains(&maximum));
    planned_source(
        host,
        &SOURCE.replace(
            "measurement-inspection(\"Conditions\", 2)",
            &format!("measurement-inspection(\"Conditions\", {maximum})"),
        ),
    )
}
fn planned_source(host: &HostAdvertisement, source: &str) -> (BodyPlan, Wake) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profiles).unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "inspection/charts", &profiles)
            .unwrap();
    let hosts = [host.clone()];
    let selected =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundary = BTreeMap::from([(
        ForeBoundaryKey {
            direction: PortDirection::Input,
            front_port_id: "chart".into(),
            track: ConnectionTrack::Payload,
        },
        ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: measurement_chart_canonical_bytes() as u32,
        },
    )]);
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
pub(super) fn input(value: i64) -> ExternalForeInput {
    let entries = ["North", "South", "East", "West"]
        .into_iter()
        .map(|label| {
            let series = MeasurementPlotSeries::from_projected(
                vec![
                    MeasurementPlotPoint::new(0, 0, value).unwrap(),
                    MeasurementPlotPoint::new(1, 1_000_000, value + 1).unwrap(),
                ],
                2,
                0,
            )
            .unwrap();
            LabelledMeasurementSeries::new(label.into(), series).unwrap()
        })
        .collect::<Vec<_>>();
    let bytes = StructuredInfoValue::leaf(
        measurement_chart_type(),
        encode_measurement_chart(&MeasurementChart::new(entries).unwrap()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    ExternalForeInput {
        front_port_id: "chart".into(),
        track: ConnectionTrack::Payload,
        bytes,
    }
}
pub(super) struct Outputs;
impl BodyForeOutputAdapter for Outputs {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("inspection Plot declares no Fore output".into())
    }
}
pub(super) struct Capture {
    pub(super) frames: Vec<CommittedFaceContribution>,
    pub(super) pressure: usize,
    pub(super) final_pressure: usize,
    pub(super) attempts: usize,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        value: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        self.attempts += 1;
        let pressure = if value.request() == RequestId(1) {
            &mut self.final_pressure
        } else {
            &mut self.pressure
        };
        if *pressure > 0 {
            *pressure -= 1;
            return Err(ReturnedFaceContribution {
                contribution: value,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        if self.frames.len() == self.frames.capacity() {
            return Err(ReturnedFaceContribution {
                contribution: value,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        self.frames.push(value);
        Ok(())
    }
}
#[test]
fn authored_inspection_flow_publishes_two_distinct_committed_frames_in_one_play() {
    let mut composition = StdHostComposition::minimal();
    composition.data = true;
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "proof/chart-owner".into(),
            boot_id: "proof/chart-owner/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        composition,
    );
    let (plan, wake) = planned(host.advertisement());
    let mut capture = Capture {
        frames: Vec::with_capacity(2),
        pressure: 2,
        final_pressure: 3,
        attempts: 0,
    };
    let report = host
        .run_body_plan_with_fore_and_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[input(100), input(200)],
            true,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert!(report.cleanup_failure.is_none());
    assert!(report.kernel_failure.is_none());
    assert_eq!(capture.frames.len(), 2);
    assert!(capture.attempts >= 7);
    for (index, frame) in capture.frames.iter().enumerate() {
        assert_eq!(frame.request(), RequestId(index as u32));
        assert_eq!(frame.scope().body_play(), &report.play);
        assert_eq!(frame.scope().host(), &host.advertisement().host_id);
        assert_eq!(frame.scope().boot(), &host.advertisement().boot_id);
        assert_eq!(
            frame.scope().plot().checked_plot_id,
            plan.plots[0].plot.checked_plot_id
        );
        assert_eq!(frame.fragment().properties.len(), 4);
        assert_eq!(
            frame.fragment().basis.active_play_id,
            report.play.active_play_id
        );
    }
    assert_ne!(
        capture.frames[0].fragment().properties,
        capture.frames[1].fragment().properties
    );
}
