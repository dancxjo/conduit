//! Real checked IMAGE/Body preparation and cancellation; no remote authority fixture.
use super::*;
use conduit_body::{Body, BodyPlotPlan, ResidentPlot};
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
fn fixture() -> (StdHost, Wake, BodyPlan) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let source = include_str!("../../../profiles/raspberry-pi-3-tick-workset.host.conduit");
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/delegated".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let host = StdHost::from_selected_timer_image(
        crate::StdHostConfig {
            host_id: "fixture/member".into(),
            boot_id: "boot/member".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "plot clock-demo {\nclock: time/tick(count = 1, period-ms = 0)\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let expanded = expand_canonical_plot(&checked, "clock-demo", &profile).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/delegated-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/delegated-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    (host, wake, plan)
}
#[test]
fn complete_preparation_does_not_start_and_cancellation_drains_before_release() {
    let (mut host, wake, plan) = fixture();
    let global = BodyPlayIdentity::bind(&plan, 97);
    let control = RunControl::default();
    let sequence = host.next_kernel_play_sequence;
    let mut timer = crate::ThreadTimer;
    let mut ticket = host
        .prepare_delegated_body(
            DelegatedBodyRequest {
                wake: &wake,
                plan: &plan,
                global_play: &global,
                control: &control,
            },
            &mut timer,
        )
        .unwrap();
    assert_eq!(ticket.receipt().global_play, global);
    assert_ne!(
        ticket.receipt().member_play.active_play_id,
        global.active_play_id
    );
    assert_eq!(host.next_kernel_play_sequence, sequence);
    let mut output = Vec::new();
    assert_eq!(
        host.poll_delegated_body(&mut ticket, &mut output, &mut timer, &control)
            .unwrap(),
        DelegatedBodyProgress::Prepared
    );
    assert!(host.release_delegated_body(&mut ticket).is_err());
    host.cancel_delegated_body(&mut ticket).unwrap();
    for _ in 0..32 {
        if host
            .poll_delegated_body(&mut ticket, &mut output, &mut timer, &control)
            .unwrap()
            == DelegatedBodyProgress::Terminal
        {
            break;
        }
    }
    assert!(matches!(
        ticket.terminal_receipt().unwrap().terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert!(output.is_empty());
    assert_eq!(host.next_kernel_play_sequence, sequence);
    host.release_delegated_body(&mut ticket).unwrap();
    assert!(host.kernel_resources.is_idle());
}
#[test]
fn foreign_global_identity_refuses_without_consuming_sequence_or_resources() {
    let (mut host, wake, plan) = fixture();
    let mut global = BodyPlayIdentity::bind(&plan, 97);
    global.wake_id = serde_json::from_str("\"wake/foreign\"").unwrap();
    let control = RunControl::default();
    let before = format!("{:?}", host.kernel_resources);
    let sequence = host.next_kernel_play_sequence;
    assert!(host
        .prepare_delegated_body(
            DelegatedBodyRequest {
                wake: &wake,
                plan: &plan,
                global_play: &global,
                control: &control
            },
            &mut crate::ThreadTimer
        )
        .is_err());
    assert_eq!(host.next_kernel_play_sequence, sequence);
    assert_eq!(format!("{:?}", host.kernel_resources), before);
}
#[test]
fn different_incarnation_cannot_cancel_or_release_original_preparation() {
    let (mut host, wake, plan) = fixture();
    let (mut other, _, _) = fixture();
    other.advertisement.boot_id = "boot/different".into();
    let global = BodyPlayIdentity::bind(&plan, 97);
    let control = RunControl::default();
    let mut ticket = host
        .prepare_delegated_body(
            DelegatedBodyRequest {
                wake: &wake,
                plan: &plan,
                global_play: &global,
                control: &control,
            },
            &mut crate::ThreadTimer,
        )
        .unwrap();
    assert!(other.cancel_delegated_body(&mut ticket).is_err());
    assert!(other.release_delegated_body(&mut ticket).is_err());
    assert!(!ticket.cancelled);
    assert!(ticket.terminal_receipt().is_none());
    host.cancel_delegated_body(&mut ticket).unwrap();
    let mut output = Vec::new();
    for _ in 0..32 {
        if host
            .poll_delegated_body(&mut ticket, &mut output, &mut crate::ThreadTimer, &control)
            .unwrap()
            == DelegatedBodyProgress::Terminal
        {
            break;
        }
    }
    host.release_delegated_body(&mut ticket).unwrap();
}

#[test]
fn authenticated_actual_preparation_advances_original_owner_once_without_local_kernel() {
    let (mut member, wake, plan) = fixture();
    let mut owner = StdHost::new_with_config(crate::StdHostConfig {
        host_id: "fixture/original-owner".into(),
        boot_id: "boot/original-owner".into(),
        offer_generation: OfferGeneration(1),
    });
    let global = owner.prospective_body_play(&wake, &plan).unwrap();
    let control = RunControl::default();
    let mut ticket = member
        .prepare_delegated_body(
            DelegatedBodyRequest {
                wake: &wake,
                plan: &plan,
                global_play: &global,
                control: &control,
            },
            &mut crate::ThreadTimer,
        )
        .unwrap();
    let owner_resources = format!("{:?}", owner.kernel_resources);
    let sequence = owner.next_kernel_play_sequence;
    let (play, started) = owner
        .acknowledge_delegated_body_preparation(&wake, &plan, ticket.receipt())
        .unwrap();
    assert_eq!(play, global);
    assert_eq!(owner.next_kernel_play_sequence, sequence + 1);
    let sign = |n| {
        bind_sign(
            &owner.advertisement.host_id,
            &owner.advertisement.boot_id,
            Some(&play.active_play_id),
            n,
        )
        .sign_id
    };
    let expected = wake
        .body_plan_ready_borrowed(&plan, sign(0))
        .unwrap()
        .body_play_started_borrowed(&plan, &play, sign(1))
        .unwrap();
    assert_eq!(started, expected);
    assert_eq!(format!("{:?}", owner.kernel_resources), owner_resources);
    assert!(owner
        .acknowledge_delegated_body_preparation(&wake, &plan, ticket.receipt())
        .is_err());
    assert_eq!(owner.next_kernel_play_sequence, sequence + 1);
    member.cancel_delegated_body(&mut ticket).unwrap();
    let mut output = Vec::new();
    for _ in 0..32 {
        if member
            .poll_delegated_body(&mut ticket, &mut output, &mut crate::ThreadTimer, &control)
            .unwrap()
            == DelegatedBodyProgress::Terminal
        {
            break;
        }
    }
    member.release_delegated_body(&mut ticket).unwrap();
}
