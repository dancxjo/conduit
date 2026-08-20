use super::*;
use crate::{
    compare_entrances, EntranceAction, PatchbayEntranceState, PatchbayInvocationOutcome,
    PatchbayRefusal, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use conduit_body::{AuthenticatedHostObservation, BodyMembership, MembershipProofId, PartId};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_presentation::{PresentationPropertyValue, PresentationRole};

const SOURCE: &str = include_str!("../../../examples/patchbay-front-door.conduit");

fn living_body_candidate() -> BodyJoinCandidate {
    let editor = FormEditor::from_source("living-body.conduit".into(), SOURCE.into()).unwrap();
    let source_document_id = editor.view().checked.source_document_id.clone().unwrap();
    let checked_form_id = editor.view().checked.forms[0].checked_form_id.clone();
    let body = Body::born(
        source_document_id,
        checked_form_id,
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
    let session =
        ZeroBodyFrontDoor::with_identity(HostId::from("zero/host"), BootId::from("zero/boot"))
            .unwrap();
    let presentation = session.project().unwrap().presentation;
    assert!(presentation.basis.body_id.is_none());
    assert!(presentation.basis.seed_id.is_none());
    assert!(presentation.basis.wake_id.is_none());
    assert!(presentation.basis.source_document_id.is_none());
    assert!(presentation.basis.checked_form_id.is_none());
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
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Unavailable { .. }
            )
    }));
    for (adapter, name) in [
        (RendererAdapterKind::NativeWayland, "native"),
        (RendererAdapterKind::HtmlDomSvg, "browser"),
    ] {
        RendererExecution::prepare(
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
fn opening_seed_is_inert_and_only_explicit_birth_embodies_host() {
    let mut session =
        ZeroBodyFrontDoor::with_identity(HostId::from("birth/host"), BootId::from("birth/boot"))
            .unwrap();
    let initial = session.project().unwrap().presentation;
    let seed = initial
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Seed)
        .unwrap()
        .identity
        .clone();
    let seed_id = session.seeds[0].seed_id.clone();
    session.open_seed(&seed_id, initial.revision).unwrap();
    let opened = session.project().unwrap().presentation;
    assert!(opened.basis.body_id.is_none());
    assert!(opened.properties.iter().any(|property| {
        property.subject == seed
            && property.name == "opened"
            && property.value == PresentationPropertyValue::Flag(true)
    }));
    for role in [
        PresentationRole::Form,
        PresentationRole::Gear,
        PresentationRole::Port,
        PresentationRole::Cord,
    ] {
        assert!(opened.subjects.iter().any(|subject| subject.role == role));
    }
    assert!(opened.actions.iter().any(|action| {
        action.target == seed
            && action.intent == "conduit.intent/birth@1"
            && matches!(
                action.availability,
                conduit_presentation::PresentationActionAvailability::Available
            )
    }));
    assert!(opened.basis.source_document_id.is_none());
    assert!(opened.basis.checked_form_id.is_none());
    assert!(opened.basis.expanded_form_id.is_none());
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

#[test]
fn discovered_body_open_is_inert_and_explicit_proof_backed_join_is_exact() {
    let mut session = ZeroBodyFrontDoor::with_identity(
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
    let mut session =
        ZeroBodyFrontDoor::with_identity(HostId::from("bounds/host"), BootId::from("bounds/boot"))
            .unwrap();
    let seed = session.seeds[0].clone();
    assert!(session.add_seed(seed).is_err());
    let candidate = living_body_candidate();
    session.observe_body_candidate(candidate.clone()).unwrap();
    assert!(session.observe_body_candidate(candidate).is_err());
}

#[test]
fn opened_seed_edits_canonical_source_and_stale_edits_are_atomic() {
    let mut session =
        ZeroBodyFrontDoor::with_identity(HostId::from("edit/host"), BootId::from("edit/boot"))
            .unwrap();
    let seed = SeedCandidate::from_source(
        "Empty",
        "empty.conduit",
        "form making {\n}\n",
        "test source",
        SignId::from("edit/seed"),
        9,
    )
    .unwrap();
    let seed_id = seed.seed_id.clone();
    session.add_seed(seed).unwrap();
    let revision = session.revision();
    session.open_seed(&seed_id, revision).unwrap();
    let document = session.opened_seed_document().unwrap();
    let graph = session
        .seeds
        .last()
        .unwrap()
        .editor()
        .unwrap()
        .patchbay_graph_for_authoring("making")
        .unwrap();
    let basis = crate::PatchbayEditBasis::new(
        document.checked.source_document_id.clone().unwrap(),
        document.revision,
        graph.expanded_form_id,
    )
    .unwrap();
    let first = crate::PatchbayEdit::PlaceGear {
        basis: basis.clone(),
        kind_id: "text/literal".into(),
    };
    assert_eq!(
        session.apply_opened_seed_edit(&first),
        PatchbayInvocationOutcome::Succeeded
    );
    let after_first = session.opened_seed_document().unwrap();
    assert!(after_first.source.contains("literal: text/literal(\"\")"));
    let unchanged = after_first.source.clone();
    assert_eq!(
        session.apply_opened_seed_edit(&first),
        PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
    );
    assert_eq!(session.opened_seed_document().unwrap().source, unchanged);

    let seed = session
        .seeds
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
            graph.expanded_form_id,
        )
        .unwrap(),
        kind_id: "text/literal".into(),
    };
    assert_eq!(
        session.apply_opened_seed_edit(&second),
        PatchbayInvocationOutcome::Succeeded
    );
    let final_source = session.opened_seed_document().unwrap().source;
    assert!(final_source.contains("literal: text/literal(\"\")"));
    assert!(final_source.contains("literal-2: text/literal(\"\")"));

    let seed = session
        .seeds
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
            graph.expanded_form_id,
        )
        .unwrap(),
        subject_identity: "gear/making/literal".into(),
        key: "value".into(),
        value: conduit_core::ConfigurationValue::U64(7),
    };
    let unchanged = document.source;
    assert_eq!(
        session.apply_opened_seed_edit(&invalid),
        PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration)
    );
    assert_eq!(session.opened_seed_document().unwrap().source, unchanged);
}
