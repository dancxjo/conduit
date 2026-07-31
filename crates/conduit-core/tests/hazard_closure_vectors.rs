use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal, AdministrativeProof,
    AdministrativeProposal, AdministrativeSubject, ArtifactDigest, AuthorityConstraintRef,
    AuthorityGrant, AuthorityScope, AuthorityTime, CONTAINMENT_POLICY_SCHEMA_VERSION,
    ContainmentPolicy, DelegationPolicy, EffectClassBinding, EffectClassTraits, EffectFlowBinding,
    EffectRequirement, GrantStatus, HAZARD_CLOSURE_POLICY_SCHEMA_VERSION, HazardClosureContext,
    HazardClosureDisposition, HazardClosureLimits, HazardClosurePolicy, HazardClosureReason,
    HazardPermit, HostCapability, Id, InstancePath, MAX_HAZARD_PROOF_NODES, ObservedGrant,
    PinnedDescriptor, PlanAuthority, ResourceRef, ResourceSelector, SemanticHash, StopPolicy,
    ToxicCombinationRule, ToxicEffectPattern, ToxicFlowRequirement, TraitRequirement,
    TransitionEffectClosure, analyze_effect_closure, analyze_transition_effect_closure,
    effect_closure_subject, effect_combination_scope, resolve_authority,
    transition_effect_closure_subject,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn class(id: &'static str, byte: u8, traits: EffectClassTraits) -> EffectClassBinding<'static> {
    let descriptor = pin(id, byte);
    let mut class = EffectClassBinding {
        identity: ZERO,
        descriptor,
        constraint: AuthorityConstraintRef {
            id: descriptor.id,
            semantic_hash: descriptor.semantic_hash,
        },
        traits,
    };
    class.identity = class.computed_semantic_hash().unwrap();
    class
}

fn authority(
    effect_id: &'static str,
    node: &'static str,
    host: &'static str,
    class: EffectClassBinding<'static>,
    audience: &'static str,
    resource_id: &'static str,
    delegation: DelegationPolicy,
) -> PlanAuthority<'static> {
    let constraints: &'static [AuthorityConstraintRef<'static>] =
        Box::leak(vec![class.constraint].into_boxed_slice());
    let effect = EffectRequirement {
        id: Id(effect_id),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("action.use"),
        resource: ResourceSelector::Exact(ResourceRef {
            kind: Id("resource.fixture"),
            id: Id(resource_id),
        }),
        requester: InstancePath::new(node).unwrap(),
        audience: Id(audience),
        constraints,
        check_at_use: true,
    };
    let capability = HostCapability {
        id: Id(effect_id),
        action: effect.action,
        resource: match effect.resource {
            ResourceSelector::Exact(resource) => resource,
            ResourceSelector::Kind(_) => unreachable!(),
        },
        host: Id(host),
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 1,
        valid_until_tick: 100,
    };
    let grant = AuthorityGrant {
        id: Id(effect_id),
        action: effect.action,
        resource: capability.resource,
        scope: AuthorityScope {
            root: effect.requester,
            descendants: false,
        },
        audience: effect.audience,
        constraints,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 1,
        expires_at_tick: 100,
        issued_for_host: Id(host),
        delegation,
        audit_id: Id("audit.fixture"),
        terminal_policy: StopPolicy::Abort,
    };
    let binding = resolve_authority(
        effect,
        Id(host),
        AuthorityTime {
            basis: Id("clock.monotonic"),
            tick: 10,
        },
        &[capability],
        &[ObservedGrant {
            grant,
            status: GrantStatus::Active,
        }],
    )
    .unwrap();
    PlanAuthority {
        node: effect.requester,
        effect_hash: effect.semantic_hash().unwrap(),
        grant_hash: grant.semantic_hash().unwrap(),
        effect,
        capability,
        grant,
        binding,
        administrative_subject: None,
        containment: None,
        policy_budgets: &[],
        commit_profile: None,
    }
}

fn pattern(id: &'static str, class: EffectClassBinding<'static>) -> ToxicEffectPattern<'static> {
    ToxicEffectPattern {
        id: Id(id),
        class: class.descriptor,
        resource: None,
        audience: None,
        host: None,
        realm: None,
        budget: None,
        persistence: TraitRequirement::Any,
        delegation: TraitRequirement::Any,
        distributed: TraitRequirement::Any,
        administrative: TraitRequirement::Any,
    }
}

fn policy(
    classes: Vec<EffectClassBinding<'static>>,
    patterns: Vec<ToxicEffectPattern<'static>>,
    flows: Vec<ToxicFlowRequirement<'static>>,
    proof_nodes: u8,
    search_steps: u32,
) -> (HazardClosurePolicy<'static>, ToxicCombinationRule<'static>) {
    let patterns = Box::leak(patterns.into_boxed_slice());
    let flows = Box::leak(flows.into_boxed_slice());
    let mut rule = ToxicCombinationRule {
        identity: ZERO,
        descriptor: pin("rule.toxic", 80),
        patterns,
        flows,
    };
    rule.identity = rule.computed_semantic_hash().unwrap();
    let classes = Box::leak(classes.into_boxed_slice());
    let rules = Box::leak(vec![rule].into_boxed_slice());
    let mut policy = HazardClosurePolicy {
        schema_version: HAZARD_CLOSURE_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.hazard", 81),
        permit_class: pin("effect.hazard-permit", 82),
        classes,
        rules,
        limits: HazardClosureLimits {
            maximum_effects: 64,
            maximum_classes: 32,
            maximum_rules: 16,
            maximum_patterns_per_rule: 8,
            maximum_flows: 32,
            maximum_permits: 16,
            maximum_proof_nodes: proof_nodes,
            maximum_search_steps: search_steps,
        },
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    (policy, rule)
}

fn context(
    authorities: &[PlanAuthority<'_>],
    flows: &[EffectFlowBinding<'_>],
    epoch: u64,
    tick: u64,
) -> HazardClosureContext<'static> {
    HazardClosureContext {
        plan_subject: effect_closure_subject(authorities, flows, epoch, Id("clock.monotonic"))
            .unwrap(),
        epoch,
        time: AuthorityTime {
            basis: Id("clock.monotonic"),
            tick,
        },
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
        profile: pin("profile.member", 90),
        source_plan: hash(plan),
        source_epoch: 1,
    }
}

fn permit_proof(
    permit_class: PinnedDescriptor<'static>,
    operation: PinnedDescriptor<'static>,
    plan_subject: SemanticHash,
    epoch: u64,
    scope: SemanticHash,
) -> AdministrativeProof<'static> {
    let subject = AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("entity.target"),
        plan: plan_subject,
        epoch,
        artifact: None,
        budget: Some(PinnedDescriptor {
            id: operation.id,
            schema_version: operation.schema_version,
            semantic_hash: scope,
        }),
    };
    let requester = principal("requester", "key.requester", 1);
    let approver_principal = principal("approver", "key.approver", 2);
    let committer_principal = principal("committer", "key.committer", 3);
    let executor_principal = principal("executor", "key.executor", 4);
    let failure = pin("failure.independent", 91);
    let approvers: &'static [AdministrativeApprover<'static>] = Box::leak(
        vec![AdministrativeApprover {
            realm: approver_principal.realm,
            entity: approver_principal.entity,
            key: approver_principal.key,
            profile: approver_principal.profile,
            failure_domain: failure,
        }]
        .into_boxed_slice(),
    );
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.permit-approval", 92),
        effect_class: permit_class,
        approvers,
        committer: AdministrativeApprover {
            realm: committer_principal.realm,
            entity: committer_principal.entity,
            key: committer_principal.key,
            profile: committer_principal.profile,
            failure_domain: pin("failure.committer", 93),
        },
        executor: AdministrativeApprover {
            realm: executor_principal.realm,
            entity: executor_principal.entity,
            key: executor_principal.key,
            profile: executor_principal.profile,
            failure_domain: pin("failure.executor", 94),
        },
        minimum_approvals: 1,
        minimum_failure_domains: 1,
        requester_independence: true,
        beneficiary_independence: true,
        successor_independence: true,
        delegation_ceiling: None,
        ceremony: None,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    let beneficiaries: &'static [AdministrativeSubject<'static>] =
        Box::leak(vec![subject].into_boxed_slice());
    let mut proposal = AdministrativeProposal {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("proposal.permit"),
        effect_class: permit_class,
        operation,
        requester,
        subject,
        beneficiaries,
        predecessor_plan: None,
        delegation: None,
        protected_handle: None,
        ceremony: None,
        time_basis: Id("clock.monotonic"),
        created_at_tick: 5,
        expires_at_tick: 40,
    };
    proposal.identity = proposal.computed_semantic_hash().unwrap();
    let mut approval = AdministrativeApproval {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("approval.permit"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approver: approver_principal,
        failure_domain: failure,
        time_basis: Id("clock.monotonic"),
        issued_at_tick: 6,
        expires_at_tick: 35,
        status: AdministrativeApprovalStatus::Current,
    };
    approval.identity = approval.computed_semantic_hash().unwrap();
    let approvals: &'static [AdministrativeApproval<'static>] =
        Box::leak(vec![approval].into_boxed_slice());
    let approval_hashes: &'static [SemanticHash] =
        Box::leak(vec![approval.identity].into_boxed_slice());
    let mut commit = AdministrativeCommit {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("commit.permit"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: approval_hashes,
        committed_by: committer_principal,
        committed_at_tick: 8,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.permit"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor: executor_principal,
        time_basis: Id("clock.monotonic"),
        not_before_tick: 8,
        expires_at_tick: 35,
    };
    execution.identity = execution.computed_semantic_hash().unwrap();
    AdministrativeProof {
        proposal,
        policy,
        approvals,
        commit,
        execution,
    }
}

fn exact_permit(
    policy: HazardClosurePolicy<'static>,
    rule: ToxicCombinationRule<'static>,
    authorities: &[PlanAuthority<'static>],
    effects: &[Id<'static>],
    closure_context: HazardClosureContext<'static>,
    operation: PinnedDescriptor<'static>,
) -> HazardPermit<'static> {
    let scope = effect_combination_scope(rule, authorities, effects).unwrap();
    let approval = permit_proof(
        policy.permit_class,
        operation,
        closure_context.plan_subject,
        closure_context.epoch,
        scope,
    );
    let mut permit = HazardPermit {
        identity: ZERO,
        descriptor: operation,
        policy_identity: policy.identity,
        rule_identity: rule.identity,
        plan_subject: closure_context.plan_subject,
        epoch: closure_context.epoch,
        scope_identity: scope,
        time_basis: closure_context.time.basis,
        not_before_tick: 8,
        expires_at_tick: 30,
        approval,
    };
    permit.identity = permit.computed_semantic_hash().unwrap();
    permit
}

#[test]
fn isolated_effects_are_safe_but_the_exact_toxic_combination_denies() {
    let network = class("class.network", 1, EffectClassTraits::default());
    let write = class(
        "class.executable-write",
        2,
        EffectClassTraits {
            persistence: true,
            ..EffectClassTraits::default()
        },
    );
    let process = class("class.process", 3, EffectClassTraits::default());
    let (policy, _) = policy(
        vec![network, write, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.write", write),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let all = [
        authority(
            "effect.network",
            "root/network",
            "host.one",
            network,
            "audience.user",
            "network",
            DelegationPolicy::None,
        ),
        authority(
            "effect.write",
            "root/write",
            "host.one",
            write,
            "audience.user",
            "artifact",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.one",
            process,
            "audience.user",
            "process",
            DelegationPolicy::None,
        ),
    ];
    for effect in &all {
        let single = [*effect];
        let mut proof = [None; MAX_HAZARD_PROOF_NODES];
        let report = analyze_effect_closure(
            policy,
            &single,
            &[],
            &[],
            context(&single, &[], 1, 10),
            &mut proof,
        )
        .unwrap();
        assert_eq!(report.disposition, HazardClosureDisposition::Accepted);
    }
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    let denial = analyze_effect_closure(
        policy,
        &all,
        &[],
        &[],
        context(&all, &[], 1, 10),
        &mut proof,
    )
    .unwrap_err();
    assert_eq!(denial.reason, HazardClosureReason::PermitMissing);
    assert_eq!(denial.rule, Some(Id("rule.toxic")));
    assert_eq!(denial.effect_count, 3);
    assert_eq!(
        proof[0].unwrap().descriptor,
        Id("rule.toxic"),
        "the proof tree names the exact rule"
    );
}

#[test]
fn exact_independently_approved_permit_authorizes_only_its_scope() {
    let network = class("class.network", 1, EffectClassTraits::default());
    let process = class("class.process", 2, EffectClassTraits::default());
    let (policy, rule) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let authorities = [
        authority(
            "effect.network",
            "root/network",
            "host.one",
            network,
            "audience.user",
            "network",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.one",
            process,
            "audience.user",
            "process",
            DelegationPolicy::None,
        ),
    ];
    let context = context(&authorities, &[], 7, 10);
    let scope = effect_combination_scope(
        rule,
        &authorities,
        &[Id("effect.network"), Id("effect.process")],
    )
    .unwrap();
    let operation = pin("permit.exact-combination", 83);
    let approval = permit_proof(
        policy.permit_class,
        operation,
        context.plan_subject,
        context.epoch,
        scope,
    );
    let mut permit = HazardPermit {
        identity: ZERO,
        descriptor: operation,
        policy_identity: policy.identity,
        rule_identity: rule.identity,
        plan_subject: context.plan_subject,
        epoch: context.epoch,
        scope_identity: scope,
        time_basis: context.time.basis,
        not_before_tick: 8,
        expires_at_tick: 30,
        approval,
    };
    permit.identity = permit.computed_semantic_hash().unwrap();
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    let permits = [permit];
    let report =
        analyze_effect_closure(policy, &authorities, &[], &permits, context, &mut proof).unwrap();
    assert_eq!(report.disposition, HazardClosureDisposition::Permitted);
    assert_eq!(report.permits_used, 1);

    let mut wrong_epoch = permit;
    wrong_epoch.epoch = 8;
    wrong_epoch.identity = wrong_epoch.computed_semantic_hash().unwrap();
    let wrong_permits = [wrong_epoch];
    assert_eq!(
        analyze_effect_closure(
            policy,
            &authorities,
            &[],
            &wrong_permits,
            context,
            &mut proof
        )
        .unwrap_err()
        .reason,
        HazardClosureReason::PermitMissing
    );
}

fn multiple_occurrence_case(
    include_second_permit: bool,
) -> Result<HazardClosureDisposition, HazardClosureReason> {
    let network = class("class.network", 1, EffectClassTraits::default());
    let process = class("class.process", 2, EffectClassTraits::default());
    let (policy, rule) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let authorities = [
        authority(
            "effect.network-one",
            "root/network-one",
            "host.one",
            network,
            "audience.user",
            "network-one",
            DelegationPolicy::None,
        ),
        authority(
            "effect.network-two",
            "root/network-two",
            "host.one",
            network,
            "audience.user",
            "network-two",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.one",
            process,
            "audience.user",
            "process",
            DelegationPolicy::None,
        ),
    ];
    let closure_context = context(&authorities, &[], 7, 10);
    let first = exact_permit(
        policy,
        rule,
        &authorities,
        &[Id("effect.network-one"), Id("effect.process")],
        closure_context,
        pin("permit.first-combination", 83),
    );
    let second = exact_permit(
        policy,
        rule,
        &authorities,
        &[Id("effect.network-two"), Id("effect.process")],
        closure_context,
        pin("permit.second-combination", 84),
    );
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    let first_only = [first];
    let both = [first, second];
    analyze_effect_closure(
        policy,
        &authorities,
        &[],
        if include_second_permit {
            &both
        } else {
            &first_only
        },
        closure_context,
        &mut proof,
    )
    .map(|report| report.disposition)
    .map_err(|denial| denial.reason)
}

#[test]
fn every_distinct_toxic_occurrence_requires_its_own_exact_permit() {
    assert_eq!(
        multiple_occurrence_case(false),
        Err(HazardClosureReason::PermitMissing)
    );
    assert_eq!(
        multiple_occurrence_case(true),
        Ok(HazardClosureDisposition::Permitted)
    );
}

#[test]
fn declared_multistage_propagation_and_remote_delegation_remain_visible() {
    let discovery = class("class.discovery", 1, EffectClassTraits::default());
    let enrollment = class(
        "class.enrollment",
        2,
        EffectClassTraits {
            administrative: true,
            ..EffectClassTraits::default()
        },
    );
    let install = class(
        "class.install",
        3,
        EffectClassTraits {
            persistence: true,
            distributed: true,
            ..EffectClassTraits::default()
        },
    );
    let execute = class("class.execute", 4, EffectClassTraits::default());
    let redelegate = class(
        "class.redelegate",
        5,
        EffectClassTraits {
            delegation: true,
            distributed: true,
            ..EffectClassTraits::default()
        },
    );
    let transfer = pin("transfer.exact-stage-output", 70);
    let patterns = vec![
        pattern("stage.discovery", discovery),
        pattern("stage.enrollment", enrollment),
        pattern("stage.install", install),
        pattern("stage.execute", execute),
        pattern("stage.redelegate", redelegate),
    ];
    let flows = vec![
        ToxicFlowRequirement {
            from_pattern: 0,
            to_pattern: 1,
            transfer,
        },
        ToxicFlowRequirement {
            from_pattern: 1,
            to_pattern: 2,
            transfer,
        },
        ToxicFlowRequirement {
            from_pattern: 2,
            to_pattern: 3,
            transfer,
        },
        ToxicFlowRequirement {
            from_pattern: 3,
            to_pattern: 4,
            transfer,
        },
    ];
    let (policy, _) = policy(
        vec![discovery, enrollment, install, execute, redelegate],
        patterns,
        flows,
        32,
        10_000,
    );
    let authorities = [
        authority(
            "effect.discovery",
            "root/composite/discovery",
            "host.one",
            discovery,
            "audience.realm",
            "discovery",
            DelegationPolicy::None,
        ),
        authority(
            "effect.enrollment",
            "root/composite/enrollment",
            "host.one",
            enrollment,
            "audience.realm",
            "member",
            DelegationPolicy::None,
        ),
        authority(
            "effect.install",
            "root/remote/install",
            "host.two",
            install,
            "audience.realm",
            "artifact",
            DelegationPolicy::CrossHostDescendants,
        ),
        authority(
            "effect.execute",
            "root/remote/execute",
            "host.two",
            execute,
            "audience.realm",
            "process",
            DelegationPolicy::None,
        ),
        authority(
            "effect.redelegate",
            "root/remote/redelegate",
            "host.three",
            redelegate,
            "audience.federated",
            "grant",
            DelegationPolicy::CrossHostDescendants,
        ),
    ];
    let exact_flows = [
        EffectFlowBinding {
            from_effect: Id("effect.discovery"),
            to_effect: Id("effect.enrollment"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.enrollment"),
            to_effect: Id("effect.install"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.install"),
            to_effect: Id("effect.execute"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.execute"),
            to_effect: Id("effect.redelegate"),
            transfer,
        },
    ];
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    let denial = analyze_effect_closure(
        policy,
        &authorities,
        &exact_flows,
        &[],
        context(&authorities, &exact_flows, 1, 10),
        &mut proof,
    )
    .unwrap_err();
    assert_eq!(denial.effect_count, 5);
    assert_eq!(denial.reason, HazardClosureReason::PermitMissing);

    let broken_flows = &exact_flows[..3];
    let report = analyze_effect_closure(
        policy,
        &authorities,
        broken_flows,
        &[],
        context(&authorities, broken_flows, 1, 10),
        &mut proof,
    )
    .unwrap();
    assert_eq!(report.disposition, HazardClosureDisposition::Accepted);
}

#[test]
fn separately_safe_plans_fail_during_old_new_overlap() {
    let network = class("class.network", 1, EffectClassTraits::default());
    let process = class("class.process", 2, EffectClassTraits::default());
    let (policy, _) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let old = [authority(
        "effect.old-network",
        "root/old/network",
        "host.one",
        network,
        "audience.user",
        "network",
        DelegationPolicy::None,
    )];
    let new_and_rollback = [authority(
        "effect.new-process",
        "root/new/process",
        "host.one",
        process,
        "audience.user",
        "process",
        DelegationPolicy::None,
    )];
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    assert!(
        analyze_effect_closure(
            policy,
            &old,
            &[],
            &[],
            context(&old, &[], 1, 10),
            &mut proof
        )
        .is_ok()
    );
    assert!(
        analyze_effect_closure(
            policy,
            &new_and_rollback,
            &[],
            &[],
            context(&new_and_rollback, &[], 2, 10),
            &mut proof
        )
        .is_ok()
    );
    let subject = transition_effect_closure_subject(
        &old,
        &new_and_rollback,
        &[],
        &[],
        2,
        Id("clock.monotonic"),
    )
    .unwrap();
    let denial = analyze_transition_effect_closure(
        policy,
        TransitionEffectClosure {
            old_authorities: &old,
            new_and_rollback_authorities: &new_and_rollback,
            old_flows: &[],
            new_and_rollback_flows: &[],
        },
        &[],
        HazardClosureContext {
            plan_subject: subject,
            epoch: 2,
            time: AuthorityTime {
                basis: Id("clock.monotonic"),
                tick: 10,
            },
        },
        &mut proof,
    )
    .unwrap_err();
    assert_eq!(denial.reason, HazardClosureReason::PermitMissing);
}

#[test]
fn exact_constraints_and_analysis_bounds_fail_closed() {
    let network = class("class.network", 1, EffectClassTraits::default());
    let process = class("class.process", 2, EffectClassTraits::default());
    let mut network_pattern = pattern("stage.network", network);
    network_pattern.host = Some(Id("host.allowed"));
    network_pattern.audience = Some(Id("audience.allowed"));
    let (scoped_policy, _) = policy(
        vec![network, process],
        vec![network_pattern, pattern("stage.process", process)],
        vec![],
        16,
        1_000,
    );
    let authorities = [
        authority(
            "effect.network",
            "root/network",
            "host.other",
            network,
            "audience.other",
            "network",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.other",
            process,
            "audience.other",
            "process",
            DelegationPolicy::None,
        ),
    ];
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    assert!(
        analyze_effect_closure(
            scoped_policy,
            &authorities,
            &[],
            &[],
            context(&authorities, &[], 1, 10),
            &mut proof
        )
        .is_ok(),
        "same class names with different exact constraints must differ"
    );

    let (tiny_proof_policy, _) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let mut too_small = [None; 15];
    assert_eq!(
        analyze_effect_closure(
            tiny_proof_policy,
            &authorities,
            &[],
            &[],
            context(&authorities, &[], 1, 10),
            &mut too_small
        )
        .unwrap_err()
        .reason,
        HazardClosureReason::ProofStorageExceeded
    );

    let (tiny_search_policy, _) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1,
    );
    assert_eq!(
        analyze_effect_closure(
            tiny_search_policy,
            &authorities,
            &[],
            &[],
            context(&authorities, &[], 1, 10),
            &mut proof
        )
        .unwrap_err()
        .reason,
        HazardClosureReason::SearchLimitExceeded
    );
}

#[test]
fn indexed_rule_meaning_and_proof_order_are_canonical() {
    let network = class("class.network", 1, EffectClassTraits::default());
    let write = class("class.write", 2, EffectClassTraits::default());
    let process = class("class.process", 3, EffectClassTraits::default());
    let transfer = pin("transfer.exact", 70);
    let patterns_a: &'static [ToxicEffectPattern<'static>] = Box::leak(
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ]
        .into_boxed_slice(),
    );
    let patterns_a_swapped: &'static [ToxicEffectPattern<'static>] = Box::leak(
        vec![
            pattern("stage.process", process),
            pattern("stage.network", network),
        ]
        .into_boxed_slice(),
    );
    let flows_a: &'static [ToxicFlowRequirement<'static>] = Box::leak(
        vec![ToxicFlowRequirement {
            from_pattern: 0,
            to_pattern: 1,
            transfer,
        }]
        .into_boxed_slice(),
    );
    let mut rule_a = ToxicCombinationRule {
        identity: ZERO,
        descriptor: pin("rule.alpha", 80),
        patterns: patterns_a,
        flows: flows_a,
    };
    rule_a.identity = rule_a.computed_semantic_hash().unwrap();
    let mut reordered_meaning = ToxicCombinationRule {
        patterns: patterns_a_swapped,
        ..rule_a
    };
    reordered_meaning.identity = reordered_meaning.computed_semantic_hash().unwrap();
    assert_ne!(
        rule_a.identity, reordered_meaning.identity,
        "flow indexes make pattern order semantic"
    );

    let patterns_b: &'static [ToxicEffectPattern<'static>] = Box::leak(
        vec![
            pattern("stage.write", write),
            pattern("stage.process", process),
        ]
        .into_boxed_slice(),
    );
    let mut rule_b = ToxicCombinationRule {
        identity: ZERO,
        descriptor: pin("rule.beta", 81),
        patterns: patterns_b,
        flows: &[],
    };
    rule_b.identity = rule_b.computed_semantic_hash().unwrap();
    let classes: &'static [EffectClassBinding<'static>] =
        Box::leak(vec![network, write, process].into_boxed_slice());
    let rules_ab: &'static [ToxicCombinationRule<'static>] =
        Box::leak(vec![rule_a, rule_b].into_boxed_slice());
    let rules_ba: &'static [ToxicCombinationRule<'static>] =
        Box::leak(vec![rule_b, rule_a].into_boxed_slice());
    let mut policy_ab = HazardClosurePolicy {
        schema_version: HAZARD_CLOSURE_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.canonical", 82),
        permit_class: pin("effect.hazard-permit", 83),
        classes,
        rules: rules_ab,
        limits: HazardClosureLimits {
            maximum_effects: 64,
            maximum_classes: 32,
            maximum_rules: 16,
            maximum_patterns_per_rule: 8,
            maximum_flows: 32,
            maximum_permits: 16,
            maximum_proof_nodes: 64,
            maximum_search_steps: 1_000,
        },
    };
    policy_ab.identity = policy_ab.computed_semantic_hash().unwrap();
    let mut policy_ba = HazardClosurePolicy {
        rules: rules_ba,
        ..policy_ab
    };
    policy_ba.identity = policy_ba.computed_semantic_hash().unwrap();
    assert_eq!(policy_ab.identity, policy_ba.identity);

    let authorities = [
        authority(
            "effect.network",
            "root/network",
            "host.one",
            network,
            "audience.user",
            "network",
            DelegationPolicy::None,
        ),
        authority(
            "effect.write",
            "root/write",
            "host.one",
            write,
            "audience.user",
            "write",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.one",
            process,
            "audience.user",
            "process",
            DelegationPolicy::None,
        ),
    ];
    let flows = [EffectFlowBinding {
        from_effect: Id("effect.network"),
        to_effect: Id("effect.process"),
        transfer,
    }];
    let closure_context = context(&authorities, &flows, 1, 10);
    let mut proof_ab = [None; MAX_HAZARD_PROOF_NODES];
    let mut proof_ba = [None; MAX_HAZARD_PROOF_NODES];
    let denial_ab = analyze_effect_closure(
        policy_ab,
        &authorities,
        &flows,
        &[],
        closure_context,
        &mut proof_ab,
    )
    .unwrap_err();
    let denial_ba = analyze_effect_closure(
        policy_ba,
        &authorities,
        &flows,
        &[],
        closure_context,
        &mut proof_ba,
    )
    .unwrap_err();
    assert_eq!(denial_ab, denial_ba);
    assert_eq!(proof_ab, proof_ba);
}

fn run_simple_fixture(scenario: &str) -> Result<&'static str, HazardClosureReason> {
    let network = class("class.network", 1, EffectClassTraits::default());
    let write = class(
        "class.executable-write",
        2,
        EffectClassTraits {
            persistence: true,
            ..EffectClassTraits::default()
        },
    );
    let process = class("class.process", 3, EffectClassTraits::default());
    let mut network_pattern = pattern("stage.network", network);
    if scenario == "constraint-distinction" {
        network_pattern.host = Some(Id("host.allowed"));
        network_pattern.audience = Some(Id("audience.allowed"));
    }
    let (policy, rule) = policy(
        vec![network, write, process],
        vec![
            network_pattern,
            pattern("stage.write", write),
            pattern("stage.process", process),
        ],
        vec![],
        if scenario == "proof-exhaustion" {
            16
        } else {
            32
        },
        if scenario == "search-exhaustion" {
            1
        } else {
            1_000
        },
    );
    let composite = scenario == "composite-hidden";
    let host = if scenario == "constraint-distinction" {
        "host.other"
    } else if scenario == "permit-host-mismatch" {
        "host.two"
    } else {
        "host.one"
    };
    let audience = if scenario == "constraint-distinction" {
        "audience.other"
    } else {
        "audience.user"
    };
    let baseline = [
        authority(
            "effect.network",
            "root/network",
            "host.one",
            network,
            "audience.user",
            "network",
            DelegationPolicy::None,
        ),
        authority(
            "effect.write",
            "root/write",
            "host.one",
            write,
            "audience.user",
            "artifact",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            "root/process",
            "host.one",
            process,
            "audience.user",
            "process",
            DelegationPolicy::None,
        ),
    ];
    let mut all = [
        authority(
            "effect.network",
            if composite {
                "root/composite/hidden/network"
            } else {
                "root/network"
            },
            host,
            network,
            audience,
            if scenario == "permit-plan-mismatch" {
                "network.other-plan"
            } else {
                "network"
            },
            DelegationPolicy::None,
        ),
        authority(
            "effect.write",
            if composite {
                "root/composite/hidden/write"
            } else {
                "root/write"
            },
            host,
            write,
            audience,
            "artifact",
            DelegationPolicy::None,
        ),
        authority(
            "effect.process",
            if composite {
                "root/composite/hidden/process"
            } else {
                "root/process"
            },
            host,
            process,
            audience,
            "process",
            DelegationPolicy::None,
        ),
    ];
    if matches!(
        scenario,
        "permit-artifact-mismatch" | "permit-realm-mismatch"
    ) {
        all[0].administrative_subject = Some(AdministrativeSubject {
            realm: if scenario == "permit-realm-mismatch" {
                Id("realm.other")
            } else {
                Id("realm.alpha")
            },
            entity: Id("entity.target"),
            plan: hash(140),
            epoch: 7,
            artifact: if scenario == "permit-artifact-mismatch" {
                Some(ArtifactDigest::from_bytes([141; 32]))
            } else {
                None
            },
            budget: None,
        });
    }
    if scenario == "permit-budget-mismatch" {
        all[0].effect.policy_budget_class = Some(pin("fixture/budget-other", 142));
        all[0].effect_hash = all[0]
            .effect
            .semantic_hash()
            .unwrap_or_else(|error| panic!("{scenario}: {error:?}"));
    }
    let selected = match scenario {
        "isolated-network" => &all[..1],
        "isolated-write" => &all[1..2],
        "isolated-process" => &all[2..],
        _ => &all[..],
    };
    let closure_context = context(
        selected,
        &[],
        if scenario == "permit-epoch-mismatch" {
            8
        } else {
            7
        },
        10,
    );
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    let mut short_proof = [None; 15];
    let storage = if scenario == "proof-exhaustion" {
        &mut short_proof[..]
    } else {
        &mut proof[..]
    };

    let mut permit_storage = Vec::new();
    if scenario == "exact-permit" || scenario.starts_with("permit-") {
        let baseline_context = context(&baseline, &[], 7, 10);
        let scope = effect_combination_scope(
            rule,
            &baseline,
            &[
                Id("effect.network"),
                Id("effect.write"),
                Id("effect.process"),
            ],
        )
        .unwrap();
        let operation = pin("permit.exact-combination", 83);
        let mut approval = permit_proof(
            policy.permit_class,
            operation,
            baseline_context.plan_subject,
            baseline_context.epoch,
            scope,
        );
        if scenario == "permit-approval-invalid" {
            approval.execution.expires_at_tick = 9;
        }
        let mut permit = HazardPermit {
            identity: ZERO,
            descriptor: operation,
            policy_identity: policy.identity,
            rule_identity: rule.identity,
            plan_subject: baseline_context.plan_subject,
            epoch: baseline_context.epoch,
            scope_identity: scope,
            time_basis: baseline_context.time.basis,
            not_before_tick: 8,
            expires_at_tick: if scenario == "permit-expired" { 10 } else { 30 },
            approval,
        };
        permit.identity = permit.computed_semantic_hash().unwrap();
        permit_storage.push(permit);
    }

    match analyze_effect_closure(
        policy,
        selected,
        &[],
        &permit_storage,
        closure_context,
        storage,
    ) {
        Ok(report) => Ok(match report.disposition {
            HazardClosureDisposition::Accepted => "accepted",
            HazardClosureDisposition::Permitted => "permitted",
        }),
        Err(error) => Err(error.reason),
    }
}

fn run_propagation_fixture(scenario: &str) -> Result<&'static str, HazardClosureReason> {
    let discovery = class("class.discovery", 1, EffectClassTraits::default());
    let enrollment = class(
        "class.enrollment",
        2,
        EffectClassTraits {
            administrative: true,
            ..EffectClassTraits::default()
        },
    );
    let install = class(
        "class.install",
        3,
        EffectClassTraits {
            persistence: true,
            distributed: true,
            ..EffectClassTraits::default()
        },
    );
    let execute = class("class.execute", 4, EffectClassTraits::default());
    let redelegate = class(
        "class.redelegate",
        5,
        EffectClassTraits {
            delegation: true,
            distributed: true,
            ..EffectClassTraits::default()
        },
    );
    let transfer = pin("transfer.exact-stage-output", 70);
    let (policy, _) = policy(
        vec![discovery, enrollment, install, execute, redelegate],
        vec![
            pattern("stage.discovery", discovery),
            pattern("stage.enrollment", enrollment),
            pattern("stage.install", install),
            pattern("stage.execute", execute),
            pattern("stage.redelegate", redelegate),
        ],
        vec![
            ToxicFlowRequirement {
                from_pattern: 0,
                to_pattern: 1,
                transfer,
            },
            ToxicFlowRequirement {
                from_pattern: 1,
                to_pattern: 2,
                transfer,
            },
            ToxicFlowRequirement {
                from_pattern: 2,
                to_pattern: 3,
                transfer,
            },
            ToxicFlowRequirement {
                from_pattern: 3,
                to_pattern: 4,
                transfer,
            },
        ],
        32,
        10_000,
    );
    let remote = scenario == "remote-federation";
    let authorities = [
        authority(
            "effect.discovery",
            "root/composite/discovery",
            "host.one",
            discovery,
            "audience.realm",
            "discovery",
            DelegationPolicy::None,
        ),
        authority(
            "effect.enrollment",
            "root/composite/enrollment",
            "host.one",
            enrollment,
            "audience.realm",
            "member",
            DelegationPolicy::None,
        ),
        authority(
            "effect.install",
            "root/composite/install",
            if remote { "host.two" } else { "host.one" },
            install,
            "audience.realm",
            "artifact",
            if remote {
                DelegationPolicy::CrossHostDescendants
            } else {
                DelegationPolicy::None
            },
        ),
        authority(
            "effect.execute",
            "root/composite/execute",
            if remote { "host.two" } else { "host.one" },
            execute,
            "audience.realm",
            "process",
            DelegationPolicy::None,
        ),
        authority(
            "effect.redelegate",
            "root/composite/redelegate",
            if remote { "host.three" } else { "host.one" },
            redelegate,
            if remote {
                "audience.federated"
            } else {
                "audience.realm"
            },
            "grant",
            if remote {
                DelegationPolicy::CrossHostDescendants
            } else {
                DelegationPolicy::None
            },
        ),
    ];
    let flows = [
        EffectFlowBinding {
            from_effect: Id("effect.discovery"),
            to_effect: Id("effect.enrollment"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.enrollment"),
            to_effect: Id("effect.install"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.install"),
            to_effect: Id("effect.execute"),
            transfer,
        },
        EffectFlowBinding {
            from_effect: Id("effect.execute"),
            to_effect: Id("effect.redelegate"),
            transfer,
        },
    ];
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    match analyze_effect_closure(
        policy,
        &authorities,
        &flows,
        &[],
        context(&authorities, &flows, 1, 10),
        &mut proof,
    ) {
        Ok(report) => Ok(match report.disposition {
            HazardClosureDisposition::Accepted => "accepted",
            HazardClosureDisposition::Permitted => "permitted",
        }),
        Err(error) => Err(error.reason),
    }
}

fn run_transition_fixture() -> Result<&'static str, HazardClosureReason> {
    let network = class("class.network", 1, EffectClassTraits::default());
    let process = class("class.process", 2, EffectClassTraits::default());
    let (policy, _) = policy(
        vec![network, process],
        vec![
            pattern("stage.network", network),
            pattern("stage.process", process),
        ],
        vec![],
        16,
        1_000,
    );
    let old = [authority(
        "effect.old-network",
        "root/old/network",
        "host.one",
        network,
        "audience.user",
        "network",
        DelegationPolicy::None,
    )];
    let new_and_rollback = [authority(
        "effect.new-process",
        "root/new/process",
        "host.one",
        process,
        "audience.user",
        "process",
        DelegationPolicy::None,
    )];
    let subject = transition_effect_closure_subject(
        &old,
        &new_and_rollback,
        &[],
        &[],
        2,
        Id("clock.monotonic"),
    )
    .unwrap();
    let mut proof = [None; MAX_HAZARD_PROOF_NODES];
    match analyze_transition_effect_closure(
        policy,
        TransitionEffectClosure {
            old_authorities: &old,
            new_and_rollback_authorities: &new_and_rollback,
            old_flows: &[],
            new_and_rollback_flows: &[],
        },
        &[],
        HazardClosureContext {
            plan_subject: subject,
            epoch: 2,
            time: AuthorityTime {
                basis: Id("clock.monotonic"),
                tick: 10,
            },
        },
        &mut proof,
    ) {
        Ok(report) => Ok(match report.disposition {
            HazardClosureDisposition::Accepted => "accepted",
            HazardClosureDisposition::Permitted => "permitted",
        }),
        Err(error) => Err(error.reason),
    }
}

fn run_fixture(scenario: &str) -> Result<&'static str, HazardClosureReason> {
    match scenario {
        "isolated-network"
        | "isolated-write"
        | "isolated-process"
        | "toxic-triad"
        | "exact-permit"
        | "permit-plan-mismatch"
        | "permit-epoch-mismatch"
        | "permit-artifact-mismatch"
        | "permit-host-mismatch"
        | "permit-realm-mismatch"
        | "permit-budget-mismatch"
        | "permit-expired"
        | "permit-approval-invalid"
        | "composite-hidden"
        | "constraint-distinction"
        | "proof-exhaustion"
        | "search-exhaustion" => run_simple_fixture(scenario),
        "propagation-chain" | "remote-federation" => run_propagation_fixture(scenario),
        "transition-overlap" => run_transition_fixture(),
        "one-permit-two-occurrences" => multiple_occurrence_case(false).map(|_| "permitted"),
        "degradation-no-bypass" => {
            // The unrelated optional effect is absent; the remaining closure is
            // reconstructed and must still match the complete toxic rule.
            run_simple_fixture("toxic-triad")
        }
        _ => panic!("unregistered hazard-closure fixture scenario {scenario}"),
    }
}

#[test]
fn every_hazard_closure_fixture_executes_independently() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/c2/hazard-closure.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 22);
    for case in cases {
        let scenario = case["scenario"].as_str().unwrap();
        let expected = case["expected"].as_str().unwrap();
        let actual = match run_fixture(scenario) {
            Ok(value) => value,
            Err(reason) => reason.code(),
        };
        assert_eq!(actual, expected, "fixture scenario {scenario}");
    }
}
