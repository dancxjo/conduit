use super::*;
use conduit_body::{Body, BodyPresentationSelector, BodyWorkset, ResidentForm};
use conduit_core::{BaseImplementationId, BootId, HostId, PlacementId, SignId};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationRole, PresentationSubject,
};
use conduit_std_host::StdHost;

#[test]
fn touch_my_own_patchbay_replans_graphical_parallel_speech_and_back() {
    let candidate = FormCandidate::from_source(
        "Hello",
        "forms/hello/main.conduit",
        include_str!("../../../../forms/hello/main.conduit"),
        "canonical Presenter-control Form",
        SignId::from("sign/form-reviewed"),
        1,
    )
    .unwrap();
    let expanded = candidate.editor().unwrap().expand_form("hello").unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.host_id = HostId::from("host/body");
    advertisement.boot_id = BootId::from("boot/body");
    let placements = default_expanded_placements(&expanded, &[advertisement.clone()]).unwrap();
    let form_plan = plan_expanded_canonical(
        &expanded,
        &[advertisement],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    let resident = ResidentForm::new(
        candidate.source_document_id.clone(),
        candidate.checked_form_id.clone(),
    );
    let body = Body::born_with_forms(
        BodyWorkset::one(resident.clone()).unwrap(),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id.clone()),
            wake_id: None,
            source_document_id: Some(resident.source_document_id.clone()),
            checked_form_id: Some(resident.checked_form_id.clone()),
            expanded_form_id: Some(expanded.expanded_form_id.clone()),
            plan_id: Some(form_plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "patchbay/self".into(),
            role: PresentationRole::Document,
            label: "Patchbay".into(),
            accessibility_name: "Patchbay controlling its own Presenter topology".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let identity = |label: &str| RendererAdapterIdentity {
        host_id: HostId::from(format!("host/{label}")),
        boot_id: BootId::from(format!("boot/{label}")),
        target_subject: format!("target/{label}"),
    };
    let mut controller = PresenterControlSession::new(
        presentation.clone(),
        BodyPresentationSelector {
            form: Some(resident),
            source_placement_id: PlacementId::from("hello/presentation"),
        },
        RendererAdapterKind::NativeWayland,
        identity("graphical"),
        identity("speech"),
    )
    .unwrap();
    let initial_topology = controller.prepare_initial_graphical().unwrap();
    let form = conduit_body::BodyFormPlan {
        form: initial_topology.presentation.form.clone().unwrap(),
        plan: form_plan,
    };
    let mut planning = BodyPlanningSession::start_with_presenters(
        &body,
        1,
        SignId::from("sign/woke"),
        vec![form],
        vec![initial_topology],
        SignId::from("sign/plan-a"),
        1,
        SignId::from("sign/play-a"),
    )
    .unwrap();
    let body_id = planning.body().body_id.clone();
    let forms = planning.current_plan().forms.clone();
    let plan_a = planning.current_plan().plan_id.clone();
    let parallel = controller
        .request_mode(
            &plan_a,
            PresenterTopologyMode::GraphicalAndSpeech,
            &mut planning,
            transition(2, "parallel"),
        )
        .unwrap();
    assert_eq!(parallel.chains.len(), 2);
    assert_eq!(parallel.presentation_id, presentation.identity.as_str());
    assert_eq!(planning.body().body_id, body_id);
    assert_eq!(planning.current_plan().forms, forms);
    assert_ne!(parallel.plan_id, plan_a);
    let speech_manifestation = parallel.chains[1].manifestation_id.clone();
    let speech_only = controller
        .request_mode(
            &parallel.plan_id,
            PresenterTopologyMode::Speech,
            &mut planning,
            transition(3, "speech-only"),
        )
        .unwrap();
    assert_eq!(speech_only.chains.len(), 1);
    assert_eq!(speech_only.chains[0].manifestation_id, speech_manifestation);
    let restored = controller
        .request_mode(
            &speech_only.plan_id,
            PresenterTopologyMode::GraphicalAndSpeech,
            &mut planning,
            transition(4, "restored"),
        )
        .unwrap();
    assert_eq!(restored.chains.len(), 2);
    assert_eq!(restored.chains[1].manifestation_id, speech_manifestation);
    assert_ne!(
        restored.chains[0].manifestation_id,
        parallel.chains[0].manifestation_id
    );
    assert_eq!(planning.snapshot().historical_plan_ids.len(), 4);
}

fn transition(sequence: u64, label: &str) -> BodyPlanningTransition {
    BodyPlanningTransition {
        unsatisfied_sign_id: Some(SignId::from(format!("sign/{label}/unsatisfied"))),
        plan_ready_sign_id: SignId::from(format!("sign/{label}/plan")),
        play_sequence: sequence,
        play_started_sign_id: SignId::from(format!("sign/{label}/play")),
    }
}
