//! Real authored Plot and kernel commit reach the owner's ordinary shared Face.
//! Lifecycle persistence and selected IMAGE construction have separate proofs.
use super::*;
use conduit_body::ResidentPlot;
use conduit_core::{ConnectionTrack, OfferGeneration, PortDirection, StructuredInfoValue};
use conduit_data::{
    LabelledMeasurementSeries, MeasurementChart, MeasurementPlotPoint, MeasurementPlotSeries,
};
use conduit_std_host::{
    body_execution::BodyRunRequest, ExternalForeInput, RunControl, StdHost, StdHostComposition,
    StdHostConfig, ThreadTimer,
};
use std::{cell::RefCell, collections::BTreeMap};
const SOURCE: &str = concat!(
    include_str!("../../../../../plots/measurement-inspection/main.conduit"),
    r#"
plot inspection/charts (
    >> chart: MeasurementChart...|
) {
    view: measurement-inspection("Conditions", 2)
    chart >> view.chart
}"#
);
fn input(value: i64) -> ExternalForeInput {
    let series = MeasurementPlotSeries::from_projected(
        vec![
            MeasurementPlotPoint::new(0, 0, value).unwrap(),
            MeasurementPlotPoint::new(1, 1_000_000, value + 1).unwrap(),
        ],
        2,
        0,
    )
    .unwrap();
    let chart = MeasurementChart::new(vec![LabelledMeasurementSeries::new(
        "North room".into(),
        series,
    )
    .unwrap()])
    .unwrap();
    ExternalForeInput {
        front_port_id: "chart".into(),
        track: ConnectionTrack::Payload,
        bytes: StructuredInfoValue::leaf(
            conduit_data::measurement_chart_type(),
            conduit_data::encode_measurement_chart(&chart).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap(),
    }
}
struct NoOutput;
impl conduit_std_host::body_execution::BodyForeOutputAdapter for NoOutput {
    fn deliver(&mut self, _: &conduit_std_host::ExternalForeDelivery) -> Result<(), String> {
        Err("no declared output".into())
    }
}
struct Observe<'a> {
    owner: &'a RefCell<Owner>,
    sender: RuntimeFaceSender,
    receiver: RuntimeFaceReceiver,
    projections: Vec<Face>,
}
impl BodyFaceContributionObserver for Observe<'_> {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        self.sender.contribute(frame)?;
        let mut owner = self.owner.borrow_mut();
        self.receiver.progress(&mut owner).unwrap();
        let face = owner.local_face_snapshot().unwrap();
        assert!(face.properties.iter().any(|p| matches!(&p.value, conduit_face::FacePropertyValue::TypedValue {contract, ..} if contract.value_kind.as_str() == conduit_data::MEASUREMENT_PLOT_SERIES_INFO_ID)));
        self.projections.push(face);
        Ok(())
    }
}
#[test]
fn actual_committed_inspection_is_current_on_owner_and_withdrawn_on_lull() {
    let mut composition = StdHostComposition::minimal();
    composition.data = true;
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "fixture/runtime-face".into(),
            boot_id: "fixture/runtime-face/boot".into(),
            offer_generation: OfferGeneration(1),
        },
        composition,
    );
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident.clone(), None, "Shared inspection").unwrap();
    let boundaries = BTreeMap::from([(
        resident,
        BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: "chart".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_data::measurement_chart_canonical_bytes() as u32,
            },
        )]),
    )]);
    owner
        .plan_workset_with_source_boundaries(&[(&source, &plot)], &boundaries)
        .unwrap();
    let realization = owner.session.realization().unwrap().clone();
    let advertised = owner.host.advertisement().clone();
    let mut host = owner.host.take_for_play().unwrap();
    let owner = RefCell::new(owner);
    let (sender, receiver) = channel();
    let mut observer = Observe {
        owner: &owner,
        sender,
        receiver,
        projections: Vec::with_capacity(2),
    };
    let report = host
        .run_body_plan_with_fore_and_face_to_with_start(
            BodyRunRequest {
                wake: &realization.wake,
                plan: &realization.plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[input(100), input(200)],
            true,
            &mut NoOutput,
            &mut observer,
            &mut Vec::new(),
            &mut ThreadTimer,
            |play, wake| {
                owner
                    .borrow_mut()
                    .session
                    .started(
                        &advertised.host_id,
                        &advertised.boot_id,
                        play.clone(),
                        wake.clone(),
                    )
                    .map_err(|e| format!("{e:?}"))
            },
        )
        .unwrap();
    assert!(report.failure.is_none(), "{:?}", report.failure);
    assert_eq!(observer.projections.len(), 2);
    assert_ne!(observer.projections[0], observer.projections[1]);
    for face in &observer.projections {
        assert_eq!(face.basis.body_id.as_ref(), Some(&report.play.body_id));
        // The shared Body Face has no single producer Play. Preserve the
        // exact admitted producer scope in its ordinary provenance subject.
        assert!(face.properties.iter().any(|property| {
            property.name == "active-play-id"
                && property.value
                    == conduit_face::FacePropertyValue::Identity(
                        report.play.active_play_id.as_str().into(),
                    )
        }));
    }
    drop(observer);
    let mut owner = owner.into_inner();
    owner.host.restore_after_play(host).unwrap();
    owner.lull().unwrap();
    assert!(owner.runtime_face.is_none());
    assert!(owner.project_runtime_face().unwrap().is_none());
}
