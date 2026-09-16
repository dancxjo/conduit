use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyFormPlan, BodyMembership,
    BodyPlayIdentity, BodyState, MembershipProofId, PartId, ResidentForm, WakeLifecycle,
};
use conduit_core::{
    BootId, ExpandedFormId, FormIdentity, HostId, OfferGeneration, bind_sign, seal_plan,
};
use conduit_workspace_model::{WorkspaceBody, WorkspaceBodyError};

fn host() -> HostId {
    "host/here".into()
}
fn boot() -> BootId {
    "boot/one".into()
}
fn form(name: &str) -> ResidentForm {
    ResidentForm::new(
        format!("source/{name}").into(),
        format!("checked/{name}").into(),
    )
}
fn born() -> WorkspaceBody {
    let form = form("morse");
    let body = Body::born(
        form.source_document_id,
        form.checked_form_id,
        1,
        "sign/birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Roseau".into()).unwrap();
    let part = PartId::bind(&body.body_id, "here", 1).unwrap();
    let proof = MembershipProofId::bind("proof/here").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host(),
                boot_id: boot(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "sign/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    WorkspaceBody::open(evidence).unwrap()
}
fn plans(body: &WorkspaceBody) -> Vec<BodyFormPlan> {
    body.evidence()
        .body
        .workset
        .forms()
        .iter()
        .map(|form| BodyFormPlan {
            form: form.clone(),
            plan: seal_plan(
                FormIdentity {
                    source_document_id: form.source_document_id.clone(),
                    checked_form_id: form.checked_form_id.clone(),
                    expanded_form_id: ExpandedFormId::from("expanded/fixture"),
                },
                vec![],
            ),
        })
        .collect()
}
fn start(body: &mut WorkspaceBody) -> BodyPlayIdentity {
    let proposal = body.propose(plans(body), &host(), &boot()).unwrap().clone();
    let play = BodyPlayIdentity::bind(&proposal.plan, 1);
    let sign = |sequence| bind_sign(&host(), &boot(), Some(&play.active_play_id), sequence).sign_id;
    let wake = proposal
        .wake
        .body_plan_ready(&proposal.plan, sign(0))
        .unwrap()
        .body_play_started(&proposal.plan, &play, sign(1))
        .unwrap();
    body.started(&host(), &boot(), play.clone(), wake).unwrap();
    play
}

fn persist_archives(body: &mut WorkspaceBody) {
    let Some(head) = body.pending_archives().last() else {
        return;
    };
    head.validate_as_head_of(body.evidence()).unwrap();
    let digest = head.digest;
    body.acknowledge_archives(digest).unwrap();
}

#[test]
fn birth_to_play_to_lull_retains_identity_and_a_later_wake_gets_a_fresh_play() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    let first = start(&mut body);
    assert_eq!(
        body.realization().unwrap().wake.lifecycle,
        WakeLifecycle::Playing
    );
    body.lull(&host(), &boot(), Some(&first)).unwrap();
    assert_eq!(body.evidence().body.state, BodyState::Lulled);
    let retained: BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(body.evidence()).unwrap()).unwrap();
    let mut restored = WorkspaceBody::open(retained).unwrap();
    let second = start(&mut restored);
    assert_eq!(restored.evidence().body_id, identity);
    assert_ne!(first.wake_id, second.wake_id);
    assert_ne!(first.active_play_id, second.active_play_id);
    restored.evidence().validate().unwrap();
}

#[test]
fn missing_workload_and_stale_boot_refuse_before_publishing_a_wake() {
    let mut body = born();
    let before = body.evidence().clone();
    assert!(body.propose(vec![], &host(), &boot()).is_err());
    assert_eq!(
        body.propose(plans(&body), &host(), &"boot/stale".into()),
        Err(WorkspaceBodyError::StaleHost)
    );
    assert_eq!(body.evidence(), &before);
    assert!(body.realization().is_none());
}

#[test]
fn awake_snapshots_do_not_resurrect_a_play_and_stale_terminal_identity_cannot_lull_it() {
    let mut body = born();
    let play = start(&mut body);
    let before = body.evidence().clone();
    assert!(matches!(
        WorkspaceBody::open(before.clone()),
        Err(WorkspaceBodyError::UnreconciledWake)
    ));
    assert_eq!(
        body.lull(&host(), &boot(), None),
        Err(WorkspaceBodyError::StalePlay)
    );
    let mut stale = play;
    stale.play_sequence += 1;
    assert_eq!(
        body.lull(&host(), &boot(), Some(&stale)),
        Err(WorkspaceBodyError::StalePlay)
    );
    assert_eq!(body.evidence(), &before);
}

#[test]
fn workload_edits_are_revision_checked_and_cannot_mutate_an_active_plan() {
    let mut body = born();
    assert_eq!(
        body.admit_form(1, form("clock"), &host(), &boot()),
        Err(WorkspaceBodyError::StaleWorkload)
    );
    body.admit_form(0, form("clock"), &host(), &boot()).unwrap();
    assert_eq!(body.evidence().body.workload_revision, 1);
    assert_eq!(body.evidence().body.workset.len(), 2);
    start(&mut body);
    let before = body.evidence().clone();
    assert_eq!(
        body.admit_form(1, form("text"), &host(), &boot()),
        Err(WorkspaceBodyError::NotLulled)
    );
    assert_eq!(body.evidence(), &before);
}

#[test]
fn repeated_refused_starts_compact_history_without_exhausting_the_body() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    for _ in 0..64 {
        body.propose(plans(&body), &host(), &boot()).unwrap();
        body.lull(&host(), &boot(), None).unwrap();
        persist_archives(&mut body);
        body.evidence().validate().unwrap();
    }
    assert_eq!(body.evidence().body_id, identity);
    assert_eq!(body.evidence().body.state, BodyState::Lulled);
    assert!(
        body.evidence()
            .compaction
            .as_ref()
            .is_some_and(|summary| summary.wakes > 0)
    );
}

#[test]
fn repeated_started_plays_compact_without_growing_the_retained_window() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    for _ in 0..32 {
        let play = start(&mut body);
        body.lull(&host(), &boot(), Some(&play)).unwrap();
        persist_archives(&mut body);
        body.evidence().validate().unwrap();
        assert!(body.evidence().body.sign_ids.len() <= conduit_body::MAX_BODY_SIGNS);
        assert!(body.evidence().wakes.len() <= conduit_body::MAX_BODY_BIOGRAPHY_WAKES);
        assert!(body.evidence().records.len() <= conduit_body::MAX_BODY_BIOGRAPHY_RECORDS);
    }
    let mut restored = WorkspaceBody::open(
        serde_json::from_str(&serde_json::to_string(body.evidence()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(restored.evidence().body_id, identity);
    let summary = restored.evidence().compaction.as_ref().unwrap();
    assert!(summary.wakes > 0);
    assert!(summary.records >= summary.wakes * 5);
    let play = start(&mut restored);
    let realization = restored.realization().unwrap();
    let startup = restored
        .evidence()
        .startup_for_play(&realization.plan, &play)
        .unwrap();
    assert!(!startup.eligible(conduit_body::StartupScope::Body));
    assert!(startup.eligible(conduit_body::StartupScope::Wake));
}

#[test]
fn sealed_history_is_body_bound_chained_and_corruption_explicit() {
    let mut body = born();
    while body.pending_archives().is_empty() {
        body.propose(plans(&body), &host(), &boot()).unwrap();
        body.lull(&host(), &boot(), None).unwrap();
    }
    let first = body.pending_archives()[0].clone();
    first.validate_as_head_of(body.evidence()).unwrap();
    let first_digest = first.digest;
    body.acknowledge_archives(first_digest).unwrap();

    while body.pending_archives().is_empty() {
        body.propose(plans(&body), &host(), &boot()).unwrap();
        body.lull(&host(), &boot(), None).unwrap();
    }
    let second = body.pending_archives()[0].clone();
    assert_eq!(second.previous_digest, Some(first_digest));
    second.validate_as_head_of(body.evidence()).unwrap();

    let mut corrupt = second.clone();
    corrupt.records[0].sequence += 1;
    assert_eq!(
        corrupt.validate(),
        Err(conduit_body::BodyBiographyError::InvalidEvidence)
    );
    let mut foreign = second;
    foreign.body_id = Body::born(
        "source/foreign".into(),
        "checked/foreign".into(),
        99,
        "sign/foreign".into(),
    )
    .unwrap()
    .body_id;
    assert_eq!(
        foreign.validate_as_head_of(body.evidence()),
        Err(conduit_body::BodyBiographyError::InvalidEvidence)
    );
}

#[test]
fn ten_thousand_wakes_keep_one_body_and_a_bounded_active_window() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    let mut sealed_segments = 0u64;
    for cycle in 0..10_000 {
        body.propose(plans(&body), &host(), &boot()).unwrap();
        body.lull(&host(), &boot(), None).unwrap();
        if let Some(head) = body.pending_archives().last() {
            head.validate_as_head_of(body.evidence()).unwrap();
            sealed_segments += body.pending_archives().len() as u64;
            let digest = head.digest;
            body.acknowledge_archives(digest).unwrap();
        }
        if cycle == 2_000 || cycle == 6_000 {
            body.admit_form(
                body.evidence().body.workload_revision,
                form("soak-companion"),
                &host(),
                &boot(),
            )
            .unwrap();
        }
        if cycle == 4_000 || cycle == 8_000 {
            body.remove_form(
                body.evidence().body.workload_revision,
                &form("soak-companion"),
                &host(),
                &boot(),
            )
            .unwrap();
        }
        assert!(body.evidence().body.sign_ids.len() <= conduit_body::MAX_BODY_SIGNS);
        assert!(body.evidence().wakes.len() <= conduit_body::MAX_BODY_BIOGRAPHY_WAKES);
        assert!(body.evidence().records.len() <= conduit_body::MAX_BODY_BIOGRAPHY_RECORDS);
    }
    assert_eq!(body.evidence().body_id, identity);
    assert!(sealed_segments > 1_000);
    assert_eq!(body.evidence().body.workload_revision, 4);
    body.evidence().validate().unwrap();
}

#[test]
fn fresh_boot_reconciles_lost_local_play_as_failure_and_retains_the_same_body() {
    let mut body = born();
    let play = start(&mut body);
    let retained: BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(body.evidence()).unwrap()).unwrap();
    assert!(WorkspaceBody::resume_here(retained.clone(), &host(), &boot()).is_err());
    assert!(
        WorkspaceBody::resume_here(
            retained.clone(),
            &"host/stranger".into(),
            &"boot/fresh".into()
        )
        .is_err()
    );
    let resumed = WorkspaceBody::resume_here(retained, &host(), &"boot/fresh".into()).unwrap();
    assert_eq!(resumed.evidence().body_id, body.evidence().body_id);
    assert_eq!(resumed.evidence().body.state, BodyState::Lulled);
    assert!(resumed.realization().is_none());
    assert_eq!(resumed.evidence().wakes[0].lifecycle, WakeLifecycle::Failed);
    assert_eq!(
        resumed.evidence().wakes[0].plans[0].active_play_id.as_ref(),
        Some(&play.active_play_id)
    );
    assert_eq!(
        resumed.evidence().membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .boot_id,
        BootId::from("boot/fresh")
    );
    resumed.evidence().validate().unwrap();
}

#[test]
fn foreground_selection_changes_neither_body_evidence_nor_running_realization() {
    let mut body = born();
    body.admit_form(0, form("notes"), &host(), &boot()).unwrap();
    assert_eq!(body.foreground(), Some(&form("morse")));
    let play = start(&mut body);
    let before = body.evidence().clone();
    let realization = body.realization().cloned();
    body.select_form(&form("notes")).unwrap();
    assert_eq!(body.foreground(), Some(&form("notes")));
    assert_eq!(body.evidence(), &before);
    assert_eq!(body.realization(), realization.as_ref());
    assert_eq!(body.realization().unwrap().play.as_ref(), Some(&play));
    let mut stale = form("notes");
    stale.source_document_id = "source/unreviewed".into();
    assert_eq!(
        body.select_form(&stale),
        Err(WorkspaceBodyError::UninstalledForm)
    );
    assert_eq!(body.foreground(), Some(&form("notes")));
}

#[test]
fn removing_forms_retains_the_body_and_membership_and_reconciles_foreground() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    let membership = body.evidence().membership.clone();
    body.admit_form(0, form("notes"), &host(), &boot()).unwrap();
    body.select_form(&form("notes")).unwrap();
    let play = start(&mut body);
    let before = body.evidence().clone();
    assert_eq!(
        body.remove_form(1, &form("notes"), &host(), &boot()),
        Err(WorkspaceBodyError::NotLulled)
    );
    assert_eq!(body.evidence(), &before);
    body.lull(&host(), &boot(), Some(&play)).unwrap();
    body.remove_form(1, &form("notes"), &host(), &boot())
        .unwrap();
    assert_eq!(body.foreground(), Some(&form("morse")));
    body.remove_form(2, &form("morse"), &host(), &boot())
        .unwrap();
    assert_eq!(body.foreground(), None);
    assert!(body.evidence().body.workset.is_empty());
    assert_eq!(body.evidence().body.state, BodyState::Lulled);
    assert_eq!(body.evidence().body_id, identity);
    assert_eq!(body.evidence().membership, membership);
    assert_eq!(body.evidence().body.workload_revision, 3);
    let restored: BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(body.evidence()).unwrap()).unwrap();
    assert!(
        WorkspaceBody::open(restored)
            .unwrap()
            .evidence()
            .body
            .workset
            .is_empty()
    );
}

#[test]
fn stale_or_absent_removal_preserves_current_workload_and_evidence() {
    let mut body = born();
    let before = body.evidence().clone();
    assert_eq!(
        body.remove_form(1, &form("morse"), &host(), &boot()),
        Err(WorkspaceBodyError::StaleWorkload)
    );
    assert_eq!(
        body.remove_form(0, &form("morse"), &host(), &"boot/stale".into()),
        Err(WorkspaceBodyError::StaleHost)
    );
    assert!(
        body.remove_form(0, &form("missing"), &host(), &boot())
            .is_err()
    );
    assert_eq!(body.evidence(), &before);
    assert_eq!(body.foreground(), Some(&form("morse")));
}

#[test]
fn library_projects_the_current_workset_and_preserves_exact_indices_when_filtered() {
    use conduit_workspace_model::library::{FormLibrary, LibraryEntry, LibraryRefusal};
    let body = born();
    let library = FormLibrary::new(vec![
        LibraryEntry {
            form: form("morse"),
            title: "Morse".into(),
            search_text: "keyboard light".into(),
        },
        LibraryEntry {
            form: form("notes"),
            title: "Notes".into(),
            search_text: "keyboard text".into(),
        },
    ])
    .unwrap();
    let view = library
        .presentation(&body, 7, "keyboard text")
        .unwrap()
        .lower()
        .unwrap();
    assert_eq!(view.revision, 7);
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "library.use.1")
    );
    assert!(
        !view
            .actions
            .iter()
            .any(|action| action.id.starts_with("library.remove"))
    );
    let view = library.presentation(&body, 8, "").unwrap().lower().unwrap();
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "library.remove.0")
    );
    assert!(matches!(
        library.presentation(&body, 9, &"x".repeat(129)),
        Err(LibraryRefusal::SearchBound)
    ));
}
