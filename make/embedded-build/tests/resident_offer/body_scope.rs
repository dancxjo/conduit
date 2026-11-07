//! One actual Body Play contains both independently projected Plot fragments.
use conduit_body::{
    Body, BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, BodyPlayIdentity,
    BodyPlotPlan, BodyWorkset, ResidentPlot,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope, body_fragment_scope::PreparedBodyFragment,
    selected_assigned_expression::ResidentExpressionBindingRefusal as E,
};

pub fn run() {
    let mut library = crate::library::Initialized::prepare().unwrap();
    let first =
        crate::support::exercise(crate::library_checks::decode(library.offers()[0].unwrap()));
    let second =
        crate::support::exercise(crate::library_checks::decode(library.offers()[1].unwrap()));
    let partitions = [&first, &second].map(|a| BodyPlotPlan {
        plot: ResidentPlot::new(
            a.model.source_document_id.clone(),
            a.model.checked_plot_id.clone(),
        ),
        plan: a.model.clone(),
    });
    let workset = BodyWorkset::from_plots(partitions.iter().map(|p| p.plot.clone())).unwrap();
    let body = Body::born_with_plots(workset, 1, "fixture/born".into()).unwrap();
    let wake = body.wake(1, "fixture/woke".into()).unwrap().1;
    let plan = BodyPlan::seal(&wake, partitions.into()).unwrap();
    let play = BodyPlayIdentity::bind(&plan, 1);
    let first_fragment = &first.model.fragments[0];
    let second_fragment = &second.model.fragments[0];
    assert_ne!(first.model.plan_id, plan.plan_id);
    let a = PreparedBodyFragment::prepare(&wake, &plan, &play, first_fragment).unwrap();
    let b = PreparedBodyFragment::prepare(&wake, &plan, &play, second_fragment).unwrap();
    assert_eq!(a.play().active_play_id, b.play().active_play_id);
    assert_eq!(a.plan().body_id, body.body_id);
    assert_eq!(a.play().wake_id, wake.wake_id);
    // Body validation is preparation. Only use of the borrowed checked witness
    // is allocation-free; this makes no MCU preparation-workspace claim.
    let ((), cost) = crate::probe::observe(|| {
        assert!(matches!(
            library.borrow_selected([
                Some(first.selected_scope(AssignedPlayScope::Body(b))),
                Some(second.selected_scope(AssignedPlayScope::Body(a))),
            ]),
            Err(E::Scope)
        ));
        assert!(library.offers().iter().all(Option::is_some));
        let (first_back, second_back) = library
            .borrow_selected([
                Some(first.selected_scope(AssignedPlayScope::Body(a))),
                Some(second.selected_scope(AssignedPlayScope::Body(b))),
            ])
            .unwrap();
        let (_, first_back) = first_back.unwrap();
        let (_, second_back) = second_back.unwrap();
        let first_back = crate::combined::retire(first_back, &first);
        let second_back = crate::combined::retire(second_back, &second);
        assert!(first_back.finish_retired().is_ok());
        assert!(second_back.finish_retired().is_ok());
        assert!(library.offers().iter().all(Option::is_some));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let mut stale_play = play.clone();
    stale_play.play_sequence += 1;
    assert!(PreparedBodyFragment::prepare(&wake, &plan, &stale_play, first_fragment).is_err());
    let mut stale_wake = wake.clone();
    stale_wake.workload_revision += 1;
    assert!(PreparedBodyFragment::prepare(&stale_wake, &plan, &play, first_fragment).is_err());
    let mut false_plan = plan.clone();
    false_plan.plan_id = first.model.plan_id.clone();
    assert!(PreparedBodyFragment::prepare(&wake, &false_plan, &play, first_fragment).is_err());
    for change in 0..3 {
        let mut substituted = first_fragment.clone();
        match change {
            0 => substituted.host_id = "fixture/another-host".into(),
            1 => substituted.boot_id = "fixture/another-boot".into(),
            _ => substituted.offer_generation.0 += 1,
        }
        assert!(PreparedBodyFragment::prepare(&wake, &plan, &play, &substituted).is_err());
    }
    // Copying a Body label into the old Plot identity form remains invalid.
    // A checked containment witness is required; no synthetic child Play starts.
    let false_plot_scope = conduit_core::ActivePlayIdentity {
        plan_id: plan.plan_id.clone(),
        active_play_id: play.active_play_id.clone(),
        host_id: first_fragment.host_id.clone(),
        boot_id: first_fragment.boot_id.clone(),
        play_sequence: play.play_sequence,
    };
    assert!(matches!(
        library.borrow_entry_0(first.selected(&false_plot_scope)),
        Err(E::Scope)
    ));
    assert!(library.offers().iter().all(Option::is_some));
    let plot = ResidentPlot::new(
        first.model.source_document_id.clone(),
        first.model.checked_plot_id.clone(),
    );
    let mask_body = Body::born_with_plots(
        BodyWorkset::from_plots([plot.clone()]).unwrap(),
        2,
        "fixture/mask-born".into(),
    )
    .unwrap();
    let mask_wake = mask_body.wake(2, "fixture/mask-woke".into()).unwrap().1;
    let topology = |model: &conduit_core::Plan| BodyMaskTopology {
        face: BodyFaceSelector {
            plot: None,
            source_placement_id: None,
        },
        chains: vec![BodyMaskChainPlan {
            plan: model.clone(),
            stage_placement_ids: vec![model.fragments[0].placements[0].placement_id.clone()],
        }],
    };
    let mask_plan = BodyPlan::seal_with_masks(
        &mask_wake,
        vec![BodyPlotPlan {
            plot: plot.clone(),
            plan: first.model.clone(),
        }],
        vec![topology(&second.model)],
    )
    .unwrap();
    let mask_play = BodyPlayIdentity::bind(&mask_plan, 2);
    let mask_scope =
        PreparedBodyFragment::prepare(&mask_wake, &mask_plan, &mask_play, second_fragment).unwrap();
    assert_eq!(mask_scope.plot_plan().plan_id, second.model.plan_id);
    crate::delegated::mask(mask_scope, &second); // Containment, not a renderer claim.
    let mask_plot_scope =
        PreparedBodyFragment::prepare(&mask_wake, &mask_plan, &mask_play, first_fragment).unwrap();
    crate::delegated::batch(mask_plot_scope, mask_scope, &first, &second);
    let duplicate = BodyPlan::seal_with_masks(
        &mask_wake,
        vec![BodyPlotPlan {
            plot,
            plan: first.model.clone(),
        }],
        vec![topology(&first.model)],
    )
    .unwrap();
    let duplicate_play = BodyPlayIdentity::bind(&duplicate, 2);
    assert!(
        PreparedBodyFragment::prepare(&mask_wake, &duplicate, &duplicate_play, first_fragment)
            .is_err()
    );
    let ambiguous =
        BodyPlan::seal_with_masks(&wake, plan.plots.clone(), vec![topology(&first.model)]).unwrap();
    let ambiguous_play = BodyPlayIdentity::bind(&ambiguous, 3);
    let unambiguous_anchor =
        PreparedBodyFragment::prepare(&wake, &ambiguous, &ambiguous_play, second_fragment).unwrap();
    let ((), iteration) = crate::probe::observe(|| {
        assert_eq!(
            unambiguous_anchor
                .local_fragments()
                .filter(Result::is_err)
                .count(),
            2
        );
        assert_eq!(
            unambiguous_anchor
                .local_fragments()
                .filter(Result::is_ok)
                .count(),
            1
        );
    });
    assert_eq!((iteration.allocations, iteration.reallocations), (0, 0));
    crate::delegated::run(a, &first);
    crate::delegated::batch(a, b, &first, &second);
    println!("two exact resident fragments bound to one checked Body Plan/Play; no child Play; zero scoped binding/retirement allocations; stale and substituted scope refused");
}
