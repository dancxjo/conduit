use conduit_body::{
    Body, BodyLifecycleError, BodyPlan, BodyPlanError, BodyPlayIdentity, BodyPlotPlan,
    ResidentPlot, WakeLifecycle,
};
use conduit_core::{
    seal_plan, CheckedPlotId, ExpandedPlotId, Plan, PlanId, PlotIdentity, SignId, SourceDocumentId,
};

fn resident(name: &str) -> ResidentPlot {
    ResidentPlot::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedPlotId::from(format!("checked/{name}")),
    )
}

fn plan(plot: &ResidentPlot, expansion: &str) -> Plan {
    seal_plan(
        PlotIdentity {
            source_document_id: plot.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            expanded_plot_id: ExpandedPlotId::from(format!("expanded/{expansion}")),
        },
        vec![],
    )
}

fn two_plot_wake() -> conduit_body::Wake {
    let body = Body::born_with_plots(
        conduit_body::BodyWorkset::from_plots([resident("dashboard"), resident("service")])
            .unwrap(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    body.wake(1, SignId::from("sign/woke")).unwrap().1
}

#[test]
fn one_body_plan_and_one_play_cover_two_exact_plots() {
    let wake = two_plot_wake();
    let dashboard = resident("dashboard");
    let service = resident("service");
    let body_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyPlotPlan {
                plot: service.clone(),
                plan: plan(&service, "service"),
            },
            BodyPlotPlan {
                plot: dashboard.clone(),
                plan: plan(&dashboard, "dashboard"),
            },
        ],
    )
    .unwrap();
    assert_eq!(body_plan.plots[0].plot, dashboard);
    assert_eq!(body_plan.plots[1].plot, service);
    assert_eq!(body_plan.verify_seal(), Ok(()));
    let mut forged = body_plan.clone();
    forged.plan_id = PlanId::from("body-plan/forged");
    assert_eq!(forged.verify_seal(), Err(BodyPlanError::InvalidIdentity));

    let waiting = wake
        .body_plan_ready(&body_plan, SignId::from("sign/planned"))
        .unwrap();
    let play = BodyPlayIdentity::bind(&body_plan, 1);
    let playing = waiting
        .body_play_started(&body_plan, &play, SignId::from("sign/playing"))
        .unwrap();
    assert_eq!(playing.lifecycle, WakeLifecycle::Playing);
    assert_eq!(playing.plans.len(), 1);
    assert_eq!(playing.plans[0].plan_id, body_plan.plan_id);
    assert_eq!(playing.plans[0].active_play_id, Some(play.active_play_id));

    let second = BodyPlayIdentity::bind(&body_plan, 2);
    assert_eq!(
        playing.body_play_started(&body_plan, &second, SignId::from("sign/parallel")),
        Err(BodyLifecycleError::InvalidTransition)
    );
}

#[test]
fn body_plan_requires_the_complete_current_workset_exactly_once() {
    let wake = two_plot_wake();
    let dashboard = resident("dashboard");
    let service = resident("service");
    let dashboard_partition = BodyPlotPlan {
        plot: dashboard.clone(),
        plan: plan(&dashboard, "dashboard"),
    };
    assert_eq!(
        BodyPlan::seal(&wake, vec![dashboard_partition.clone()]),
        Err(BodyPlanError::MissingPlot)
    );
    assert_eq!(
        BodyPlan::seal(
            &wake,
            vec![dashboard_partition.clone(), dashboard_partition]
        ),
        Err(BodyPlanError::DuplicatePlot)
    );
    let unowned = resident("unowned");
    assert_eq!(
        BodyPlan::seal(
            &wake,
            vec![
                BodyPlotPlan {
                    plot: dashboard.clone(),
                    plan: plan(&dashboard, "dashboard"),
                },
                BodyPlotPlan {
                    plot: unowned.clone(),
                    plan: plan(&unowned, "unowned"),
                },
            ],
        ),
        Err(BodyPlanError::UnexpectedPlot)
    );

    let only_service = Body::born(
        service.source_document_id.clone(),
        service.checked_plot_id.clone(),
        9,
        SignId::from("sign/other-born"),
    )
    .unwrap()
    .wake(1, SignId::from("sign/other-woke"))
    .unwrap()
    .1;
    let stale = BodyPlan::seal(
        &only_service,
        vec![BodyPlotPlan {
            plot: service.clone(),
            plan: plan(&service, "service"),
        }],
    )
    .unwrap();
    assert_eq!(stale.validate_for(&wake), Err(BodyPlanError::WrongBody));
}

#[test]
fn legacy_single_plan_validation_uses_current_workset_not_seed_provenance() {
    let seed = resident("seed");
    let replacement = resident("replacement");
    let body = Body::born(
        seed.source_document_id,
        seed.checked_plot_id,
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .remove_plot(&resident("seed"), SignId::from("sign/remove-seed"))
    .unwrap()
    .admit_plot(replacement.clone(), SignId::from("sign/add-replacement"))
    .unwrap();
    let wake = body.wake(1, SignId::from("sign/woke")).unwrap().1;
    wake.plan_ready(
        &plan(&replacement, "replacement"),
        SignId::from("sign/planned"),
    )
    .unwrap();
}

#[test]
fn workload_change_replaces_the_plan_and_play_without_replacing_the_wake() {
    let seed = resident("dashboard");
    let service = resident("service");
    let (awake_body, wake) = Body::born(
        seed.source_document_id.clone(),
        seed.checked_plot_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .admit_plot(service.clone(), SignId::from("sign/admit-service"))
    .unwrap()
    .wake(1, SignId::from("sign/woke"))
    .unwrap();
    let initial_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyPlotPlan {
                plot: seed.clone(),
                plan: plan(&seed, "dashboard"),
            },
            BodyPlotPlan {
                plot: service.clone(),
                plan: plan(&service, "service"),
            },
        ],
    )
    .unwrap();
    let initial_play = BodyPlayIdentity::bind(&initial_plan, 1);
    let playing = wake
        .body_plan_ready(&initial_plan, SignId::from("sign/initial-plan"))
        .unwrap()
        .body_play_started(
            &initial_plan,
            &initial_play,
            SignId::from("sign/initial-play"),
        )
        .unwrap();

    let recorder = resident("recorder");
    let changed_body = awake_body
        .admit_plot(recorder.clone(), SignId::from("sign/admit-recorder"))
        .unwrap();
    let changed = playing
        .workload_changed(&changed_body, SignId::from("sign/workload-changed"))
        .unwrap();
    assert_eq!(changed.wake_id, wake.wake_id);
    assert_eq!(changed.lifecycle, WakeLifecycle::Unsatisfied);
    assert_eq!(
        initial_plan.validate_for(&changed),
        Err(BodyPlanError::StaleWorkload)
    );

    let replacement = BodyPlan::seal(
        &changed,
        vec![
            BodyPlotPlan {
                plot: seed.clone(),
                plan: plan(&seed, "dashboard-2"),
            },
            BodyPlotPlan {
                plot: service.clone(),
                plan: plan(&service, "service-2"),
            },
            BodyPlotPlan {
                plot: recorder.clone(),
                plan: plan(&recorder, "recorder"),
            },
        ],
    )
    .unwrap();
    let replacement_play = BodyPlayIdentity::bind(&replacement, 2);
    let replaced = changed
        .body_plan_ready(&replacement, SignId::from("sign/replacement-plan"))
        .unwrap()
        .body_play_started(
            &replacement,
            &replacement_play,
            SignId::from("sign/replacement-play"),
        )
        .unwrap();
    assert_eq!(replaced.wake_id, wake.wake_id);
    assert_eq!(replaced.lifecycle, WakeLifecycle::Playing);
    assert_eq!(replaced.plans.len(), 2);
    assert_eq!(
        replaced.plans[0].state,
        conduit_body::WakePlanState::Superseded
    );
    assert_eq!(
        replaced.plans[1].state,
        conduit_body::WakePlanState::Playing
    );
}
