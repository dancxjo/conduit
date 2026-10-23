//! Four simultaneous targets in the unchanged real grant, including Vision.
//! The golden graph is the checked authoring projection of reviewed Vision,
//! Source SHA256 652ef750da420307d6676d866fc10e09075b5360157c4d671ab6dc9375c60485.
//! Native preparation separately checks that actual embedded Source and identity.
use super::*;

pub(super) fn run(applications: &[Option<Initial>; 4], transfer: &mut [u8]) {
    let applications = on_caller(|| {
        let mut applications = applications.clone();
        let Some(Initial::Patchbay { targets, selected }) = &mut applications[0] else {
            panic!("Patchbay fixture");
        };
        for index in 0..3 {
            targets[index] = targets[index + 1].take();
        }
        let graph: patchbay_graph::PatchbayGraph =
            serde_json::from_str(include_str!("../fixtures/vision-graph.json")).unwrap();
        assert_eq!(
            graph.source_document_id.as_str(),
            "5345dc154d1209167735c7d9e37a725c9a470a239fd7764e2986aacd58af4382"
        );
        assert_eq!(graph.cords.len(), 16);
        targets[3] = Some(data::Target {
            graph,
            plan: None,
            body: targets[0].as_ref().unwrap().body.clone(),
        });
        *selected = 0;
        applications
    });
    STAGE.store(70, Ordering::Relaxed);
    let mut state = State::default();
    initialize::initialize(&mut state, &applications, transfer);
    for target in [3u8, 0, 3] {
        STAGE.store(71 + usize::from(target), Ordering::Relaxed);
        assert!(matches!(
            exchange(&mut state, &Command::Select { slot: 0, target }, transfer),
            Reply::Updated { .. }
        ));
        let Reply::Graph { graph: Some(graph) } =
            exchange(&mut state, &Command::Graph { slot: 0 }, transfer)
        else {
            panic!("exact retained Source graph");
        };
        let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
            unreachable!()
        };
        assert_eq!(graph, targets[usize::from(target)].as_ref().unwrap().graph);
    }
    drop(state);
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
    assert!(!unsafe { private_heap::exhausted() });
}
