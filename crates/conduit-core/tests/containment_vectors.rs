use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeControlKind, AdministrativeControlRecord,
    AdministrativeExecution, AdministrativePrincipal, AdministrativeProof, AdministrativeProposal,
    AdministrativeSubject, AdministrativeSupportEdge, CONTAINMENT_POLICY_SCHEMA_VERSION,
    ContainmentContext, ContainmentDisposition, ContainmentPolicy, ContainmentReason,
    ContainmentReasonNode, DelegationEnvelope, Id, PinnedDescriptor, ResourceRef, ResourceSelector,
    SemanticHash, validate_administrative_proof, validate_control_record,
    validate_delegation_narrowing, validate_effect_containment, validate_reason_tree,
    validate_recovery_narrowing, validate_support_graph,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn principal(
    entity: &'static str,
    key: &'static str,
    plan: u8,
) -> AdministrativePrincipal<'static> {
    AdministrativePrincipal {
        realm: Id("realm.alpha"),
        entity: Id(entity),
        key: Id(key),
        profile: pin("profile.member", 10),
        source_plan: hash(plan),
        source_epoch: 7,
    }
}

fn subject(entity: &'static str, plan: u8) -> AdministrativeSubject<'static> {
    AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id(entity),
        plan: hash(plan),
        epoch: 7,
        artifact: None,
        budget: None,
    }
}

#[derive(Clone, Copy)]
struct Case {
    requester: AdministrativePrincipal<'static>,
    beneficiary: AdministrativeSubject<'static>,
    predecessor_plan: Option<SemanticHash>,
    approver_one: AdministrativePrincipal<'static>,
    approver_two: AdministrativePrincipal<'static>,
    committer: AdministrativePrincipal<'static>,
    executor: AdministrativePrincipal<'static>,
    policy_approver_count: usize,
    approval_count: usize,
    minimum_approvals: u8,
    minimum_failure_domains: u8,
    shared_failure_domain: bool,
    first_status: AdministrativeApprovalStatus,
    duplicate_approver: bool,
    replay_approval: bool,
    now_tick: u64,
    proposal_expires_at_tick: u64,
    delegation_ceiling: Option<DelegationEnvelope<'static>>,
    requested_delegation: Option<DelegationEnvelope<'static>>,
    protected_handle: Option<PinnedDescriptor<'static>>,
    policy_ceremony: Option<PinnedDescriptor<'static>>,
    proposal_ceremony: Option<PinnedDescriptor<'static>>,
    context_subject: Option<AdministrativeSubject<'static>>,
}

impl Default for Case {
    fn default() -> Self {
        Self {
            requester: principal("requester", "key.requester", 1),
            beneficiary: subject("target", 2),
            predecessor_plan: None,
            approver_one: principal("approver.one", "key.approver.one", 3),
            approver_two: principal("approver.two", "key.approver.two", 4),
            committer: principal("committer", "key.committer", 5),
            executor: principal("executor", "key.executor", 6),
            policy_approver_count: 1,
            approval_count: 1,
            minimum_approvals: 1,
            minimum_failure_domains: 1,
            shared_failure_domain: false,
            first_status: AdministrativeApprovalStatus::Current,
            duplicate_approver: false,
            replay_approval: false,
            now_tick: 20,
            proposal_expires_at_tick: 40,
            delegation_ceiling: None,
            requested_delegation: None,
            protected_handle: None,
            policy_ceremony: None,
            proposal_ceremony: None,
            context_subject: None,
        }
    }
}

fn run(case: Case) -> Result<(), ContainmentReason> {
    let domain_one = pin("failure.rack.one", 20);
    let domain_two = if case.shared_failure_domain {
        domain_one
    } else {
        pin("failure.rack.two", 21)
    };
    let second_principal = if case.duplicate_approver {
        case.approver_one
    } else {
        case.approver_two
    };
    let approvers = [
        AdministrativeApprover {
            realm: case.approver_one.realm,
            entity: case.approver_one.entity,
            key: case.approver_one.key,
            profile: case.approver_one.profile,
            failure_domain: domain_one,
        },
        AdministrativeApprover {
            realm: second_principal.realm,
            entity: second_principal.entity,
            key: second_principal.key,
            profile: second_principal.profile,
            failure_domain: domain_two,
        },
    ];
    let effect_class = pin("effect.admin", 30);
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.containment", 31),
        effect_class,
        approvers: &approvers[..case.policy_approver_count],
        committer: AdministrativeApprover {
            realm: Id("realm.alpha"),
            entity: Id("committer"),
            key: Id("key.committer"),
            profile: pin("profile.member", 10),
            failure_domain: pin("failure.committer", 22),
        },
        executor: AdministrativeApprover {
            realm: Id("realm.alpha"),
            entity: Id("executor"),
            key: Id("key.executor"),
            profile: pin("profile.member", 10),
            failure_domain: pin("failure.executor", 23),
        },
        minimum_approvals: case.minimum_approvals,
        minimum_failure_domains: case.minimum_failure_domains,
        requester_independence: true,
        beneficiary_independence: true,
        successor_independence: true,
        delegation_ceiling: case.delegation_ceiling,
        ceremony: case.policy_ceremony,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();

    let beneficiaries = [case.beneficiary];
    let mut proposal = AdministrativeProposal {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("proposal.one"),
        effect_class,
        operation: pin("operation.exact", 32),
        requester: case.requester,
        subject: case.beneficiary,
        beneficiaries: &beneficiaries,
        predecessor_plan: case.predecessor_plan,
        delegation: case.requested_delegation,
        protected_handle: case.protected_handle,
        ceremony: case.proposal_ceremony,
        time_basis: Id("clock.monotonic"),
        created_at_tick: 10,
        expires_at_tick: case.proposal_expires_at_tick,
    };
    proposal.identity = proposal.computed_semantic_hash().unwrap();

    let mut approvals = [
        AdministrativeApproval {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            id: Id("approval.one"),
            proposal_identity: if case.replay_approval {
                hash(99)
            } else {
                proposal.identity
            },
            policy_identity: policy.identity,
            approver: case.approver_one,
            failure_domain: domain_one,
            time_basis: Id("clock.monotonic"),
            issued_at_tick: 12,
            expires_at_tick: 35,
            status: case.first_status,
        },
        AdministrativeApproval {
            schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
            identity: ZERO,
            id: Id("approval.two"),
            proposal_identity: proposal.identity,
            policy_identity: policy.identity,
            approver: second_principal,
            failure_domain: domain_two,
            time_basis: Id("clock.monotonic"),
            issued_at_tick: 13,
            expires_at_tick: 35,
            status: AdministrativeApprovalStatus::Current,
        },
    ];
    for approval in &mut approvals {
        approval.identity = approval.computed_semantic_hash().unwrap();
    }
    let approval_hashes = [approvals[0].identity, approvals[1].identity];
    let mut commit = AdministrativeCommit {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("commit.one"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: &approval_hashes[..case.approval_count],
        committed_by: case.committer,
        committed_at_tick: 16,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.one"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor: case.executor,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 16,
        expires_at_tick: 35,
    };
    execution.identity = execution.computed_semantic_hash().unwrap();
    validate_administrative_proof(
        AdministrativeProof {
            proposal,
            policy,
            approvals: &approvals[..case.approval_count],
            commit,
            execution,
        },
        ContainmentContext {
            subject: case.context_subject.unwrap_or(case.beneficiary),
            time_basis: Id("clock.monotonic"),
            now_tick: case.now_tick,
        },
    )
}

fn envelope() -> DelegationEnvelope<'static> {
    DelegationEnvelope {
        action: Id("artifact.install"),
        resource: ResourceSelector::Kind(Id("artifact")),
        audience: Id("runtime"),
        time_basis: Id("clock.monotonic"),
        not_before_tick: 10,
        expires_at_tick: 40,
        remaining_depth: 3,
    }
}

#[test]
fn ordinary_granted_effect_needs_no_administrative_proof() {
    let class = pin("effect.ordinary", 40);
    assert_eq!(
        validate_effect_containment(
            class,
            &[pin("effect.admin", 30)],
            None,
            ContainmentContext {
                subject: subject("target", 2),
                time_basis: Id("clock.monotonic"),
                now_tick: 20,
            }
        ),
        Ok(ContainmentDisposition::Ordinary)
    );
}

#[test]
fn exact_external_approval_permits_one_bounded_action() {
    assert_eq!(run(Case::default()), Ok(()));
}

#[test]
fn workload_cannot_approve_its_own_grant_enlargement() {
    let requester = principal("requester", "key.requester", 1);
    assert_eq!(
        run(Case {
            approver_one: requester,
            ..Case::default()
        }),
        Err(ContainmentReason::SelfSupporting)
    );
}

#[test]
fn active_plan_cannot_solely_authorize_its_successor() {
    assert_eq!(
        run(Case {
            predecessor_plan: Some(hash(1)),
            approver_one: principal("external.entity", "key.external", 1),
            ..Case::default()
        }),
        Err(ContainmentReason::SuccessorSelfAuthorized)
    );
}

#[test]
fn cyclic_mutual_plan_approval_is_rejected() {
    let edges = [
        AdministrativeSupportEdge {
            supporter: hash(1),
            beneficiary: hash(2),
        },
        AdministrativeSupportEdge {
            supporter: hash(2),
            beneficiary: hash(1),
        },
    ];
    assert_eq!(
        validate_support_graph(&edges, &mut [false; 2]),
        Err(ContainmentReason::CyclicSupport)
    );
}

#[test]
fn member_cannot_enroll_a_clone_for_its_own_benefit() {
    let requester = principal("member", "key.member", 8);
    assert_eq!(
        run(Case {
            requester,
            beneficiary: subject("member", 8),
            approver_one: requester,
            ..Case::default()
        }),
        Err(ContainmentReason::SelfSupporting)
    );
}

#[test]
fn installer_cannot_authorize_the_artifact_it_installs() {
    let installer = principal("installer", "key.installer", 9);
    assert_eq!(
        run(Case {
            requester: installer,
            beneficiary: subject("installer", 9),
            approver_one: installer,
            ..Case::default()
        }),
        Err(ContainmentReason::SelfSupporting)
    );
}

#[test]
fn threshold_requires_genuinely_separate_failure_domains() {
    let separate = Case {
        policy_approver_count: 2,
        approval_count: 2,
        minimum_approvals: 2,
        minimum_failure_domains: 2,
        ..Case::default()
    };
    assert_eq!(run(separate), Ok(()));
    assert_eq!(
        run(Case {
            shared_failure_domain: true,
            ..separate
        }),
        Err(ContainmentReason::FailureDomainInsufficient)
    );
}

#[test]
fn approval_replay_and_subject_replay_fail_exactly() {
    assert_eq!(
        run(Case {
            replay_approval: true,
            ..Case::default()
        }),
        Err(ContainmentReason::ApprovalReplay)
    );

    let base = Case::default();
    for replayed_subject in [
        AdministrativeSubject {
            realm: Id("realm.beta"),
            ..base.beneficiary
        },
        AdministrativeSubject {
            entity: Id("another.entity"),
            ..base.beneficiary
        },
        AdministrativeSubject {
            plan: hash(80),
            ..base.beneficiary
        },
        AdministrativeSubject {
            epoch: 8,
            ..base.beneficiary
        },
        AdministrativeSubject {
            artifact: Some(conduit_core::ArtifactDigest::from_bytes([81; 32])),
            ..base.beneficiary
        },
        AdministrativeSubject {
            budget: Some(pin("budget.larger", 82)),
            ..base.beneficiary
        },
    ] {
        assert_eq!(
            run(Case {
                context_subject: Some(replayed_subject),
                ..base
            }),
            Err(ContainmentReason::SubjectMismatch)
        );
    }
}

#[test]
fn expired_revoked_and_conflicting_approvals_fail_closed() {
    assert_eq!(
        run(Case {
            now_tick: 36,
            proposal_expires_at_tick: 50,
            ..Case::default()
        }),
        Err(ContainmentReason::ApprovalExpired)
    );
    assert_eq!(
        run(Case {
            first_status: AdministrativeApprovalStatus::Revoked,
            ..Case::default()
        }),
        Err(ContainmentReason::ApprovalRevoked)
    );
    assert_eq!(
        run(Case {
            policy_approver_count: 2,
            approval_count: 2,
            minimum_approvals: 2,
            duplicate_approver: true,
            ..Case::default()
        }),
        Err(ContainmentReason::ApprovalConflict)
    );
}

#[test]
fn delegation_is_monotonic_across_every_dimension() {
    let parent = envelope();
    let narrower = DelegationEnvelope {
        resource: ResourceSelector::Exact(ResourceRef {
            kind: Id("artifact"),
            id: Id("artifact.one"),
        }),
        not_before_tick: 12,
        expires_at_tick: 30,
        remaining_depth: 2,
        ..parent
    };
    assert_eq!(validate_delegation_narrowing(parent, narrower), Ok(()));
    for widened in [
        DelegationEnvelope {
            action: Id("artifact.replace"),
            ..narrower
        },
        DelegationEnvelope {
            resource: ResourceSelector::Kind(Id("resource")),
            ..narrower
        },
        DelegationEnvelope {
            audience: Id("other.runtime"),
            ..narrower
        },
        DelegationEnvelope {
            not_before_tick: 9,
            ..narrower
        },
        DelegationEnvelope {
            remaining_depth: 4,
            ..narrower
        },
    ] {
        assert_eq!(
            validate_delegation_narrowing(parent, widened),
            Err(ContainmentReason::DelegationWidened)
        );
    }
}

#[test]
fn governance_handle_requires_the_exact_pinned_ceremony() {
    assert_eq!(
        run(Case {
            protected_handle: Some(pin("handle.realm-root", 50)),
            ..Case::default()
        }),
        Err(ContainmentReason::CeremonyRequired)
    );
    assert_eq!(
        run(Case {
            protected_handle: Some(pin("handle.realm-root", 50)),
            proposal_ceremony: Some(pin("ceremony.rotate-one-key", 51)),
            policy_ceremony: Some(pin("ceremony.rotate-one-key", 51)),
            ..Case::default()
        }),
        Ok(())
    );
}

#[test]
fn recovery_path_cannot_widen_triggering_authority() {
    let trigger = envelope();
    assert_eq!(
        validate_recovery_narrowing(
            trigger,
            DelegationEnvelope {
                expires_at_tick: 30,
                remaining_depth: 1,
                ..trigger
            }
        ),
        Ok(())
    );
    assert_eq!(
        validate_recovery_narrowing(
            trigger,
            DelegationEnvelope {
                expires_at_tick: 50,
                ..trigger
            }
        ),
        Err(ContainmentReason::RecoveryWidened)
    );
}

#[test]
fn explanation_trees_are_caller_owned_and_bounded() {
    assert_eq!(
        validate_reason_tree(&[
            ContainmentReasonNode {
                reason: ContainmentReason::ApprovalMissing,
                parent: None,
                depth: 0,
            },
            ContainmentReasonNode {
                reason: ContainmentReason::ApproverNotAllowed,
                parent: Some(0),
                depth: 1,
            },
        ]),
        Ok(())
    );
    assert_eq!(
        validate_reason_tree(&[ContainmentReasonNode {
            reason: ContainmentReason::ApprovalMissing,
            parent: Some(0),
            depth: 1,
        }]),
        Err(ContainmentReason::ReasonTreeInvalid)
    );
}

#[test]
fn control_evidence_has_an_identity_distinct_from_every_authorization_stage() {
    let mut record = AdministrativeControlRecord {
        identity: ZERO,
        sequence: 4,
        record_id: Id("control.commit.one"),
        proposal_identity: hash(60),
        stage_identity: hash(61),
        realm: Id("realm.alpha"),
        entity: Id("committer"),
        epoch: 7,
        kind: AdministrativeControlKind::Committed,
    };
    record.identity = record.computed_semantic_hash().unwrap();
    assert_eq!(validate_control_record(record), Ok(()));
}

#[test]
fn every_containment_fixture_case_executes_independently() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/c2/containment-v1.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 33);
    for case in cases {
        let scenario = case["scenario"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let base = Case::default();
        let result = match scenario {
            "ordinary" => validate_effect_containment(
                pin("effect.ordinary", 40),
                &[pin("effect.admin", 30)],
                None,
                ContainmentContext {
                    subject: base.beneficiary,
                    time_basis: Id("clock.monotonic"),
                    now_tick: 20,
                },
            )
            .map(|_| ()),
            "self-grant" => run(Case {
                approver_one: base.requester,
                ..base
            }),
            "self-successor" => run(Case {
                predecessor_plan: Some(hash(1)),
                approver_one: principal("external.entity", "key.external", 1),
                ..base
            }),
            "cycle" => validate_support_graph(
                &[
                    AdministrativeSupportEdge {
                        supporter: hash(1),
                        beneficiary: hash(2),
                    },
                    AdministrativeSupportEdge {
                        supporter: hash(2),
                        beneficiary: hash(1),
                    },
                ],
                &mut [false; 2],
            ),
            "clone" | "installer" => {
                let actor = if scenario == "clone" {
                    principal("member", "key.member", 8)
                } else {
                    principal("installer", "key.installer", 9)
                };
                run(Case {
                    requester: actor,
                    beneficiary: subject(actor.entity.as_str(), actor.source_plan.as_bytes()[0]),
                    approver_one: actor,
                    ..base
                })
            }
            "external" => run(base),
            "commit-authority" => run(Case {
                committer: principal("other.committer", "key.other.committer", 5),
                ..base
            }),
            "execution-authority" => run(Case {
                executor: principal("other.executor", "key.other.executor", 6),
                ..base
            }),
            "threshold-separate" => run(Case {
                policy_approver_count: 2,
                approval_count: 2,
                minimum_approvals: 2,
                minimum_failure_domains: 2,
                ..base
            }),
            "threshold-shared" => run(Case {
                policy_approver_count: 2,
                approval_count: 2,
                minimum_approvals: 2,
                minimum_failure_domains: 2,
                shared_failure_domain: true,
                ..base
            }),
            "replay-realm" | "replay-entity" | "replay-plan" | "replay-epoch"
            | "replay-artifact" | "replay-budget" => {
                let context_subject = match scenario {
                    "replay-realm" => AdministrativeSubject {
                        realm: Id("realm.beta"),
                        ..base.beneficiary
                    },
                    "replay-entity" => AdministrativeSubject {
                        entity: Id("another.entity"),
                        ..base.beneficiary
                    },
                    "replay-plan" => AdministrativeSubject {
                        plan: hash(80),
                        ..base.beneficiary
                    },
                    "replay-epoch" => AdministrativeSubject {
                        epoch: 8,
                        ..base.beneficiary
                    },
                    "replay-artifact" => AdministrativeSubject {
                        artifact: Some(conduit_core::ArtifactDigest::from_bytes([81; 32])),
                        ..base.beneficiary
                    },
                    "replay-budget" => AdministrativeSubject {
                        budget: Some(pin("budget.larger", 82)),
                        ..base.beneficiary
                    },
                    _ => unreachable!(),
                };
                run(Case {
                    context_subject: Some(context_subject),
                    ..base
                })
            }
            "expired" => run(Case {
                now_tick: 36,
                proposal_expires_at_tick: 50,
                ..base
            }),
            "revoked" => run(Case {
                first_status: AdministrativeApprovalStatus::Revoked,
                ..base
            }),
            "conflict" => run(Case {
                policy_approver_count: 2,
                approval_count: 2,
                minimum_approvals: 2,
                duplicate_approver: true,
                ..base
            }),
            "unavailable" => run(Case {
                policy_approver_count: 2,
                approval_count: 1,
                minimum_approvals: 2,
                ..base
            }),
            "delegation-narrow" => run(Case {
                delegation_ceiling: Some(envelope()),
                requested_delegation: Some(DelegationEnvelope {
                    expires_at_tick: 30,
                    remaining_depth: 2,
                    ..envelope()
                }),
                ..base
            }),
            "delegation-action"
            | "delegation-resource"
            | "delegation-audience"
            | "delegation-time"
            | "delegation-depth" => {
                let requested = match scenario {
                    "delegation-action" => DelegationEnvelope {
                        action: Id("artifact.replace"),
                        ..envelope()
                    },
                    "delegation-resource" => DelegationEnvelope {
                        resource: ResourceSelector::Kind(Id("resource")),
                        ..envelope()
                    },
                    "delegation-audience" => DelegationEnvelope {
                        audience: Id("other.runtime"),
                        ..envelope()
                    },
                    "delegation-time" => DelegationEnvelope {
                        expires_at_tick: 41,
                        ..envelope()
                    },
                    "delegation-depth" => DelegationEnvelope {
                        remaining_depth: 4,
                        ..envelope()
                    },
                    _ => unreachable!(),
                };
                run(Case {
                    delegation_ceiling: Some(envelope()),
                    requested_delegation: Some(requested),
                    ..base
                })
            }
            "ceremony-missing" => run(Case {
                protected_handle: Some(pin("handle.realm-root", 50)),
                ..base
            }),
            "ceremony-exact" => run(Case {
                protected_handle: Some(pin("handle.realm-root", 50)),
                proposal_ceremony: Some(pin("ceremony.rotate-one-key", 51)),
                policy_ceremony: Some(pin("ceremony.rotate-one-key", 51)),
                ..base
            }),
            "recovery-narrow" => validate_recovery_narrowing(
                envelope(),
                DelegationEnvelope {
                    expires_at_tick: 30,
                    ..envelope()
                },
            ),
            "recovery-widen" => validate_recovery_narrowing(
                envelope(),
                DelegationEnvelope {
                    expires_at_tick: 50,
                    ..envelope()
                },
            ),
            _ => panic!("unregistered containment fixture scenario {scenario}"),
        };
        let actual = match result {
            Ok(()) => "accepted",
            Err(reason) => reason.code(),
        };
        assert_eq!(actual, expected, "fixture {}", case["id"]);
    }
}
