use conduit_body::{
    Body, BodyBiographyEvidence, BodyBiographyRecordKind, BodyFulfillment, BodyLifecycleError,
    BodyLifecycleEvent, BodyMembership, BodyMembershipRevision, BodyState, FulfillmentObligation,
    MembershipProofId, MembershipRefusal, PartId, ResidentForm,
};
use conduit_core::{AuthorityGrantId, CheckedFormId, SignId, SourceDocumentId};

fn born() -> Body {
    Body::born(
        SourceDocumentId::from("source/seed"),
        CheckedFormId::from("checked/seed"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

fn fulfillment(final_wake_id: Option<conduit_body::WakeId>) -> BodyFulfillment {
    BodyFulfillment {
        final_wake_id,
        authority_grant_id: AuthorityGrantId::from("grant/operator-fulfill"),
        attribution: "operator/alice".into(),
        settled_obligations: vec![FulfillmentObligation {
            obligation_id: "obligation/runtime-released".into(),
            settlement_sign_id: SignId::from("sign/runtime-released"),
        }],
    }
}

#[test]
fn fulfillment_is_attributable_terminal_truth_after_retained_lull() {
    let original = born();
    let identity = original.body_id.clone();
    let (awake, wake) = original.wake(1, SignId::from("sign/woke")).unwrap();
    assert_eq!(
        awake.fulfill(
            fulfillment(Some(wake.wake_id.clone())),
            SignId::from("sign/too-early")
        ),
        Err(BodyLifecycleError::InvalidTransition)
    );

    let lulled_wake = wake.lull(SignId::from("sign/lulled")).unwrap();
    let lulled = awake
        .retain_after_lull(&lulled_wake, SignId::from("sign/lull-retained"))
        .unwrap();
    let fulfilled = lulled
        .fulfill(
            fulfillment(Some(lulled_wake.wake_id.clone())),
            SignId::from("sign/fulfilled"),
        )
        .unwrap();

    assert_eq!(fulfilled.body_id, identity);
    assert!(matches!(
        fulfilled.state,
        BodyState::Fulfilled { ref sign_id } if sign_id.as_str() == "sign/fulfilled"
    ));
    assert!(matches!(
        fulfilled.events.last(),
        Some(BodyLifecycleEvent::Fulfilled {
            final_workload_revision: 0,
            final_wake_id: Some(wake_id),
            attribution,
            settled_obligations,
            ..
        }) if wake_id == &lulled_wake.wake_id
            && attribution == "operator/alice"
            && settled_obligations.len() == 1
    ));
    fulfilled.validate().unwrap();

    assert_eq!(
        fulfilled.wake(2, SignId::from("sign/revive")),
        Err(BodyLifecycleError::Fulfilled)
    );
    assert_eq!(
        fulfilled.admit_form(
            ResidentForm::new(
                SourceDocumentId::from("source/new"),
                CheckedFormId::from("checked/new"),
            ),
            SignId::from("sign/add"),
        ),
        Err(BodyLifecycleError::Fulfilled)
    );
    assert_eq!(
        fulfilled.remove_form(
            &ResidentForm::new(
                SourceDocumentId::from("source/seed"),
                CheckedFormId::from("checked/seed"),
            ),
            SignId::from("sign/remove"),
        ),
        Err(BodyLifecycleError::Fulfilled)
    );
    assert_eq!(
        fulfilled.fulfill(fulfillment(None), SignId::from("sign/again")),
        Err(BodyLifecycleError::Fulfilled)
    );
}

#[test]
fn incomplete_or_tampered_fulfillment_never_becomes_terminal_truth() {
    let body = born();
    let no_cleanup_required = BodyFulfillment {
        final_wake_id: None,
        authority_grant_id: AuthorityGrantId::from("grant/operator-fulfill"),
        attribution: "operator/alice".into(),
        settled_obligations: Vec::new(),
    };
    body.fulfill(no_cleanup_required, SignId::from("sign/no-cleanup-needed"))
        .unwrap();

    let mut missing_authority = fulfillment(None);
    missing_authority.authority_grant_id = AuthorityGrantId::from("");
    assert_eq!(
        body.fulfill(missing_authority, SignId::from("sign/fulfilled")),
        Err(BodyLifecycleError::UnsettledObligations)
    );
    assert_eq!(body.state, BodyState::Lulled);

    let mut duplicate = fulfillment(None);
    duplicate
        .settled_obligations
        .push(duplicate.settled_obligations[0].clone());
    assert_eq!(
        body.fulfill(duplicate, SignId::from("sign/fulfilled")),
        Err(BodyLifecycleError::UnsettledObligations)
    );

    let mut fulfilled = body
        .fulfill(fulfillment(None), SignId::from("sign/fulfilled"))
        .unwrap();
    if let BodyLifecycleEvent::Fulfilled {
        final_workload_revision,
        ..
    } = fulfilled.events.last_mut().unwrap()
    {
        *final_workload_revision = 1;
    }
    assert_eq!(
        fulfilled.validate(),
        Err(BodyLifecycleError::InvalidTransition)
    );
}

#[test]
fn biography_and_compaction_retain_exact_terminal_provenance() {
    let seed = born();
    let mut biography = BodyBiographyEvidence::born(
        seed.clone(),
        BodyMembership::new(seed.body_id.clone()).unwrap(),
        "Orifina Dawnheart".into(),
    )
    .unwrap();
    let changed = seed
        .admit_form(
            ResidentForm::new(
                SourceDocumentId::from("source/useful"),
                CheckedFormId::from("checked/useful"),
            ),
            SignId::from("sign/form-admitted"),
        )
        .unwrap();
    let fulfilled = changed
        .fulfill(fulfillment(None), SignId::from("sign/fulfilled"))
        .unwrap();
    biography
        .append_body_lifecycle_events(
            fulfilled,
            &[
                (SignId::from("sign/form-admitted"), 2),
                (SignId::from("sign/fulfilled"), 3),
            ],
        )
        .unwrap();
    assert!(matches!(
        biography.records.last().unwrap().kind,
        BodyBiographyRecordKind::Fulfilled {
            final_workload_revision: 1,
            ..
        }
    ));

    let segment = biography
        .seal_body_workload_history()
        .unwrap()
        .expect("workload prefix is archived");
    assert_eq!(segment.body_events.len(), 1);
    assert!(matches!(biography.body.state, BodyState::Fulfilled { .. }));
    assert!(matches!(
        biography.body.events.last(),
        Some(BodyLifecycleEvent::Fulfilled { .. })
    ));
    assert!(matches!(
        biography.records.last().unwrap().kind,
        BodyBiographyRecordKind::Fulfilled { .. }
    ));
    biography.validate().unwrap();
    assert_eq!(
        biography.membership.admit(
            &biography.body_id,
            BodyMembershipRevision(0),
            PartId::bind(&biography.body_id, "late", 1).unwrap(),
            MembershipProofId::bind("proof/late").unwrap(),
            SignId::from("sign/late-part"),
        ),
        Err(MembershipRefusal::BodyFulfilled)
    );
}
