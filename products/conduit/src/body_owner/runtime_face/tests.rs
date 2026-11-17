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
    assert!(owner.runtime_face.iter().all(Option::is_none));
    assert!(owner.project_runtime_face().unwrap().is_none());
}

fn ambient_image_host() -> StdHost {
    use conduit_host_make::*;
    let mut config = parse_host_configuration_conduit(include_str!(
        "../../../../../targets/std/profiles/raspberry-pi-5-measurement-cpu.host.conduit"
    ))
    .unwrap();
    config.target = ConfigurationTarget {
        architecture: std::env::consts::ARCH.into(),
        machine: "computer".into(),
        board: None,
        os: Some("linux".into()),
        make_descriptor: None,
    };
    config.capabilities = [
        ("time/tick", "std/kernel-time-tick@2"),
        ("ambient/home-observation", "std/ambient-clock-home@1"),
        ("face/ambient-wallpaper", "std/ambient-wallpaper@1"),
        ("face/ambient-clock-weather", "std/ambient-clock-weather@1"),
    ]
    .into_iter()
    .map(|(kind, implementation)| CapabilityIntent {
        kind: kind.into(),
        pinned_implementation: Some(implementation.into()),
    })
    .collect();
    let workspace = conduit_semantic_catalog::ambient_home_workspace_bytes().unwrap() * 2;
    config
        .bases
        .iter_mut()
        .find(|b| b.kind == "compute/cpu")
        .unwrap()
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("memory_bytes".into(), serde_json::json!(workspace));
    let memory = config
        .resources
        .iter_mut()
        .find(|r| r.id == "memory/measurement")
        .unwrap();
    memory.slots = workspace;
    memory.bytes = u64::from(workspace);
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(config, &catalog, &packages).unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/owner-ambient-facets".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_and_cpu_image(
        StdHostConfig {
            host_id: "fixture/owner-ambient".into(),
            boot_id: "fixture/owner-ambient/boot".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
        &checked,
    )
    .unwrap()
}
struct ObserveAspects<'a> {
    owner: &'a RefCell<Owner>,
    sender: RuntimeFaceSender,
    receiver: RuntimeFaceReceiver,
    deferred: bool,
    pressures: usize,
    accepted: [usize; 2],
    retirement_pressure: usize,
    last: Option<Face>,
    both: usize,
    retired_one: bool,
    navigation_checked: bool,
}
impl ObserveAspects<'_> {
    fn snapshot(&mut self) {
        let mut owner = self.owner.borrow_mut();
        self.receiver.progress(&mut owner).unwrap();
        let face = owner.local_face_snapshot().unwrap();
        assert_eq!(
            owner.local_face_snapshot().unwrap(),
            face,
            "reads must preserve revision"
        );
        if let Some(previous) = &self.last {
            if previous != &face {
                assert!(
                    face.revision > previous.revision,
                    "replacement and retirement must advance revision"
                );
            }
        }
        let active = owner
            .runtime_face
            .iter()
            .flatten()
            .filter(|f| owner.runtime_face_is_current(f))
            .collect::<Vec<_>>();
        let active_count = active.len();
        if active_count == 2 {
            assert_eq!(active[0].fragment().basis, active[1].fragment().basis);
        }
        drop(active);
        if active_count == 2 {
            assert!(face.subjects.iter().any(|s| s.name == "Wallpaper"));
            assert!(face.subjects.iter().any(|s| s.name == "Clock and weather"));
            assert_eq!(
                face.subjects
                    .iter()
                    .filter(|s| s.name == "Face contribution provenance")
                    .count(),
                1
            );
            face.validate().unwrap();
            self.both += 1;
            if !self.navigation_checked {
                let action = face
                    .actions
                    .iter()
                    .find(|a| a.intent == "conduit.intent/open-plot-library@1")
                    .unwrap();
                let show = crate::mask_test_common::available_mask_show(&face);
                let interaction = conduit_face::FaceInteraction::new(
                    &face,
                    &show,
                    &action.identity,
                    &action.target,
                    vec![],
                    1,
                )
                .unwrap();
                assert_eq!(
                    owner
                        .navigate_owner_context(&show, &interaction)
                        .unwrap_err(),
                    "owner navigation requires this contribution's current BodyFace routing"
                );
                assert_eq!(
                    owner.local_face_snapshot().unwrap(),
                    face,
                    "unsupported navigation must retain both contributions"
                );
                self.navigation_checked = true;
            }
        }
        if active_count == 1 && self.accepted.iter().any(|count| *count == 3) && self.both > 0 {
            self.retired_one = true;
        }
        self.last = Some(face);
    }
}
impl BodyFaceContributionObserver for ObserveAspects<'_> {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        let index = match frame.fragment().subjects[0].name.as_str() {
            "Wallpaper" => 0,
            "Clock and weather" => 1,
            _ => panic!("unexpected facet"),
        };
        // Hold the last sibling while the already accepted route can retire.
        // This is ordinary observer pressure, with the original token returned.
        if self.accepted[1 - index] == 3
            && self.accepted[index] == 2
            && self.retirement_pressure < 8
        {
            self.retirement_pressure += 1;
            self.snapshot();
            return Err(ReturnedFaceContribution {
                contribution: frame,
                refusal: FaceContributionRefusal::Pressure,
            });
        }
        match self.sender.contribute(frame) {
            Err(returned) => {
                assert_eq!(returned.refusal, FaceContributionRefusal::Pressure);
                self.pressures += 1;
                self.snapshot();
                Err(returned)
            }
            Ok(()) => {
                self.accepted[index] += 1;
                if !self.deferred {
                    self.deferred = true;
                } else {
                    self.snapshot();
                }
                Ok(())
            }
        }
    }
}
#[test]
fn owner_preserves_two_real_facet_routes_through_channel_pressure_replacement_and_retirement() {
    let host = ambient_image_host();
    let source = crate::plot_source::parse(
        &include_str!("../../../../../plots/ambient-home/aspects.conduit")
            .replace("1440", "3")
            .replace("60000", "0"),
    )
    .unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident, None, "Shared ambient display").unwrap();
    owner
        .plan_workset_with_source_boundaries(&[(&source, &plot)], &BTreeMap::new())
        .unwrap();
    let realization = owner.session.realization().unwrap().clone();
    let advertised = owner.host.advertisement().clone();
    let mut host = owner.host.take_for_play().unwrap();
    let owner = RefCell::new(owner);
    let (sender, receiver) = channel();
    let mut observer = ObserveAspects {
        owner: &owner,
        sender,
        receiver,
        deferred: false,
        pressures: 0,
        accepted: [0, 0],
        retirement_pressure: 0,
        last: None,
        both: 0,
        retired_one: false,
        navigation_checked: false,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &realization.wake,
                plan: &realization.plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            false,
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
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert_eq!(observer.accepted, [3, 3]);
    assert!(observer.pressures > 0);
    assert!(observer.both > 0);
    assert!(observer.navigation_checked);
    assert!(observer.retired_one);
    let last_revision = observer.last.as_ref().unwrap().revision;
    observer.receiver.progress(&mut owner.borrow_mut()).unwrap();
    assert!(owner.borrow().runtime_face.iter().all(Option::is_none));
    assert!(owner.borrow().project_runtime_face().unwrap().is_none());
    assert!(owner.borrow().local_face_snapshot().unwrap().revision > last_revision);
    drop(observer);
    drop(result);
    let mut owner = owner.into_inner();
    owner.host.restore_after_play(host).unwrap();
    owner.lull().unwrap();
    assert!(owner.runtime_face.iter().all(Option::is_none));
}

#[test]
fn composition_preserves_one_parent_basis_and_rejects_mismatches_or_collisions() {
    let first = FaceFragment {
        basis: conduit_face::FaceContributionBasis {
            checked_plot_id: "checked/parent".into(),
            plan_id: "plan/body".into(),
            active_play_id: "play/body".into(),
            required_interaction_context: None,
        },
        subjects: vec![conduit_face::FaceSubject {
            identity: "facet/wallpaper".into(),
            role: conduit_face::FaceRole::Document,
            name: "Wallpaper".into(),
        }],
        relationships: vec![],
        composition: vec![],
        properties: vec![],
        text: vec![],
        actions: vec![],
        disclosures: vec![],
        temporal_references: vec![],
        temporal_facts: vec![],
    };
    let mut second = first.clone();
    second.subjects[0].identity = "facet/clock".into();
    let composed = compose_fragments(&first, Some(&second)).unwrap();
    assert_eq!(composed.basis, first.basis);
    assert_eq!(composed.subjects.len(), 2);
    assert!(compose_fragments(&first, Some(&first)).is_err());
    second.basis.checked_plot_id = "checked/other".into();
    assert!(compose_fragments(&first, Some(&second)).is_err());
}
