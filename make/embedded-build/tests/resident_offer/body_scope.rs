//! One actual Body Play contains both independently projected Plot fragments.
use conduit_body::{Body, BodyPlan, BodyPlayIdentity, BodyPlotPlan, BodyWorkset, ResidentPlot};
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
    println!("two exact resident fragments bound to one checked Body Plan/Play; no child Play; zero scoped binding/retirement allocations; stale and substituted scope refused");
}
