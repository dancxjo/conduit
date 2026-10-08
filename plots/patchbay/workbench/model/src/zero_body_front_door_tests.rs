use super::*;
use patchbay_application::compare_entrances;

use crate::{
    prepare_renderer_execution, PatchbayInvocationOutcome, PatchbayRefusal,
    RendererAdapterIdentity, RendererAdapterKind,
};
use conduit_body::{AuthenticatedHostObservation, BodyMembership, MembershipProofId, PartId};
use conduit_core::{
    kind_id, BootId, HostId, OfferGeneration, ResourceAccessMode, ResourceContentRequirement,
    ResourceRetention, ResourceSemanticIdentity, ResourceSharing, ResourceVersionIdentity,
};
use conduit_presentation::{PresentationPropertyValue, PresentationRole};
use patchbay_application::{EntranceAction, PatchbayEntranceState};

const SOURCE: &str = include_str!("../../../../../plots/patchbay-front-door/main.conduit");

fn living_body_candidate() -> BodyJoinCandidate {
    let editor = PlotEditor::from_source("living-body.conduit".into(), SOURCE.into()).unwrap();
    let source_document_id = editor.view().checked.source_document_id.clone().unwrap();
    let checked_plot_id = editor.view().checked.plots[0].checked_plot_id.clone();
    let body = Body::born(
        source_document_id,
        checked_plot_id,
        7,
        SignId::from("living/body/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(8, SignId::from("living/body/woke")).unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let remote = PartId::bind(&body.body_id, "living/remote-host", 1).unwrap();
    let remote_proof = MembershipProofId::bind("living/remote-proof").unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            remote.clone(),
            remote_proof.clone(),
            SignId::from("living/remote/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &remote,
            AuthenticatedHostObservation {
                host_id: HostId::from("living/remote-host"),
                boot_id: BootId::from("living/remote-boot"),
                offer_generation: OfferGeneration(1),
                proof_id: remote_proof,
                sequence: 1,
            },
            SignId::from("living/remote/present"),
        )
        .unwrap();
    BodyJoinCandidate::new(
        "Living Body",
        body,
        wake,
        membership,
        editor,
        MembershipProofId::bind("living/local-join-proof").unwrap(),
        SignId::from("living/body/discovered"),
        11,
    )
    .unwrap()
}

#[test]
fn zero_body_world_is_valid_and_native_browser_semantics_match() {
    let session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("zero/host"),
        BootId::from("zero/boot"),
    )
    .unwrap();
    let presentation = session.project().unwrap().presentation;
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.checked_plot_id.is_none());
    assert!(presentation.basis.wake_id.is_none());
    assert!(presentation.basis.source_document_id.is_none());
    assert!(presentation.basis.checked_plot_id.is_none());
    assert!(!presentation.subjects.iter().any(|subject| matches!(
        subject.role,
        PresentationRole::Body | PresentationRole::Part
    )));
    let host = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Host)
        .unwrap();
    assert!(presentation.properties.iter().any(|property| {
        property.subject == host.identity
            && property.name == "current-body"
            && property.value == PresentationPropertyValue::Text("none".into())
    }));
    let native = PatchbayEntranceState::enter(&presentation).unwrap();
    let browser = PatchbayEntranceState::enter(&presentation).unwrap();
    assert_eq!(
        native.selected_subject.as_deref(),
        Some(host.identity.as_str())
    );
    assert_eq!(
        native.available_actions,
        vec![EntranceAction::Inspect, EntranceAction::Birth]
    );
    assert!(native.body_id.is_none());
    let report = compare_entrances(&presentation, &native, &browser).unwrap();
    assert!(report.equivalent);
    let mut actions = presentation.actions.clone();
    actions.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut disclosures = presentation.disclosures.clone();
    disclosures.sort_by(|left, right| left.subject.cmp(&right.subject));
    assert_eq!(report.semantic_actions, actions);
    assert_eq!(report.disclosures, disclosures);
    assert!(presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/open@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Available
            )
    }));
    assert!(presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/birth@1"
            && action.target == host.identity
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Available
            )
    }));
    assert!(presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/birth@1"
            && action.target != host.identity
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Unavailable { .. }
            )
    }));
    for (adapter, name) in [
        (RendererAdapterKind::NativeWayland, "native"),
        (RendererAdapterKind::HtmlDomSvg, "browser"),
    ] {
        prepare_renderer_execution(
            presentation.clone(),
            adapter,
            RendererAdapterIdentity {
                host_id: HostId::from(format!("zero/{name}")),
                boot_id: BootId::from(format!("zero/{name}/boot")),
                target_subject: format!("zero/{name}/target"),
            },
            SignId::from(format!("zero/{name}/prepared")),
        )
        .unwrap();
    }
}

#[test]
fn opening_plot_is_inert_and_only_explicit_birth_embodies_host() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("birth/host"),
        BootId::from("birth/boot"),
    )
    .unwrap();
    let initial = session.project().unwrap().presentation;
    let plot = initial
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Plot)
        .unwrap()
        .identity
        .clone();
    let plot_id = session.plots[0].checked_plot_id.clone();
    session.open_plot(&plot_id, initial.revision).unwrap();
    let opened = session.project().unwrap().presentation;
    assert!(opened.basis.body_id.is_none());
    assert!(opened.properties.iter().any(|property| {
        property.subject == plot
            && property.name == "opened"
            && property.value == PresentationPropertyValue::Flag(true)
    }));
    for role in [
        PresentationRole::Plot,
        PresentationRole::Gear,
        PresentationRole::Port,
        PresentationRole::Cord,
    ] {
        assert!(opened.subjects.iter().any(|subject| subject.role == role));
    }
    assert!(opened.actions.iter().any(|action| {
        action.target == plot
            && action.intent == "conduit.intent/birth@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Available
            )
    }));
    assert!(opened.basis.source_document_id.is_none());
    assert!(opened.basis.checked_plot_id.is_none());
    assert!(opened.basis.expanded_plot_id.is_none());
    assert!(opened.basis.plan_id.is_none());
    assert!(opened.basis.active_play_id.is_none());
    assert!(session.clone().birth(initial.revision).is_err());
    let embodied = session.birth(opened.revision).unwrap();
    let projection = embodied.project().unwrap();
    assert!(projection.presentation.basis.body_id.is_some());
    assert!(projection.presentation.basis.wake_id.is_none());
    assert!(projection.presentation.basis.plan_id.is_none());
    assert!(projection.presentation.basis.active_play_id.is_none());
    assert!(projection.presentation.actions.iter().any(|action| {
        action.intent == "conduit.intent/wake@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Available
            )
    }));
    assert_eq!(projection.parts.parts.len(), 1);
}

fn creche_session(suffix: &str) -> ZeroBodyFrontDoor {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from(format!("creche/{suffix}/host")),
        BootId::from(format!("creche/{suffix}/boot")),
    )
    .unwrap();
    session
        .add_plot(
            PlotCandidate::from_source_plot(
                "Patchbay front door",
                "plots/patchbay-front-door/main.conduit",
                SOURCE,
                "patchbay-front-door",
                "reviewed installed plot",
                SignId::from(format!("creche/{suffix}/plot-reviewed")),
                2,
            )
            .unwrap(),
        )
        .unwrap();
    session
}

#[test]
fn selected_checkpoint_host_offers_todo_at_birth() {
    let root = std::env::temp_dir().join(format!(
        "conduit-creche-todo-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let content = ResourceContentRequirement {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        version: ResourceVersionIdentity::from_digest([2; 32]),
        content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
        maximum_bytes: conduit_std_offers::TODO_CHECKPOINT_MAX_BYTES,
        maximum_items: 1,
        retention: ResourceRetention::ExternalDurable,
        sharing: ResourceSharing::SingleWriterPublished,
        access: ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 1,
        sensitive: false,
    };
    let host = conduit_std_host::StdHost::new_for_todo_checkpoint_once(
        conduit_std_host::StdHostConfig {
            host_id: HostId::from("creche/todo/host"),
            boot_id: BootId::from("creche/todo/boot"),
            offer_generation: OfferGeneration(1),
        },
        &root,
        content,
    )
    .unwrap();
    let door = ZeroBodyFrontDoor::from_model(
        crate::host_adapter::test_host_adapter_arc(),
        PatchbayModel::from_advertisement(host.advertisement().clone()),
    )
    .unwrap();
    let mut draft = door
        .creche_draft("00112233-4455-6677-8899-000000000003".into())
        .unwrap();
    assert_eq!(draft.choices().len(), 3);
    assert_eq!(draft.choices()[2].title, "Todo list");
    draft.select(draft.revision(), 2, true).unwrap();
    let selection = draft.selection(draft.revision()).unwrap();
    assert_eq!(selection.workset.plots().len(), 1);
    assert_eq!(
        door.primary_selected_source(&selection).unwrap(),
        Some(include_str!(
            "../../../../../plots/todo/checkpoint-once.conduit"
        ))
    );
    let revision = door.revision();
    let born = door.birth_from_creche(selection, revision).unwrap();
    assert_eq!(born.body().workset.plots().len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn creche_birth_uses_one_path_for_zero_one_and_many_reviewed_plots() {
    for (suffix, selected) in [("zero", 0_usize), ("one", 1), ("many", 2)] {
        let session = creche_session(suffix);
        assert!(session.opened().is_none());
        let mut draft = session
            .creche_draft(format!("00112233-4455-6677-8899-{selected:012}"))
            .unwrap();
        assert!(draft.choices().iter().all(|choice| !choice.selected));
        assert!(session.opened().is_none());
        assert!(session
            .project()
            .unwrap()
            .presentation
            .properties
            .iter()
            .all(|property| property.name != "friendly-name"));
        for index in 0..selected {
            draft.select(draft.revision(), index, true).unwrap();
        }
        let selection = draft.selection(draft.revision()).unwrap();
        let expected = selection.clone();
        let revision = session.revision();
        let embodied = session.birth_from_creche(selection, revision).unwrap();

        assert_eq!(embodied.body().workset, expected.workset);
        assert_eq!(embodied.body().workload_revision, 0);
        assert!(embodied.wake().is_none());
        let evidence = embodied.birth_evidence().unwrap();
        assert_eq!(evidence.selection_revision, expected.revision);
        assert_eq!(evidence.friendly_name, expected.friendly_name);
        assert_eq!(evidence.workset, expected.workset);
        assert_eq!(embodied.body().sign_ids, vec![evidence.sign_id.clone()]);
        let projection = embodied.project().unwrap();
        assert_eq!(
            projection.presentation.basis.body_id.as_ref(),
            Some(&embodied.body().body_id)
        );
        assert!(projection.presentation.basis.wake_id.is_none());
        if selected == 0 {
            assert!(projection.presentation.basis.checked_plot_id.is_none());
            assert!(projection
                .presentation
                .subjects
                .iter()
                .all(|subject| { subject.role != PresentationRole::Plot }));
        }
    }
}

#[test]
fn independent_creche_births_bind_the_creating_host_boot() {
    let birth = |session: ZeroBodyFrontDoor| {
        let mut draft = session
            .creche_draft("00112233-4455-6677-8899-000000000001".into())
            .unwrap();
        draft.select(draft.revision(), 0, true).unwrap();
        let selection = draft.selection(draft.revision()).unwrap();
        let revision = session.revision();
        session
            .birth_from_creche(selection, revision)
            .unwrap()
            .body()
            .clone()
    };
    let first = birth(creche_session("first"));
    let second = birth(creche_session("second"));
    let replay = birth(creche_session("first"));
    assert_eq!(first.workset, second.workset);
    assert_eq!(first.birth_sequence, second.birth_sequence);
    assert_ne!(first.sign_ids[0], second.sign_ids[0]);
    assert_ne!(first.body_id, second.body_id);
    assert_eq!(first.sign_ids[0], replay.sign_ids[0]);
    assert_eq!(first.body_id, replay.body_id);
}

#[test]
fn discovered_body_open_is_inert_and_explicit_proof_backed_join_is_exact() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("join/local-host"),
        BootId::from("join/local-boot"),
    )
    .unwrap();
    let candidate = living_body_candidate();
    let body_id = candidate.body.body_id.clone();
    session.observe_body_candidate(candidate).unwrap();
    let observed = session.project().unwrap().presentation;
    let body_subject = format!("body/{}", body_id.as_str());
    assert!(observed.subjects.iter().any(|subject| {
        subject.identity == body_subject && subject.role == PresentationRole::Body
    }));
    assert!(observed.basis.body_id.is_none());
    let mut entrance = PatchbayEntranceState::enter(&observed).unwrap();
    entrance.select(&observed, &body_subject).unwrap();
    assert_eq!(
        entrance.available_actions,
        vec![EntranceAction::Inspect, EntranceAction::Open]
    );
    session.open_body(&body_id, observed.revision).unwrap();
    let opened = session.project().unwrap().presentation;
    assert!(opened.basis.body_id.is_none());
    assert!(session.clone().join_open_body(observed.revision).is_err());
    let joined = session.join_open_body(opened.revision).unwrap();
    assert_eq!(joined.body().body_id, body_id);
    let projection = joined.project().unwrap();
    assert_eq!(
        projection.presentation.basis.body_id.as_ref(),
        Some(&body_id)
    );
    assert_eq!(projection.parts.parts.len(), 2);
    assert!(projection
        .parts
        .parts
        .iter()
        .any(|part| { part.details.host_id.as_ref() == Some(&HostId::from("join/local-host")) }));
}

#[test]
fn zero_body_sources_are_finite_and_duplicates_fail_closed() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("bounds/host"),
        BootId::from("bounds/boot"),
    )
    .unwrap();
    let seed = session.plots[0].clone();
    assert!(session.add_plot(seed).is_err());
    let candidate = living_body_candidate();
    session.observe_body_candidate(candidate.clone()).unwrap();
    assert!(session.observe_body_candidate(candidate).is_err());
}

#[test]
fn opened_plot_edits_canonical_source_and_stale_edits_are_atomic() {
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("edit/host"),
        BootId::from("edit/boot"),
    )
    .unwrap();
    let plot = PlotCandidate::from_source(
        "Empty",
        "empty.conduit",
        "plot making {\n}\n",
        "test source",
        SignId::from("edit/plot"),
        9,
    )
    .unwrap();
    let plot_id = plot.checked_plot_id.clone();
    session.add_plot(plot).unwrap();
    let revision = session.revision();
    session.open_plot(&plot_id, revision).unwrap();
    let document = session.opened_plot_document().unwrap();
    let graph = session
        .plots
        .last()
        .unwrap()
        .editor()
        .unwrap()
        .patchbay_graph_for_authoring("making")
        .unwrap();
    let basis = crate::PatchbayEditBasis::new(
        document.checked.source_document_id.clone().unwrap(),
        document.revision,
        graph.expanded_plot_id,
    )
    .unwrap();
    let first = crate::PatchbayEdit::PlaceGear {
        basis: basis.clone(),
        kind_id: "text/literal".into(),
    };
    assert_eq!(
        session.apply_opened_plot_edit(&first),
        PatchbayInvocationOutcome::Succeeded
    );
    let after_first = session.opened_plot_document().unwrap();
    assert!(after_first.source.contains("literal: text/literal(\"\")"));
    let unchanged = after_first.source.clone();
    assert_eq!(
        session.apply_opened_plot_edit(&first),
        PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
    );
    assert_eq!(session.opened_plot_document().unwrap().source, unchanged);

    let seed = session
        .plots
        .iter()
        .find(|seed| seed.source_name == "empty.conduit")
        .unwrap();
    let document = seed.editor().unwrap().view();
    let graph = seed
        .editor()
        .unwrap()
        .patchbay_graph_for_authoring("making")
        .unwrap();
    let second = crate::PatchbayEdit::PlaceGear {
        basis: crate::PatchbayEditBasis::new(
            document.checked.source_document_id.clone().unwrap(),
            document.revision,
            graph.expanded_plot_id,
        )
        .unwrap(),
        kind_id: "text/literal".into(),
    };
    assert_eq!(
        session.apply_opened_plot_edit(&second),
        PatchbayInvocationOutcome::Succeeded
    );
    let final_source = session.opened_plot_document().unwrap().source;
    assert!(final_source.contains("literal: text/literal(\"\")"));
    assert!(final_source.contains("literal-2: text/literal(\"\")"));

    let seed = session
        .plots
        .iter()
        .find(|seed| seed.source_name == "empty.conduit")
        .unwrap();
    let document = seed.editor().unwrap().view();
    let graph = seed
        .editor()
        .unwrap()
        .patchbay_graph_for_authoring("making")
        .unwrap();
    let invalid = crate::PatchbayEdit::ConfigureGear {
        basis: crate::PatchbayEditBasis::new(
            document.checked.source_document_id.clone().unwrap(),
            document.revision,
            graph.expanded_plot_id,
        )
        .unwrap(),
        subject_identity: "gear/making/literal".into(),
        key: "value".into(),
        value: conduit_core::ConfigurationValue::U64(7),
    };
    let unchanged = document.source;
    assert_eq!(
        session.apply_opened_plot_edit(&invalid),
        PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration)
    );
    assert_eq!(session.opened_plot_document().unwrap().source, unchanged);
}
