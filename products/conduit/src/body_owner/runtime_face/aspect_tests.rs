//! Actual owner retention of two ordinary ambient facet routes.
use super::*;

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
    photo_checked: bool,
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
                let play = owner.session.realization().unwrap().play.clone();
                let requests = owner
                    .runtime_face
                    .each_ref()
                    .map(|s| s.as_ref().map(|f| f.request()));
                for intent in [
                    "conduit.intent/open-plot-library@1",
                    "conduit.intent/open-body-overview@1",
                ] {
                    let current = owner.local_face_snapshot().unwrap();
                    let action = current.actions.iter().find(|a| a.intent == intent).unwrap();
                    let show = crate::mask_test_common::available_mask_show(&current);
                    let interaction = conduit_face::FaceInteraction::new(
                        &current,
                        &show,
                        &action.identity,
                        &action.target,
                        vec![],
                        1,
                    )
                    .unwrap();
                    assert!(
                        owner
                            .navigate_owner_context(&show, &interaction)
                            .unwrap()
                            .is_some()
                    );
                    let updated = owner.local_face_snapshot().unwrap();
                    assert!(updated.subjects.iter().any(|s| s.name == "Wallpaper"));
                    assert!(
                        updated
                            .subjects
                            .iter()
                            .any(|s| s.name == "Clock and weather")
                    );
                    assert_eq!(
                        owner
                            .runtime_face
                            .each_ref()
                            .map(|s| s.as_ref().map(|f| f.request())),
                        requests
                    );
                    assert_eq!(owner.session.realization().unwrap().play, play);
                    assert!(
                        owner.navigate_owner_context(&show, &interaction).is_err(),
                        "prior Show must be stale after context change"
                    );
                }
                self.navigation_checked = true;
            }
            let body = format!("body/{}", owner.session.evidence().body_id.as_str());
            for (name, kind) in [
                ("Wallpaper", conduit_face::FaceCompositionKind::Subordinate),
                (
                    "Clock and weather",
                    conduit_face::FaceCompositionKind::Associate,
                ),
            ] {
                let subject = face.subjects.iter().find(|s| s.name == name).unwrap();
                assert!(
                    face.composition.iter().any(|r| r.source == subject.identity
                        && r.target == body
                        && r.kind == kind)
                );
            }
            if let Some(credit)=face.properties.iter().find(|p|p.name=="attribution" && matches!(&p.value,conduit_face::FacePropertyValue::Text(text) if text=="Fixture photographer credit")) {
                let outline=conduit_std_host::spoken_face_mask::select_spoken_outline(&face).unwrap();
                assert!(!outline.clauses.iter().any(|text|text.contains("Fixture photographer credit")));
                let mut reader=conduit_face::FaceReadingCursor::new(&face).unwrap();
                reader.command(&face,conduit_face::FaceReadingCommand::FocusSubject(credit.subject.clone())).unwrap();
                let mut heard=false;
                for _ in 0..reader.clause_count() {
                    if reader.next_read_clause(&face).unwrap().is_some_and(|c|c.text.contains("Fixture photographer credit")) {heard=true;break;}
                    reader.command(&face,conduit_face::FaceReadingCommand::Next).unwrap();
                }
                assert!(heard,"explicit focus retains full photograph credit");self.photo_checked=true;
            }
        }
        if active_count == 1 && self.accepted.iter().any(|count| *count == 3) && self.both > 0 {
            self.retired_one = true;
        }
        self.last = Some(owner.local_face_snapshot().unwrap());
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
    exercise_owner_aspects(false);
}
#[test]
fn owner_wallpaper_is_subordinate_in_speech_but_explicit_focus_retains_credit() {
    exercise_owner_aspects(true);
}
#[path = "aspect_fixture.rs"]
mod aspect_fixture;
fn exercise_owner_aspects(photos: bool) {
    let host = ambient_image_host();
    let source = crate::plot_source::parse(
        &include_str!("../../../../../plots/ambient-home/aspects.conduit")
            .replace("1440", "3")
            .replace("60000", "0"),
    )
    .unwrap();
    let plot = source
        .expand_named_for_authoring(if photos {
            "ambient-aspects"
        } else {
            "ambient-observed-aspects"
        })
        .unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident.clone(), None, "Shared ambient display").unwrap();
    let boundaries = if photos {
        BTreeMap::from([(
            resident,
            BTreeMap::from([(
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Input,
                    front_port_id: "observations".into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 8192,
                },
            )]),
        )])
    } else {
        BTreeMap::new()
    };
    owner
        .plan_workset_with_source_boundaries(&[(&source, &plot)], &boundaries)
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
        photo_checked: false,
    };
    let inputs = if photos {
        (0..3).map(|_| aspect_fixture::input()).collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &realization.wake,
                plan: &realization.plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            photos,
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
    assert_eq!(observer.photo_checked, photos);
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
