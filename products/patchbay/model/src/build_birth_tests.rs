use conduit_body::{
    BodyLifecycleEvent, BodyState, MembershipEventKind, MembershipRefusal, MembershipState,
    WakeLifecycle, WakeLifecycleEvent,
};
use conduit_core::{bind_active_play, CheckedFormId, SignId, SourceDocumentId};
use conduit_std_host::StdHost;

use crate::{BirthSigns, BuildBirthController, BuildBirthError, FormEditor, PatchbayMode};

fn editor(source: &str) -> FormEditor {
    FormEditor::from_source("build.conduit".into(), source.into()).unwrap()
}

fn planned_on_fresh_host(
    editor: &FormEditor,
) -> (conduit_core::Plan, conduit_core::ActivePlayIdentity) {
    let expanded = editor.expand_form(&editor.view().open_form).unwrap();
    let host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let fragment = plan.fragments.first().unwrap();
    let play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    (plan, play)
}

fn origin() -> conduit_core::HostAdvertisement {
    StdHost::new().advertisement().clone()
}

fn birth_signs(label: &str) -> BirthSigns {
    BirthSigns {
        body_born: SignId::from(format!("{label}/body").as_str()),
        part_admitted: SignId::from(format!("{label}/part").as_str()),
        host_attached: SignId::from(format!("{label}/host").as_str()),
    }
}

#[test]
fn build_birth_wake_replan_play_and_lull_use_exact_canonical_values() {
    let mut editor = editor(include_str!("../../../../forms/hello/main.conduit"));
    let mut lifecycle = BuildBirthController::new();
    let build = lifecycle.document(&editor).unwrap();
    assert_eq!(build.mode, PatchbayMode::FormOpened);
    assert_eq!(build.revisions.saved_revision, 0);
    assert_eq!(build.revisions.checked_revision, Some(0));

    editor
        .replace_source(format!(
            "{}\n",
            include_str!("../../../../forms/hello/main.conduit")
        ))
        .unwrap();
    assert_eq!(
        lifecycle.birth(&editor, &origin(), 7, birth_signs("build/born")),
        Err(BuildBirthError::UncheckedRevision)
    );
    editor.recheck().unwrap();
    lifecycle
        .birth(&editor, &origin(), 7, birth_signs("build/born"))
        .unwrap();
    let born_body = lifecycle.body().unwrap().clone();
    let (first_plan, first_play) = planned_on_fresh_host(&editor);
    let (replacement_plan, replacement_play) = planned_on_fresh_host(&editor);
    assert_ne!(first_plan.plan_id, replacement_plan.plan_id);
    assert_eq!(born_body.state, BodyState::Lulled);
    assert!(matches!(
        born_body.events.as_slice(),
        [BodyLifecycleEvent::Born { initial_workset, workload_revision: 0, sign_id }]
            if initial_workset == &born_body.workset && sign_id.as_str() == "build/born/body"
    ));
    assert!(lifecycle.wake_value().is_none());

    editor
        .replace_source(format!("{}\n\n", editor.view().source))
        .unwrap();
    editor.recheck().unwrap();
    assert_eq!(lifecycle.body(), Some(&born_body));

    lifecycle.wake(3, SignId::from("build/woke")).unwrap();
    lifecycle
        .plan_ready(&first_plan, SignId::from("build/planned-a"))
        .unwrap();
    lifecycle
        .play_started(&first_play, SignId::from("build/played-a"))
        .unwrap();
    lifecycle
        .became_unsatisfied(&first_plan.plan_id, SignId::from("build/unsatisfied"))
        .unwrap();
    lifecycle
        .plan_ready(&replacement_plan, SignId::from("build/planned-b"))
        .unwrap();
    lifecycle
        .play_started(&replacement_play, SignId::from("build/played-b"))
        .unwrap();

    let wake = lifecycle.wake_value().unwrap();
    assert_eq!(wake.lifecycle, WakeLifecycle::Playing);
    assert_eq!(wake.plans.len(), 2);
    assert_ne!(wake.plans[0].plan_id, wake.plans[1].plan_id);
    assert_ne!(wake.plans[0].active_play_id, wake.plans[1].active_play_id);
    assert!(wake.events.iter().any(|event| matches!(
        event,
        WakeLifecycleEvent::Replanned {
            prior_plan_id,
            replacement_plan_id,
            ..
        } if prior_plan_id == &first_plan.plan_id && replacement_plan_id == &replacement_plan.plan_id
    )));

    lifecycle
        .lull(
            SignId::from("build/lulled"),
            SignId::from("build/lull-retained"),
        )
        .unwrap();
    assert_eq!(lifecycle.body().unwrap().state, BodyState::Lulled);
    assert_eq!(
        lifecycle.wake_value().unwrap().lifecycle,
        WakeLifecycle::Lulled
    );
    let document = lifecycle.document(&editor).unwrap();
    assert_eq!(document.mode, PatchbayMode::BornLulled);
    assert_eq!(document.revisions.current_revision, 2);
    assert_eq!(document.revisions.saved_revision, 0);
    assert_eq!(document.revisions.checked_revision, Some(2));
    assert_eq!(document.revisions.born_revision, Some(1));
    let text = document.lines.join("\n");
    assert!(text.contains("last-born=1"));
    assert!(text.contains("PLAN "));
    assert!(text.contains("PLAY "));
    assert!(text.contains("Replanned"));
    assert!(text.contains("Lulled"));
}

#[test]
fn build_document_exposes_face_ports_and_info_without_planning() {
    let mut editor = editor(include_str!("../../../../forms/greet/main.conduit"));
    editor.open_back("welcome").unwrap();
    let document = BuildBirthController::new().document(&editor).unwrap();
    let text = document.lines.join("\n");
    assert!(text.contains("FORM greet"));
    assert!(text.contains("FACE inputs=1 outputs=1"));
    assert!(text.contains("PORT name direction=Input info=value/text@1 temporal=Value"));
    assert!(text.contains("PORT text direction=Output info=value/text@1 temporal=Value"));
    assert!(text.contains("kind=text/join"));
    assert!(text.contains("CORD "));
    assert!(text.contains("info=value/text@1"));
    assert!(text.contains("BODY not born"));
}

#[test]
fn invalid_duplicate_and_terminal_transitions_remain_distinct() {
    let mut invalid = editor("form broken {");
    let mut lifecycle = BuildBirthController::new();
    assert_eq!(
        lifecycle.birth(&invalid, &origin(), 0, birth_signs("invalid/born")),
        Err(BuildBirthError::UncheckedRevision)
    );
    assert!(lifecycle
        .document(&invalid)
        .unwrap()
        .lines
        .join("\n")
        .contains("unchecked"));

    invalid
        .replace_source(include_str!("../../../../forms/hello/main.conduit").into())
        .unwrap();
    invalid.recheck().unwrap();
    lifecycle
        .birth(&invalid, &origin(), 0, birth_signs("valid/born"))
        .unwrap();
    assert_eq!(
        lifecycle.birth(&invalid, &origin(), 1, birth_signs("duplicate/born")),
        Err(BuildBirthError::AlreadyBorn)
    );
    assert_eq!(
        lifecycle.plan_ready(
            &planned_on_fresh_host(&invalid).0,
            SignId::from("early/plan")
        ),
        Err(BuildBirthError::BodyNotAwake)
    );
    lifecycle.wake(0, SignId::from("valid/woke")).unwrap();
    lifecycle
        .fail_wake(
            SignId::from("wake/failed"),
            SignId::from("wake/failure-retained"),
        )
        .unwrap();
    assert_eq!(lifecycle.body().unwrap().state, BodyState::Lulled);
    assert_eq!(
        lifecycle.wake_value().unwrap().lifecycle,
        WakeLifecycle::Failed
    );
}

#[test]
fn birth_explicitly_admits_and_attaches_exactly_one_here_part() {
    let editor = editor(include_str!("../../../../forms/hello/main.conduit"));
    let origin = origin();
    let mut lifecycle = BuildBirthController::new();
    lifecycle
        .birth(&editor, &origin, 11, birth_signs("birth"))
        .unwrap();

    let body = lifecycle.body().unwrap();
    let membership = lifecycle.membership().unwrap();
    assert_eq!(membership.body_id, body.body_id);
    assert_eq!(membership.parts.len(), 1);
    assert_eq!(membership.events.len(), 2);
    let part = &membership.parts[0];
    let current = part.current.as_ref().unwrap();
    assert_eq!(part.state, MembershipState::Admitted);
    assert_eq!(current.host_id, origin.host_id);
    assert_eq!(current.boot_id, origin.boot_id);
    assert_eq!(current.offer_generation, origin.offer_generation);
    assert_ne!(part.part_id.as_str(), body.body_id.as_str());
    assert_ne!(part.part_id.as_str(), current.host_id.as_str());
    assert_ne!(current.host_id.as_str(), current.boot_id.as_str());
    assert!(matches!(
        membership.events[0].kind,
        MembershipEventKind::Admitted { .. }
    ));
    assert!(matches!(
        membership.events[1].kind,
        MembershipEventKind::HostAttached { .. }
    ));
    assert_ne!(membership.events[0].sign_id, membership.events[1].sign_id);

    let document = lifecycle.document(&editor).unwrap().lines.join("\n");
    assert!(document.contains("PARTS 1"));
    assert!(document.contains("This computer HERE AVAILABLE attached when BORN"));
    assert!(document.contains(origin.host_id.as_str()));
    assert!(document.contains(origin.boot_id.as_str()));
}

#[test]
fn host_loss_keeps_membership_but_drops_current_boot_and_wrong_body_refuses() {
    let editor = editor(include_str!("../../../../forms/hello/main.conduit"));
    let origin = origin();
    let mut lifecycle = BuildBirthController::new();
    lifecycle
        .birth(&editor, &origin, 12, birth_signs("birth"))
        .unwrap();
    let body_id = lifecycle.body().unwrap().body_id.clone();
    let wrong_body = conduit_body::Body::born(
        SourceDocumentId::from("source/wrong"),
        CheckedFormId::from("checked/wrong"),
        99,
        SignId::from("wrong/body-born"),
    )
    .unwrap()
    .body_id;

    assert_eq!(
        lifecycle.origin_offline(
            &wrong_body,
            &origin.boot_id,
            SignId::from("birth/wrong-body-detach")
        ),
        Err(BuildBirthError::Membership(MembershipRefusal::WrongBody))
    );
    assert!(lifecycle.membership().unwrap().parts[0].is_present());

    lifecycle
        .origin_offline(
            &body_id,
            &origin.boot_id,
            SignId::from("birth/host-offline"),
        )
        .unwrap();
    let part = &lifecycle.membership().unwrap().parts[0];
    assert_eq!(part.state, MembershipState::Admitted);
    assert_eq!(part.current, None);
}
