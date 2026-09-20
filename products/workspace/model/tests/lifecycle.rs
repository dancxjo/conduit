use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyFormPlan, BodyMembership,
    BodyPlayIdentity, BodyState, FulfillmentReadiness, MembershipProofId, PartId,
    PurposeObligationState, ResidentForm, WakeLifecycle, derive_fulfillment_readiness,
};
use conduit_core::{
    AuthorityGrantId, BootId, ExpandedFormId, FormIdentity, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, PROTOCOL_VERSION, SignId, bind_sign, seal_plan,
};
use conduit_workspace_model::{
    CurrentHostOfferError, CurrentHostOffers, WorkspaceBody, WorkspaceBodyError,
};

#[test]
fn tutorial_builds_an_exact_orifina_request_from_current_body_truth() {
    let body = born();
    let request = conduit_workspace_model::tutorial::generative_request(
        &body,
        "request/orifina/initial".into(),
        11,
        conduit_workspace_model::tutorial::TutorialPlayback::Lulled,
    )
    .unwrap();
    assert_eq!(
        request.policy.template_contract_revision,
        conduit_presentation::ORIFINA_COMPLETION_POLICY_REVISION
    );
    assert_eq!(request.semantic_data.presentation.revision, 11);
    assert_eq!(
        request.semantic_data.presentation.basis.body_id.as_ref(),
        Some(&body.evidence().body.body_id)
    );
    assert!(
        request
            .semantic_data
            .presentation
            .properties
            .iter()
            .any(|property| {
                property.name == "readiness"
                    && property.value
                        == conduit_presentation::PresentationPropertyValue::Text("not-ready".into())
            })
    );
    assert!(
        request
            .semantic_data
            .presentation
            .actions
            .iter()
            .any(|action| action.identity == "body.wake")
    );
    assert!(
        !request
            .semantic_data
            .presentation
            .actions
            .iter()
            .any(|action| action.identity == "body.fulfill")
    );
}

#[test]
fn exact_tutorial_completion_only_presents_fulfillment_as_an_operator_choice() {
    let mut body = born();
    let proposal = body
        .propose(plans(&body), &host(), &boot())
        .unwrap()
        .clone();
    body.fail(
        &host(),
        &boot(),
        vec![conduit_body::WakeRejectionEvidence {
            reason_code: "execution.line-unavailable".into(),
            category: "Connectivity".into(),
            stage: "Body execution".into(),
            resource: "execution-line".into(),
            required: 1,
            available: 0,
            host_id: host(),
            boot_id: boot(),
            plan_id: Some(proposal.plan.plan_id.clone()),
            checked_form_ids: proposal
                .plan
                .forms
                .iter()
                .map(|form| form.form.checked_form_id.clone())
                .collect(),
        }],
    )
    .unwrap();
    let play = start(&mut body);
    body.lull(&host(), &boot(), Some(&play)).unwrap();

    let mut evidence = body.evidence().clone();
    let mut membership = evidence.membership.clone();
    let other_part = PartId::bind(&evidence.body_id, "other", 1).unwrap();
    let other_proof = MembershipProofId::bind("proof/other").unwrap();
    let admitted = membership
        .admit(
            &evidence.body_id,
            membership.revision,
            other_part.clone(),
            other_proof.clone(),
            "sign/admit-other".into(),
        )
        .unwrap();
    let joined = membership
        .observe_present(
            &evidence.body_id,
            membership.revision,
            &other_part,
            AuthenticatedHostObservation {
                host_id: "host/other".into(),
                boot_id: "boot/other".into(),
                offer_generation: OfferGeneration(1),
                proof_id: other_proof,
                sequence: 1,
            },
            "sign/join-other".into(),
        )
        .unwrap();
    let next = evidence.records.last().unwrap().sequence + 1;
    evidence
        .append_membership_events(membership, &[(admitted, next), (joined, next + 1)])
        .unwrap();
    let body = WorkspaceBody::open(evidence).unwrap();
    let request = conduit_workspace_model::tutorial::generative_request(
        &body,
        "request/orifina/ready".into(),
        12,
        conduit_workspace_model::tutorial::TutorialPlayback::Completed,
    )
    .unwrap();
    assert!(
        request
            .semantic_data
            .presentation
            .properties
            .iter()
            .any(|property| {
                property.name == "readiness"
                    && property.value
                        == conduit_presentation::PresentationPropertyValue::Text("ready".into())
            })
    );
    assert_eq!(request.semantic_data.presentation.actions.len(), 1);
    assert_eq!(
        request.semantic_data.presentation.actions[0].identity,
        "body.fulfill"
    );
    assert!(matches!(body.evidence().body.state, BodyState::Lulled));
}

#[test]
fn tutorial_guidance_is_a_renderer_neutral_revision_bound_application_view() {
    let body = born();
    let view = conduit_workspace_model::tutorial::presentation(
        &body,
        7,
        conduit_workspace_model::tutorial::TutorialPlayback::Lulled,
    )
    .unwrap()
    .lower()
    .unwrap();
    assert_eq!(view.revision, 7);
    assert!(view.nodes.iter().any(|node| node.text == "Wake this Body"));
    assert!(view.actions.iter().any(|action| action.id == "body.wake"));
    assert!(view.nodes.iter().any(|node| {
        node.text.contains("Purpose · exact readiness") && node.text.contains("not ready")
    }));
}

#[test]
fn revised_tutorial_uses_the_shared_host_invitation_action() {
    let mut body = born();
    body.admit_form(0, form("notes"), &host(), &boot()).unwrap();
    start(&mut body);
    let view = conduit_workspace_model::tutorial::presentation(
        &body,
        8,
        conduit_workspace_model::tutorial::TutorialPlayback::Playing,
    )
    .unwrap()
    .lower()
    .unwrap();
    assert!(
        view.nodes
            .iter()
            .any(|node| node.text == "Invite another Host")
    );
    assert!(
        view.actions
            .iter()
            .any(|action| action.id == "body.invite-host")
    );
}

#[test]
fn tutorial_purpose_is_derived_from_exact_body_evidence_not_a_chapter_counter() {
    let mut body = born();
    let initial = conduit_workspace_model::tutorial::purpose_state(&body).unwrap();
    assert!(matches!(
        initial.obligations[0].state,
        PurposeObligationState::Satisfied { ref evidence_sign_ids }
            if evidence_sign_ids == &[SignId::from("sign/birth")]
    ));
    assert!(
        initial.obligations[1..]
            .iter()
            .all(|obligation| matches!(obligation.state, PurposeObligationState::Pending))
    );

    start(&mut body);
    let active = conduit_workspace_model::tutorial::purpose_state(&body).unwrap();
    for obligation_id in ["born", "wake", "plan-ready", "play-started"] {
        let obligation = active
            .obligations
            .iter()
            .find(|obligation| obligation.obligation_id == obligation_id)
            .unwrap();
        assert!(matches!(
            obligation.state,
            PurposeObligationState::Satisfied { .. }
        ));
    }
    assert!(matches!(
        derive_fulfillment_readiness(&active).unwrap(),
        FulfillmentReadiness::NotReady { ref reasons, .. }
            if reasons.iter().any(|reason| reason.obligation_id == "add-host")
                && reasons.iter().any(|reason| reason.obligation_id == "repair-fault")
    ));
}

#[test]
fn repaired_wake_advances_tutorial_guidance_from_fault_to_continuity() {
    let mut body = born();
    let proposal = body
        .propose(plans(&body), &host(), &boot())
        .unwrap()
        .clone();
    body.fail(
        &host(),
        &boot(),
        vec![conduit_body::WakeRejectionEvidence {
            reason_code: "execution.line-unavailable".into(),
            category: "Connectivity".into(),
            stage: "Body execution".into(),
            resource: "execution-line".into(),
            required: 1,
            available: 0,
            host_id: host(),
            boot_id: boot(),
            plan_id: Some(proposal.plan.plan_id.clone()),
            checked_form_ids: proposal
                .plan
                .forms
                .iter()
                .map(|form| form.form.checked_form_id.clone())
                .collect(),
        }],
    )
    .unwrap();

    let repair = conduit_workspace_model::tutorial::presentation(
        &body,
        9,
        conduit_workspace_model::tutorial::TutorialPlayback::Refused,
    )
    .unwrap()
    .lower()
    .unwrap();
    assert!(
        repair
            .nodes
            .iter()
            .any(|node| node.text == "Inspect the real fault")
    );
    assert!(
        repair
            .actions
            .iter()
            .any(|action| action.id == "body.inspect-lifecycle")
    );

    start(&mut body);
    let purpose = conduit_workspace_model::tutorial::purpose_state(&body).unwrap();
    assert!(purpose.obligations.iter().any(|obligation| {
        obligation.obligation_id == "repair-fault"
            && matches!(obligation.state, PurposeObligationState::Satisfied { .. })
    }));
    let repaired = conduit_workspace_model::tutorial::presentation(
        &body,
        10,
        conduit_workspace_model::tutorial::TutorialPlayback::Playing,
    )
    .unwrap()
    .lower()
    .unwrap();
    assert!(
        repaired
            .nodes
            .iter()
            .any(|node| node.text == "The same Body woke again")
    );
    assert!(
        !repaired
            .nodes
            .iter()
            .any(|node| node.text == "Inspect the real fault")
    );
}

fn host() -> HostId {
    "host/here".into()
}

#[test]
fn invitation_transfer_methods_share_one_revision_bound_semantic_identity() {
    let transfer_uri =
        "https://example.invalid/workspace/#body-invitation=header.payload.signature";
    let semantic = conduit_workspace_model::invitation::InvitationPresentation {
        invitation_id: "invitation/one",
        body_id: "body/one",
        body_name: "Orifina",
        expires_at_millis: 42,
        transfer_uri,
        clipboard_available: true,
        share_available: false,
    }
    .view(12)
    .unwrap();
    let view = semantic.lower().unwrap();
    assert_eq!(view.revision, 12);
    for identity in [
        "invitation.show-qr",
        "invitation.copy-link",
        "invitation.dismiss",
    ] {
        assert!(view.actions.iter().any(|action| action.id == identity));
    }
    assert!(
        !view
            .actions
            .iter()
            .any(|action| action.id == "invitation.share")
    );
    let link_nodes: Vec<_> = view
        .nodes
        .iter()
        .filter(|node| node.key == "invitation-link")
        .collect();
    assert_eq!(link_nodes.len(), 1);
    assert_eq!(link_nodes[0].value, transfer_uri);
}

#[test]
fn explicit_fulfillment_is_terminal_attributable_and_inspectable_after_restore() {
    let mut body = born();
    body.fulfill(
        &host(),
        &boot(),
        AuthorityGrantId::from("grant/operator-finish"),
        "operator/alice".into(),
    )
    .unwrap();

    assert!(matches!(
        body.evidence().body.state,
        BodyState::Fulfilled { .. }
    ));
    assert!(body.evidence().membership.fulfilled_sign_id.is_some());
    let last = body.evidence().records.last().unwrap();
    assert!(matches!(
        &last.kind,
        conduit_body::BodyBiographyRecordKind::Fulfilled {
            authority_grant_id,
            attribution,
            settled_obligations,
            ..
        } if authority_grant_id.as_str() == "grant/operator-finish"
            && attribution == "operator/alice"
            && settled_obligations.len() == 1
            && settled_obligations[0].obligation_id == "obligation/workspace-runtime-empty"
    ));

    assert_eq!(
        body.admit_form(0, form("notes"), &host(), &boot()),
        Err(WorkspaceBodyError::Lifecycle(
            conduit_body::BodyLifecycleError::Fulfilled
        ))
    );
    assert_eq!(
        body.fulfill(
            &host(),
            &boot(),
            AuthorityGrantId::from("grant/operator-finish"),
            "operator/alice".into(),
        ),
        Err(WorkspaceBodyError::Lifecycle(
            conduit_body::BodyLifecycleError::Fulfilled
        ))
    );
    let restored: BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(body.evidence()).unwrap()).unwrap();
    let restored = WorkspaceBody::open(restored).unwrap();
    assert!(matches!(
        restored.evidence().body.state,
        BodyState::Fulfilled { .. }
    ));
}

#[test]
fn fulfillment_refuses_until_the_current_play_is_retired() {
    let mut body = born();
    let play = start(&mut body);
    let before = body.evidence().clone();
    assert_eq!(
        body.fulfill(
            &host(),
            &boot(),
            AuthorityGrantId::from("grant/operator-finish"),
            "operator/alice".into(),
        ),
        Err(WorkspaceBodyError::NotLulled)
    );
    assert_eq!(body.evidence(), &before);
    body.lull(&host(), &boot(), Some(&play)).unwrap();
    body.fulfill(
        &host(),
        &boot(),
        AuthorityGrantId::from("grant/operator-finish"),
        "operator/alice".into(),
    )
    .unwrap();
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
fn advertisement(host_id: HostId, boot_id: BootId, generation: u64) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("test/current-offers@1"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
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
fn current_host_offers_require_exact_membership_and_are_reconciled_after_boot_loss() {
    let body = born();
    let mut offers = CurrentHostOffers::new();
    offers
        .observe(body.evidence(), advertisement(host(), boot(), 1))
        .unwrap();
    assert_eq!(offers.hosts().len(), 1);
    assert_eq!(
        offers.observe(
            body.evidence(),
            advertisement(host(), "boot/stale".into(), 1)
        ),
        Err(CurrentHostOfferError::NotCurrentMember)
    );
    assert_eq!(
        offers.observe(body.evidence(), advertisement(host(), boot(), 2)),
        Err(CurrentHostOfferError::NotCurrentMember)
    );
    let resumed =
        WorkspaceBody::resume_here(body.evidence().clone(), &host(), &"boot/restarted".into())
            .unwrap();
    offers.reconcile(resumed.evidence());
    assert!(offers.hosts().is_empty());
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
fn quantitative_pre_play_refusal_survives_into_the_body_biography() {
    let mut body = born();
    let proposal = body
        .propose(plans(&body), &host(), &boot())
        .unwrap()
        .clone();
    let rejection = conduit_body::WakeRejectionEvidence {
        reason_code: "lowering.capacity".into(),
        category: "Capacity".into(),
        stage: "Body lowering".into(),
        resource: "nodes".into(),
        required: 33,
        available: 32,
        host_id: host(),
        boot_id: boot(),
        plan_id: Some(proposal.plan.plan_id.clone()),
        checked_form_ids: proposal
            .plan
            .forms
            .iter()
            .map(|form| form.form.checked_form_id.clone())
            .collect(),
    };
    body.fail(&host(), &boot(), vec![rejection.clone()])
        .unwrap();
    assert!(body.realization().is_none());
    let wake = body.evidence().wakes.last().unwrap();
    assert_eq!(wake.lifecycle, conduit_body::WakeLifecycle::Failed);
    assert_eq!(wake.plans.len(), 1);
    assert_eq!(wake.plans[0].plan_id, proposal.plan.plan_id);
    assert_eq!(
        wake.plans[0].state,
        conduit_body::WakePlanState::AwaitingPlay
    );
    assert_eq!(wake.rejections, vec![rejection]);
}

#[test]
fn refusal_evidence_cannot_claim_unrelated_host_plan_or_form_provenance() {
    let mut body = born();
    let proposal = body
        .propose(plans(&body), &host(), &boot())
        .unwrap()
        .clone();
    let valid = conduit_body::WakeRejectionEvidence {
        reason_code: "lowering.capacity".into(),
        category: "Capacity".into(),
        stage: "Body lowering".into(),
        resource: "nodes".into(),
        required: 33,
        available: 32,
        host_id: host(),
        boot_id: boot(),
        plan_id: Some(proposal.plan.plan_id.clone()),
        checked_form_ids: proposal
            .plan
            .forms
            .iter()
            .map(|form| form.form.checked_form_id.clone())
            .collect(),
    };
    for forged in [
        conduit_body::WakeRejectionEvidence {
            host_id: "host/other".into(),
            ..valid.clone()
        },
        conduit_body::WakeRejectionEvidence {
            plan_id: Some("plan/other".into()),
            ..valid.clone()
        },
        conduit_body::WakeRejectionEvidence {
            checked_form_ids: vec!["form/other".into()],
            ..valid.clone()
        },
    ] {
        assert_eq!(
            body.fail(&host(), &boot(), vec![forged]),
            Err(WorkspaceBodyError::StalePlay)
        );
        assert!(body.realization().is_some());
    }
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
    let page = conduit_body::BodyBiographyArchiveSegment::load_page(
        body.evidence(),
        vec![second.clone(), first.clone()],
    )
    .unwrap();
    assert_eq!(page.segments.len(), 2);
    assert_eq!(page.next_digest, None);

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
fn repeated_form_changes_roll_over_without_rebirth_or_active_growth() {
    let mut body = born();
    let identity = body.evidence().body_id.clone();
    let companion = form("rolling-companion");
    let mut archived_form_events = 0usize;
    for cycle in 0..200 {
        let revision = body.evidence().body.workload_revision;
        if cycle % 2 == 0 {
            body.admit_form(revision, companion.clone(), &host(), &boot())
                .unwrap();
        } else {
            body.remove_form(revision, &companion, &host(), &boot())
                .unwrap();
        }
        for segment in body.pending_archives() {
            segment.validate().unwrap();
            archived_form_events += segment.body_events.len();
            assert!(segment.membership_events.is_empty());
        }
        persist_archives(&mut body);
        assert!(body.evidence().body.sign_ids.len() <= conduit_body::MAX_BODY_SIGNS);
        assert!(body.evidence().records.len() <= conduit_body::MAX_BODY_BIOGRAPHY_RECORDS);
    }
    assert_eq!(body.evidence().body_id, identity);
    assert_eq!(body.evidence().body.workload_revision, 200);
    assert!(archived_form_events >= 100);
    body.evidence().validate().unwrap();
}

#[test]
fn repeated_host_continuity_rolls_membership_history_into_exact_segments() {
    let initial = born();
    let identity = initial.evidence().body_id.clone();
    let mut evidence = initial.evidence().clone();
    let mut current_boot = boot();
    let mut archived_membership_events = 0usize;
    for cycle in 0..100 {
        let next_boot = BootId::from(format!("boot/continuity-{cycle}"));
        let mut resumed = WorkspaceBody::resume_here(evidence, &host(), &next_boot).unwrap();
        for segment in resumed.pending_archives() {
            segment.validate().unwrap();
            archived_membership_events += segment.membership_events.len();
            assert!(segment.body_events.is_empty());
        }
        persist_archives(&mut resumed);
        assert!(resumed.evidence().membership.events.len() <= conduit_body::MAX_MEMBERSHIP_EVENTS);
        assert!(resumed.evidence().records.len() <= conduit_body::MAX_BODY_BIOGRAPHY_RECORDS);
        evidence = resumed.evidence().clone();
        current_boot = next_boot;
    }
    assert_eq!(evidence.body_id, identity);
    assert_eq!(
        evidence.membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .boot_id,
        current_boot
    );
    assert!(archived_membership_events >= 64);
    evidence.validate().unwrap();
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
    use conduit_workspace_model::library::{
        FormLibrary, LibraryAvailability, LibraryEntry, LibraryRefusal,
    };
    let body = born();
    let library = FormLibrary::new(vec![
        LibraryEntry {
            form: form("morse"),
            title: "Morse".into(),
            search_text: "keyboard light".into(),
            availability: LibraryAvailability::Available,
            graceful_fallback: None,
        },
        LibraryEntry {
            form: form("notes"),
            title: "Notes".into(),
            search_text: "keyboard text".into(),
            availability: LibraryAvailability::NeedsCapability(
                "Needs a text model realization.".into(),
            ),
            graceful_fallback: Some(conduit_workspace_model::library::LibraryFallback {
                title: "Keyboard Notes".into(),
                availability: LibraryAvailability::Available,
            }),
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
        !view
            .actions
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

#[test]
fn library_keeps_reviewed_forms_visible_when_the_body_is_at_capacity() {
    use conduit_body::MAX_BODY_FORMS;
    use conduit_workspace_model::library::{FormLibrary, LibraryAvailability, LibraryEntry};

    let mut body = born();
    for index in 1..MAX_BODY_FORMS {
        body.admit_form(
            body.evidence().body.workload_revision,
            form(&format!("resident-{index}")),
            &host(),
            &boot(),
        )
        .unwrap();
        persist_archives(&mut body);
    }
    let library = FormLibrary::new(vec![
        LibraryEntry {
            form: form("morse"),
            title: "Morse".into(),
            search_text: "resident".into(),
            availability: LibraryAvailability::Available,
            graceful_fallback: None,
        },
        LibraryEntry {
            form: form("another"),
            title: "Another reviewed Form".into(),
            search_text: "candidate".into(),
            availability: LibraryAvailability::Available,
            graceful_fallback: None,
        },
    ])
    .unwrap();

    let semantic = library.presentation(&body, 10, "candidate").unwrap();
    let encoded = format!("{semantic:?}");
    assert!(encoded.contains("Body at capacity"));
    assert!(encoded.contains("Remove a Form before adding another"));
    let lowered = semantic.lower().unwrap();
    assert!(
        !lowered
            .actions
            .iter()
            .any(|action| action.id == "library.use.1")
    );

    let resident = library
        .presentation(&body, 11, "resident")
        .unwrap()
        .lower()
        .unwrap();
    assert!(
        resident
            .actions
            .iter()
            .any(|action| action.id == "library.use.0")
    );
    assert!(
        resident
            .actions
            .iter()
            .any(|action| action.id == "library.remove.0")
    );
}
