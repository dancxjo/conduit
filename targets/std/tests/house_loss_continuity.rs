use conduit_body::{
    AuthenticatedHostObservation, Body, BodyFormPlan, BodyMembership, BodyPlan, BodyPlayIdentity,
    HostPresenceClock, HostPresenceClockScale, HostPresenceState, HostPresenceTable,
    MembershipProofId, PartId, ResidentForm,
};
use conduit_core::{
    BootId, HostId, LinkBindingId, ObservationKind, OfferGeneration, SignId, TerminalDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, StdHostConfig, ThreadTimer};

const GREET: &str = include_str!("../../../forms/greet/main.conduit");

fn host(role: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(format!("host/rosehip-{role}")),
        boot_id: BootId::from(format!("boot/rosehip-{role}")),
        offer_generation: OfferGeneration(1),
    })
}

fn planned_greet(host: &StdHost, entry: &str) -> (ResidentForm, conduit_core::Plan) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(GREET);
    assert_eq!(syntax.round_trip(), GREET);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, entry, &profiles).unwrap();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    (
        ResidentForm::new(
            plan.source_document_id.clone(),
            plan.checked_form_id.clone(),
        ),
        plan,
    )
}

fn attach(
    membership: &mut BodyMembership,
    body_id: &conduit_body::BodyId,
    role: &str,
    host: &StdHost,
) -> PartId {
    let part = PartId::bind(body_id, &format!("part/rosehip-{role}"), 1).unwrap();
    membership
        .admit(
            body_id,
            membership.revision,
            part.clone(),
            MembershipProofId::bind(&format!("proof/rosehip-{role}")).unwrap(),
            SignId::from(format!("sign/rosehip-{role}-admitted")),
        )
        .unwrap();
    let advertisement = host.advertisement();
    membership
        .observe_present(
            body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                proof_id: MembershipProofId::bind(&format!("proof/rosehip-{role}-host")).unwrap(),
                sequence: 1,
            },
            SignId::from(format!("sign/rosehip-{role}-present")),
        )
        .unwrap();
    part
}

#[test]
fn one_house_keeps_unrelated_form_running_after_another_host_is_lost() {
    let lost_host = host("conversation");
    let mut continuing_host = host("status");
    let (conversation, conversation_plan) = planned_greet(&lost_host, "welcome");
    let (status, status_plan) = planned_greet(&continuing_host, "default-welcome");
    let body = Body::born(
        conversation.source_document_id.clone(),
        conversation.checked_form_id.clone(),
        1,
        SignId::from("sign/rosehip-born"),
    )
    .unwrap()
    .admit_form(status.clone(), SignId::from("sign/rosehip-status-admitted"))
    .unwrap();
    let body_id = body.body_id.clone();
    let wake = body.wake(1, SignId::from("sign/rosehip-wake")).unwrap().1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![
            BodyFormPlan {
                form: conversation,
                plan: conversation_plan,
            },
            BodyFormPlan {
                form: status,
                plan: status_plan.clone(),
            },
        ],
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&body_plan, 1);
    let playing = wake
        .body_plan_ready(&body_plan, SignId::from("sign/rosehip-plan-ready"))
        .unwrap()
        .body_play_started(&body_plan, &play, SignId::from("sign/rosehip-play-started"))
        .unwrap();

    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    let lost_part = attach(&mut membership, &body_id, "conversation", &lost_host);
    let continuing_part = attach(&mut membership, &body_id, "status", &continuing_host);
    let clock = HostPresenceClock::new(
        "clock/rosehip-presence".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let mut presence = HostPresenceTable::new(body_id.clone(), clock, 1_000).unwrap();
    for (part, role) in [(&lost_part, "conversation"), (&continuing_part, "status")] {
        presence
            .start(
                &membership,
                part,
                LinkBindingId::from(format!("line/rosehip-{role}")),
                1,
                1,
                100,
                SignId::from(format!("sign/rosehip-{role}-lease")),
            )
            .unwrap();
    }
    presence
        .lose_session(
            &mut membership,
            &lost_part,
            &LinkBindingId::from("line/rosehip-conversation"),
            2,
            SignId::from("sign/rosehip-conversation-lost"),
        )
        .unwrap();

    assert_eq!(playing.body_id, body_id);
    assert_eq!(membership.body_id, body_id);
    assert_eq!(presence.body_id, body_id);
    assert_eq!(presence.leases[0].state, HostPresenceState::Unavailable);
    assert_eq!(presence.leases[1].state, HostPresenceState::Available);
    assert!(membership.parts[0].current.is_none());
    assert!(membership.parts[1].current.is_some());
    assert_eq!(body_plan.forms.len(), 2);

    let mut output = Vec::with_capacity(4_096);
    let report = continuing_host
        .run_fragment_to(
            status_plan.fragments[0].clone(),
            &mut output,
            &mut ThreadTimer,
        )
        .expect("unrelated status Form remains executable on its available Host");
    assert!(String::from_utf8(output).unwrap().contains("HelloTravis"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
}
