use conduit_body::{
    Body, BodyLifecycleError, HoldPolicy, HoldReasonId, HoldReleaseAuthority, HoldReleaseOutcome,
    HoldSourceId, PlanningBasis, WakeLifecycle, WakeLifecycleEvent, WakePlanState,
    MAX_HOLD_BASIS_SIGNS,
};
use conduit_core::{
    mandatory_sign_storage_requirement, seal_plan, ArtifactId, BootId, CancellationPolicy,
    CapabilityId, CapabilityLimits, CheckedFormId, ExecutionProfileId, ExpandedFormId,
    ExpectedSign, ExpectedTerminal, FormIdentity, FragmentId, GearId, HostId, ImplementationId,
    KindContractRevision, KindId, OfferGeneration, PlacementId, Plan, PlanFragment, PlanId,
    PlannedGear, ResourceBinding, ResourceClassId, ResourcePoolId, SignId, SignStorageBudget,
    SourceDocumentId, TerminalPolicy,
};

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source-a"),
        CheckedFormId::from("checked-a"),
        1,
        SignId::from("born"),
    )
    .unwrap()
}

fn exact_plan(label: &str, host: &str) -> Plan {
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    let fragment = PlanFragment {
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source-a"),
        checked_form_id: CheckedFormId::from("checked-a"),
        expanded_form_id: ExpandedFormId::from(label),
        realization_backs: Vec::new(),
        host_id: HostId::from(host),
        boot_id: BootId::from(format!("{host}-boot")),
        offer_generation: OfferGeneration(7),
        placements: vec![PlannedGear {
            placement_id: PlacementId::from(format!("{label}-placement")),
            gear_id: GearId::from("gear-a"),
            kind_id: KindId::from("test/kind"),
            kind_contract_revision: KindContractRevision::from("test/kind@1"),
            execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
            configuration: vec![],
            host_id: HostId::from(host),
            boot_id: BootId::from(format!("{host}-boot")),
            offer_generation: OfferGeneration(7),
            capability_id: CapabilityId::from(format!("{host}/capability")),
            implementation_id: ImplementationId::from(format!("{host}/implementation")),
            artifact_id: ArtifactId::from(format!("{host}/artifact")),
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 16,
            },
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resources: vec![ResourceBinding {
                content: None,
                pool_id: ResourcePoolId::from(format!("{host}/timer-pool")),
                class_id: ResourceClassId::from("test/timer"),
                units: 1,
                protected: None,
                compute: None,
            }],
            authority: vec![],
            pool_references: vec![],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: Vec::new(),
        connections: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![ExpectedTerminal::PlanCompleted],
        expected_sign: expected_sign.clone(),
        sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap_or(
            SignStorageBudget {
                item_capacity: 0,
                byte_capacity: 0,
            },
        ),
        plan_fragments: vec![],
    };
    seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source-a"),
            checked_form_id: CheckedFormId::from("checked-a"),
            expanded_form_id: ExpandedFormId::from(label),
        },
        vec![fragment],
    )
}

fn basis(label: &str) -> PlanningBasis {
    PlanningBasis::new(vec![
        SignId::from(format!("{label}/host-offer")),
        SignId::from(format!("{label}/resource-ready")),
        SignId::from(format!("{label}/line-ready")),
    ])
    .unwrap()
}

fn policy(hold_replacement_plan: bool) -> HoldPolicy {
    HoldPolicy {
        reason: HoldReasonId::from("operator-inspection"),
        source: HoldSourceId::from("controller/policy-a"),
        release_authority: HoldReleaseAuthority::new(conduit_core::AuthorityGrantId::from(
            "grant/release-policy-a",
        )),
        hold_replacement_plan,
    }
}

#[test]
fn wake_can_hold_exact_plan_with_no_play_and_inspect_current_validity() {
    let (awake, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let plan = exact_plan("plan-a", "host-a");
    let planned_basis = basis("basis-a");
    let policy = policy(true);
    let held = wake
        .plan_held(
            &plan,
            planned_basis.clone(),
            policy.clone(),
            SignId::from("held-a"),
        )
        .unwrap();

    assert!(matches!(awake.state, conduit_body::BodyState::Awake { .. }));
    assert_eq!(held.lifecycle, WakeLifecycle::Held);
    assert_eq!(held.plans[0].state, WakePlanState::Held);
    assert_eq!(held.plans[0].active_play_id, None);
    assert!(held
        .events
        .iter()
        .all(|event| !matches!(event, WakeLifecycleEvent::PlayStarted { .. })));

    let inspection = held.inspect_hold(&planned_basis).unwrap().unwrap();
    assert_eq!(inspection.plan, &plan);
    assert_eq!(inspection.plan.fragments[0].host_id.as_str(), "host-a");
    assert_eq!(
        inspection.plan.fragments[0].placements[0].resources[0]
            .pool_id
            .as_str(),
        "host-a/timer-pool"
    );
    assert_eq!(inspection.basis.sign_ids(), planned_basis.sign_ids());
    assert_eq!(inspection.policy, &policy);
    assert!(inspection.remains_valid);
    assert!(
        !held
            .inspect_hold(&basis("basis-new"))
            .unwrap()
            .unwrap()
            .remains_valid
    );
    assert_eq!(held.validate(), Ok(()));
}

#[test]
fn authorized_release_revalidates_then_creates_the_first_play_identity() {
    let (_, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let plan = exact_plan("plan-a", "host-a");
    let planned_basis = basis("basis-a");
    let policy = policy(true);
    let held = wake
        .plan_held(
            &plan,
            planned_basis.clone(),
            policy.clone(),
            SignId::from("held-a"),
        )
        .unwrap();

    let wrong = HoldReleaseAuthority::new(conduit_core::AuthorityGrantId::from("grant/wrong"));
    assert_eq!(
        held.release_hold(
            &wrong,
            &planned_basis,
            &HostId::from("controller-host"),
            &BootId::from("controller-boot"),
            1,
            SignId::from("denied"),
        ),
        Err(BodyLifecycleError::AuthorityDenied)
    );
    assert_eq!(held.plans[0].active_play_id, None);

    let HoldReleaseOutcome::PlayStarted { wake, active_play } = held
        .release_hold(
            &policy.release_authority,
            &planned_basis,
            &HostId::from("controller-host"),
            &BootId::from("controller-boot"),
            1,
            SignId::from("released"),
        )
        .unwrap()
    else {
        panic!("valid basis must begin the held Plan");
    };
    assert_eq!(wake.lifecycle, WakeLifecycle::Playing);
    assert_eq!(
        wake.plans[0].active_play_id,
        Some(active_play.active_play_id.clone())
    );
    assert_eq!(active_play.plan_id, plan.plan_id);
    assert!(matches!(
        wake.events.last(),
        Some(WakeLifecycleEvent::HeldPlanReleased { active_play_id, .. })
            if active_play_id == &active_play.active_play_id
    ));
    assert_eq!(wake.inspect_hold(&planned_basis), Ok(None));
    assert_eq!(wake.validate(), Ok(()));
}

#[test]
fn stale_release_requires_replan_and_persistent_policy_reholds_replacement() {
    let (_, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let plan_a = exact_plan("plan-a", "host-a");
    let policy = policy(true);
    let held = wake
        .plan_held(
            &plan_a,
            basis("basis-a"),
            policy.clone(),
            SignId::from("held-a"),
        )
        .unwrap();

    let HoldReleaseOutcome::ReplanRequired { wake: stale } = held
        .release_hold(
            &policy.release_authority,
            &basis("basis-b"),
            &HostId::from("controller-host"),
            &BootId::from("controller-boot"),
            1,
            SignId::from("stale-a"),
        )
        .unwrap()
    else {
        panic!("changed Signs must not start a Play");
    };
    assert_eq!(stale.lifecycle, WakeLifecycle::AwaitingReplacement);
    assert_eq!(stale.plans[0].state, WakePlanState::Invalidated);
    assert_eq!(stale.plans[0].active_play_id, None);

    let plan_b = exact_plan("plan-b", "host-b");
    assert_eq!(
        stale.plan_ready(&plan_b, SignId::from("bypass-hold")),
        Err(BodyLifecycleError::HoldRequired)
    );
    let replacement = stale
        .plan_held(&plan_b, basis("basis-b"), policy, SignId::from("held-b"))
        .unwrap();
    assert_eq!(replacement.lifecycle, WakeLifecycle::Held);
    assert_eq!(replacement.plans[0].state, WakePlanState::Superseded);
    assert_eq!(replacement.plans[1].state, WakePlanState::Held);
    assert_eq!(replacement.plans[1].active_play_id, None);
    assert_eq!(replacement.validate(), Ok(()));
}

#[test]
fn direct_hold_and_lull_are_distinct_and_an_active_play_cannot_be_held() {
    let (_, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let plan_a = exact_plan("plan-a", "host-a");
    let direct = wake.plan_ready(&plan_a, SignId::from("plan-a")).unwrap();
    let active = conduit_core::bind_active_play(
        &plan_a.plan_id,
        &HostId::from("host-a"),
        &BootId::from("host-a-boot"),
        1,
    );
    let playing = direct
        .play_started(&active, SignId::from("playing-a"))
        .unwrap();
    assert_eq!(
        playing.plan_held(
            &exact_plan("plan-b", "host-b"),
            basis("basis-b"),
            policy(true),
            SignId::from("pause-masquerade"),
        ),
        Err(BodyLifecycleError::InvalidTransition)
    );
    let lulled = playing.lull(SignId::from("lulled")).unwrap();
    assert_eq!(lulled.lifecycle, WakeLifecycle::Lulled);
    assert_ne!(lulled.lifecycle, WakeLifecycle::Held);
}

#[test]
fn nonpersistent_hold_allows_direct_replacement_after_stale_release() {
    let (_, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let policy = policy(false);
    let held = wake
        .plan_held(
            &exact_plan("plan-a", "host-a"),
            basis("basis-a"),
            policy.clone(),
            SignId::from("held-a"),
        )
        .unwrap();
    let HoldReleaseOutcome::ReplanRequired { wake: stale } = held
        .release_hold(
            &policy.release_authority,
            &basis("basis-b"),
            &HostId::from("controller-host"),
            &BootId::from("controller-boot"),
            1,
            SignId::from("stale-a"),
        )
        .unwrap()
    else {
        panic!("changed basis requires replacement");
    };
    let replacement = stale
        .plan_ready(&exact_plan("plan-b", "host-b"), SignId::from("plan-b"))
        .unwrap();
    assert_eq!(replacement.lifecycle, WakeLifecycle::AwaitingPlay);
}

#[test]
fn basis_bounds_and_held_transition_tampering_fail_closed() {
    assert_eq!(
        PlanningBasis::new(vec![]),
        Err(BodyLifecycleError::InvalidPlanningBasis)
    );
    assert_eq!(
        PlanningBasis::new(vec![SignId::from("same"), SignId::from("same")]),
        Err(BodyLifecycleError::InvalidPlanningBasis)
    );
    assert_eq!(
        PlanningBasis::new(
            (0..=MAX_HOLD_BASIS_SIGNS)
                .map(|index| SignId::from(format!("sign-{index}")))
                .collect(),
        ),
        Err(BodyLifecycleError::PlanningBasisCapacityExhausted)
    );

    let (_, wake) = body().wake(1, SignId::from("wake")).unwrap();
    let policy = policy(true);
    let held = wake
        .plan_held(
            &exact_plan("plan-a", "host-a"),
            basis("basis-a"),
            policy.clone(),
            SignId::from("held-a"),
        )
        .unwrap();
    let HoldReleaseOutcome::ReplanRequired { wake: mut stale } = held
        .release_hold(
            &policy.release_authority,
            &basis("basis-b"),
            &HostId::from("controller-host"),
            &BootId::from("controller-boot"),
            1,
            SignId::from("stale-a"),
        )
        .unwrap()
    else {
        panic!("changed basis requires replacement");
    };
    let Some(WakeLifecycleEvent::HeldPlanInvalidated {
        current_basis_sign_ids,
        ..
    }) = stale.events.last_mut()
    else {
        panic!("stale release must retain exact invalidation evidence");
    };
    current_basis_sign_ids.clear();
    assert_eq!(stale.validate(), Err(BodyLifecycleError::InvalidTransition));
}

#[test]
fn architecture_contract_keeps_hold_between_plan_and_play() {
    let document = include_str!("../../../docs/architecture/pre-play-hold.md");
    for required in [
        "BODY -> WAKE -> PLAN -> [optional HOLD] -> PLAY",
        "no `ActivePlayId` exists",
        "HeldPlanReleased",
        "HeldPlanInvalidated",
        "AwaitingReplacement",
        "HoldRequired",
        "cannot masquerade as runtime pause",
        "automatically by waiting in HOLD",
    ] {
        assert!(document.contains(required), "missing {required}");
    }
}
