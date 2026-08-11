use std::path::PathBuf;

use crate::{
    AuthoredEnvironment, AuthoredPart, FormEditor, MachineProfile, PrewakeController, PrewakeError,
    PrewakeState,
};

fn environment() -> AuthoredEnvironment {
    let mut environment = AuthoredEnvironment::new("prewake-bench").unwrap();
    environment
        .add_part(AuthoredPart::reviewed(
            "laptop",
            "Simulator",
            MachineProfile::LaptopLinux,
        ))
        .unwrap();
    environment
}

fn editor(source: &str) -> FormEditor {
    FormEditor::from_source(PathBuf::from("prewake.conduit"), source.into()).unwrap()
}

fn hello_with(message: &str) -> String {
    include_str!("../../../examples/hello.conduit").replace("Hello, world.", message)
}

#[test]
fn auto_rehearsal_rechecks_replans_and_plays_with_new_immutable_identities() {
    let environment = environment();
    let mut editor = editor(include_str!("../../../examples/hello.conduit"));
    let mut prewake = PrewakeController::default();
    prewake.enter(&editor, &environment).unwrap();
    let (first_plan, first_play) = match prewake.state() {
        PrewakeState::Auto { plan, play, .. } => (plan.clone(), play.clone()),
        state => panic!("unexpected state {state:?}"),
    };
    editor
        .replace_source(hello_with("Hello, rehearsal."))
        .unwrap();
    editor.recheck().unwrap();
    prewake.rehearse(&editor, &environment).unwrap();
    let PrewakeState::Auto { plan, play, .. } = prewake.state() else {
        panic!()
    };
    assert_ne!(plan.plan_id, first_plan.plan_id);
    assert_ne!(play.active_play_ids, first_play.active_play_ids);
    assert_eq!(first_plan.plan_id, first_play.plan_id);
    assert_eq!(prewake.history().len(), 1);
    let provenance = prewake.provenance();
    assert!(provenance.simulation_truth);
    assert!(!provenance.observed_live_truth);
    assert!(!provenance.physical_effect_authority);
    assert!(!provenance.promotable_to_physical_plan);
}

#[test]
fn hold_replaces_pending_plan_and_refuses_stale_release_without_losing_coherent_state() {
    let environment = environment();
    let mut editor = editor(include_str!("../../../examples/hello.conduit"));
    let mut prewake = PrewakeController::default();
    prewake.set_hold(true);
    prewake.enter(&editor, &environment).unwrap();
    let first = match prewake.state() {
        PrewakeState::Held { plan, .. } => plan.plan_id.clone(),
        _ => panic!(),
    };
    editor
        .replace_source(hello_with("Hello, held replacement."))
        .unwrap();
    editor.recheck().unwrap();
    prewake.rehearse(&editor, &environment).unwrap();
    let replacement = match prewake.state() {
        PrewakeState::Held { plan, .. } => plan.plan_id.clone(),
        _ => panic!(),
    };
    assert_ne!(first, replacement);
    editor.replace_source(hello_with("Hello, stale.")).unwrap();
    editor.recheck().unwrap();
    assert_eq!(
        prewake.release(&editor, &environment),
        Err(PrewakeError::StaleHeldPlan {
            plan_id: replacement.clone()
        })
    );
    assert!(
        matches!(prewake.state(), PrewakeState::Held { plan, .. } if plan.plan_id == replacement)
    );
    prewake.rehearse(&editor, &environment).unwrap();
    prewake.release(&editor, &environment).unwrap();
    assert!(
        matches!(prewake.state(), PrewakeState::Auto { play, .. } if play.plan_id != replacement)
    );
}

#[test]
fn invalid_edit_preserves_last_coherent_rehearsal_and_never_runs() {
    let environment = environment();
    let mut editor = editor(include_str!("../../../examples/hello.conduit"));
    let mut prewake = PrewakeController::default();
    prewake.enter(&editor, &environment).unwrap();
    let coherent = prewake.state().clone();
    editor.replace_source("this is not a Form".into()).unwrap();
    editor.recheck().unwrap();
    assert_eq!(
        prewake.rehearse(&editor, &environment),
        Err(PrewakeError::InvalidForm)
    );
    assert_eq!(prewake.state(), &coherent);
    assert_eq!(prewake.last_refusal(), Some(&PrewakeError::InvalidForm));
}
