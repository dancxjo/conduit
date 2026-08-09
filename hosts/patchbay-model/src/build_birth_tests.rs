use conduit_body::{BodyLifecycleEvent, BodyState, WakeLifecycle, WakeLifecycleEvent};
use conduit_core::{bind_active_play, SignId};
use conduit_std_host::StdHost;

use crate::{BuildBirthController, BuildBirthError, FormEditor, PatchbayMode};

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

#[test]
fn build_birth_wake_replan_play_and_lull_use_exact_canonical_values() {
    let mut editor = editor(include_str!("../../../examples/hello.conduit"));
    let mut lifecycle = BuildBirthController::new();
    let build = lifecycle.document(&editor).unwrap();
    assert_eq!(build.mode, PatchbayMode::Build);
    assert_eq!(build.revisions.saved_revision, 0);
    assert_eq!(build.revisions.checked_revision, Some(0));

    editor
        .replace_source(format!(
            "{}\n",
            include_str!("../../../examples/hello.conduit")
        ))
        .unwrap();
    assert_eq!(
        lifecycle.birth(&editor, 7, SignId::from("build/born")),
        Err(BuildBirthError::UncheckedRevision)
    );
    editor.recheck().unwrap();
    lifecycle
        .birth(&editor, 7, SignId::from("build/born"))
        .unwrap();
    let born_body = lifecycle.body().unwrap().clone();
    let (first_plan, first_play) = planned_on_fresh_host(&editor);
    let (replacement_plan, replacement_play) = planned_on_fresh_host(&editor);
    assert_ne!(first_plan.plan_id, replacement_plan.plan_id);
    assert_eq!(born_body.state, BodyState::Lulled);
    assert_eq!(
        born_body.events,
        vec![BodyLifecycleEvent::Born {
            sign_id: SignId::from("build/born")
        }]
    );
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
    let mut editor = editor(include_str!("../../../examples/greet.conduit"));
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
        lifecycle.birth(&invalid, 0, SignId::from("invalid/born")),
        Err(BuildBirthError::UncheckedRevision)
    );
    assert!(lifecycle
        .document(&invalid)
        .unwrap()
        .lines
        .join("\n")
        .contains("unchecked"));

    invalid
        .replace_source(include_str!("../../../examples/hello.conduit").into())
        .unwrap();
    invalid.recheck().unwrap();
    lifecycle
        .birth(&invalid, 0, SignId::from("valid/born"))
        .unwrap();
    assert_eq!(
        lifecycle.birth(&invalid, 1, SignId::from("duplicate/born")),
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
