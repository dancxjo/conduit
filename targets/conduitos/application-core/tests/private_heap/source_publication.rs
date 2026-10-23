//! Complete software-native Source Face/Collection Show in the real fixed grant.
//! Fixture is an actual initialized workspace capture, not QEMU acknowledgement.
use super::*;
use conduit_face::{Face, FaceContributionBasis, FacePropertyValue, MaskShow};
#[derive(serde::Deserialize)]
struct Fixture {
    face: Face,
    show: MaskShow,
    graphs: Vec<patchbay_graph::PatchbayGraph>,
    topology: patchbay_application::PatchbayMaskTopology,
}
pub(super) fn run(original: &[Option<Initial>; 4], transfer: &mut [u8]) {
    let (applications, fixture) = on_caller(|| {
        let fixture: Fixture =
            data::decode(include_bytes!("../fixtures/source-publication.catz")).unwrap();
        let mut applications = original.clone();
        // Current native placements admit Tour and Patchbay only. The retained
        // historical biography fixture is a separate aggregate scenario above.
        for application in &mut applications {
            if matches!(application, Some(Initial::Tutorial { .. })) {
                *application = None;
            }
        }
        let Some(Initial::Patchbay { targets, selected }) = &mut applications[0] else {
            unreachable!()
        };
        for (index, graph) in fixture.graphs.iter().enumerate() {
            let subject = format!("plot/{}", graph.checked_plot_id.as_str());
            let plan = fixture
                .face
                .properties
                .iter()
                .find(|p| p.subject == subject && p.name == "observed-plan-id")
                .map(|p| match &p.value {
                    FacePropertyValue::Identity(value) => value.clone().into(),
                    _ => unreachable!(),
                });
            targets[index] = Some(data::Target {
                graph: graph.clone(),
                plan,
                body: fixture.topology.body_plan_id.clone(),
            });
        }
        *selected = 0;
        (applications, fixture)
    });
    STAGE.store(80, Ordering::Relaxed);
    let mut state = State::default();
    initialize::initialize(&mut state, &applications, transfer);
    // Reproduce actual actor revision2 with one ordinary selection before the
    // canonical protocol, retaining the same four prepared graphs.
    let Reply::Applied { snapshot, .. } = exchange(
        &mut state,
        &Command::Apply {
            slot: 0,
            input: Vec::new(),
        },
        transfer,
    ) else {
        panic!("initial view")
    };
    let select = on_caller(|| {
        let view = conduit_face::ApplicationView::decode(&snapshot.view).unwrap();
        conduit_face::ApplicationEvent {
            revision: view.revision,
            action: "patchbay.plot.2".into(),
            kind: conduit_face::ApplicationEventKind::Activate,
            value: Vec::new(),
        }
        .encode(&view)
        .unwrap()
    });
    drop(snapshot);
    assert!(matches!(
        exchange(
            &mut state,
            &Command::Apply {
                slot: 0,
                input: select
            },
            transfer
        ),
        Reply::Applied { .. }
    ));
    assert!(matches!(
        exchange(
            &mut state,
            &Command::PatchbayCanonical { slot: 0 },
            transfer
        ),
        Reply::Prepared
    ));
    assert!(matches!(
        exchange(
            &mut state,
            &Command::Topology {
                topology: fixture.topology
            },
            transfer
        ),
        Reply::Updated { .. }
    ));
    let publish = on_caller(|| Command::PatchbayPublish {
        slot: 0,
        body: fixture.face.basis.body_id.clone().unwrap(),
        basis: FaceContributionBasis {
            checked_plot_id: fixture.face.basis.checked_plot_id.clone().unwrap(),
            plan_id: fixture.face.basis.plan_id.clone().unwrap(),
            active_play_id: fixture.face.basis.active_play_id.clone().unwrap(),
            required_interaction_context: None,
        },
        face: fixture.face,
        show: fixture.show,
    });
    let consume = on_caller(|| {
        let Command::PatchbayPublish { face, show, .. } = &publish else {
            unreachable!()
        };
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == "patchbay.plot.0")
            .unwrap();
        Command::PatchbayConsume {
            slot: 0,
            input: conduit_face::FaceInteraction::new(
                face,
                show,
                &action.identity,
                &action.target,
                Vec::new(),
                1,
            )
            .unwrap()
            .encode(),
        }
    });
    STAGE.store(81, Ordering::Relaxed);
    assert!(matches!(
        exchange(&mut state, &publish, transfer),
        Reply::PatchbayPublished { .. }
    ));
    STAGE.store(82, Ordering::Relaxed);
    assert!(matches!(
        exchange(&mut state, &consume, transfer),
        Reply::PatchbayConsumed {
            target: 0,
            refusal: None,
            ..
        }
    ));
    report(
        "full Collection Source publication peak=",
        PEAK.load(Ordering::Relaxed),
    );
    drop(state);
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    assert!(!unsafe { private_heap::exhausted() });
}
