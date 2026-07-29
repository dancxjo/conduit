use conduit_core::{
    AuthorityConstraintRef, AuthorityEventKind, AuthorityGrant, AuthorityReason, AuthorityScope,
    AuthorityTime, CanonicalValue, DelegationPolicy, EffectRequirement, EvidenceValue, GrantStatus,
    HostCapability, Id, InstancePath, NodeEffectSet, ObservedGrant, PlacedEffect, ResourceRef,
    ResourceSelector, SemanticHash, Sensitivity, SensitivityDisposition, SensitivityReason,
    SensitivityUse, StopPolicy, TerminalCauseCode, TypeContractRef,
    aggregate_composite_effect_sets, assess_sensitivity, authority_bound_event,
    authority_denial_event, authority_terminal_cause, resolve_authority, resolve_authority_plan,
    validate_authority_at_use,
};

const RESOURCE: ResourceRef<'static> = ResourceRef {
    kind: Id("fixture/microphone"),
    id: Id("fixture/device-a"),
};
const OTHER_RESOURCE: ResourceRef<'static> = ResourceRef {
    kind: Id("fixture/microphone"),
    id: Id("fixture/device-b"),
};
const VALUE_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([3; 32]),
};
const CONSTRAINT: AuthorityConstraintRef<'static> = AuthorityConstraintRef {
    id: Id("fixture/duration"),
    semantic_hash: SemanticHash::from_bytes([4; 32]),
};
const OTHER_CONSTRAINT: AuthorityConstraintRef<'static> = AuthorityConstraintRef {
    id: Id("fixture/rate"),
    semantic_hash: SemanticHash::from_bytes([5; 32]),
};

fn time(tick: u64) -> AuthorityTime<'static> {
    AuthorityTime {
        basis: Id("clock/monotonic"),
        tick,
    }
}

fn effect(resource: ResourceSelector<'static>) -> EffectRequirement<'static> {
    EffectRequirement {
        id: Id("capture"),
        administrative_class: None,
        policy_budget_class: None,
        action: Id("audio/capture"),
        resource,
        requester: InstancePath::new("root/capture").unwrap(),
        audience: Id("fixture/run"),
        constraints: &[CONSTRAINT],
        check_at_use: true,
    }
}

fn capability(
    id: &'static str,
    resource: ResourceRef<'static>,
    host: &'static str,
) -> HostCapability<'static> {
    HostCapability {
        id: Id(id),
        action: Id("audio/capture"),
        resource,
        host: Id(host),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 100,
    }
}

fn grant(
    id: &'static str,
    resource: ResourceRef<'static>,
    root: &'static str,
) -> AuthorityGrant<'static> {
    AuthorityGrant {
        id: Id(id),
        action: Id("audio/capture"),
        resource,
        scope: AuthorityScope {
            root: InstancePath::new(root).unwrap(),
            descendants: true,
        },
        audience: Id("fixture/run"),
        constraints: &[CONSTRAINT],
        time_basis: Id("clock/monotonic"),
        not_before_tick: 0,
        expires_at_tick: 80,
        issued_for_host: Id("host/a"),
        delegation: DelegationPolicy::SameHostDescendants,
        audit_id: Id("fixture/audit"),
        terminal_policy: StopPolicy::Abort,
    }
}

#[test]
fn capability_and_grant_are_both_required_and_selection_is_stable() {
    let requirement = effect(ResourceSelector::Exact(RESOURCE));
    let caps = [
        capability("cap-b", RESOURCE, "host/a"),
        capability("cap-a", RESOURCE, "host/a"),
    ];
    let grants = [
        ObservedGrant {
            grant: grant("grant-b", RESOURCE, "root"),
            status: GrantStatus::Active,
        },
        ObservedGrant {
            grant: grant("grant-a", RESOURCE, "root"),
            status: GrantStatus::Active,
        },
    ];
    let binding =
        resolve_authority(requirement, Id("host/a"), time(10), &caps, &grants).expect("authorized");
    assert_eq!(binding.grant_id, Id("grant-a"));
    assert_eq!(binding.capability_id, Id("cap-a"));
    assert_eq!(binding.resource, RESOURCE);
    let event = authority_bound_event(0, requirement, binding);
    assert_eq!(event.kind, AuthorityEventKind::Bound);
    assert_eq!(event.audit_id, Some(Id("fixture/audit")));

    assert_eq!(
        resolve_authority(requirement, Id("host/a"), time(10), &caps, &[])
            .unwrap_err()
            .reason,
        AuthorityReason::GrantMissing
    );
    assert_eq!(
        resolve_authority(requirement, Id("host/a"), time(10), &[], &grants)
            .unwrap_err()
            .reason,
        AuthorityReason::CapabilityMissing
    );
}

#[test]
fn authority_plan_resolution_is_all_or_nothing() {
    let first = effect(ResourceSelector::Exact(RESOURCE));
    let second = EffectRequirement {
        id: Id("second"),
        action: Id("filesystem/write"),
        requester: InstancePath::new("root/sink").unwrap(),
        ..first
    };
    let cap = capability("cap", RESOURCE, "host/a");
    let grant = ObservedGrant {
        grant: grant("grant", RESOURCE, "root"),
        status: GrantStatus::Active,
    };
    let mut bindings = [None; 2];
    let denial = resolve_authority_plan(
        &[
            PlacedEffect {
                effect: first,
                host: Id("host/a"),
            },
            PlacedEffect {
                effect: second,
                host: Id("host/a"),
            },
        ],
        time(10),
        &[cap],
        &[grant],
        &mut bindings,
    )
    .unwrap_err();
    assert_eq!(denial.effect_id, Id("second"));
    assert_eq!(bindings, [None, None]);
}

#[test]
fn mismatched_scope_resource_expiry_and_cross_host_delegation_deny() {
    let requirement = effect(ResourceSelector::Exact(RESOURCE));
    let cap = capability("cap-a", RESOURCE, "host/a");
    for (observed, host, reason) in [
        (
            ObservedGrant {
                grant: grant("grant", OTHER_RESOURCE, "root"),
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::ResourceMismatch,
        ),
        (
            ObservedGrant {
                grant: grant("grant", RESOURCE, "other"),
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::ScopeMismatch,
        ),
        (
            ObservedGrant {
                grant: AuthorityGrant {
                    expires_at_tick: 10,
                    ..grant("grant", RESOURCE, "root")
                },
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::Expired,
        ),
        (
            ObservedGrant {
                grant: AuthorityGrant {
                    audience: Id("fixture/other"),
                    ..grant("grant", RESOURCE, "root")
                },
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::AudienceMismatch,
        ),
        (
            ObservedGrant {
                grant: AuthorityGrant {
                    constraints: &[],
                    ..grant("grant", RESOURCE, "root")
                },
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::ConstraintMismatch,
        ),
        (
            ObservedGrant {
                grant: AuthorityGrant {
                    time_basis: Id("clock/wall"),
                    ..grant("grant", RESOURCE, "root")
                },
                status: GrantStatus::Active,
            },
            Id("host/a"),
            AuthorityReason::TimeBasisMismatch,
        ),
    ] {
        assert_eq!(
            resolve_authority(requirement, host, time(10), &[cap], &[observed])
                .unwrap_err()
                .reason,
            reason
        );
    }

    let remote_cap = capability("remote", RESOURCE, "host/b");
    let nondelegable = ObservedGrant {
        grant: AuthorityGrant {
            delegation: DelegationPolicy::None,
            ..grant("grant", RESOURCE, "root/capture")
        },
        status: GrantStatus::Active,
    };
    assert_eq!(
        resolve_authority(
            requirement,
            Id("host/b"),
            time(10),
            &[remote_cap],
            &[nondelegable],
        )
        .unwrap_err()
        .reason,
        AuthorityReason::DelegationDenied
    );
}

#[test]
fn revocation_is_checked_at_use_and_maps_to_lifecycle() {
    let requirement = effect(ResourceSelector::Exact(RESOURCE));
    let cap = capability("cap", RESOURCE, "host/a");
    let active = ObservedGrant {
        grant: grant("grant", RESOURCE, "root"),
        status: GrantStatus::Active,
    };
    let binding =
        resolve_authority(requirement, Id("host/a"), time(10), &[cap], &[active]).unwrap();
    let revoked = ObservedGrant {
        status: GrantStatus::Revoked {
            at_tick: 20,
            reason: Id("fixture/operator"),
        },
        ..active
    };
    let denial =
        validate_authority_at_use(binding, requirement, time(20), cap, revoked).unwrap_err();
    assert_eq!(denial.reason, AuthorityReason::Revoked);
    let cause = authority_terminal_cause(denial, active.grant).unwrap();
    assert_eq!(cause.code, TerminalCauseCode::AuthorityRevoked);
    assert_eq!(cause.stop, StopPolicy::Abort);
    assert!(denial.to_string().contains("root/capture"));
    let event = authority_denial_event(1, denial, Some(active.grant));
    assert_eq!(event.kind, AuthorityEventKind::Revoked);
    assert_eq!(event.grant_id, Some(Id("grant")));
}

#[test]
fn composites_aggregate_every_reachable_effect_without_exports_hiding_it() {
    let first = effect(ResourceSelector::Exact(RESOURCE));
    let second = EffectRequirement {
        id: Id("write"),
        action: Id("filesystem/write"),
        requester: InstancePath::new("root/sink").unwrap(),
        ..first
    };
    let mut output = [None; 2];
    let children = [
        NodeEffectSet {
            definition: Id("fixture/source"),
            instance: InstancePath::new("root/capture").unwrap(),
            effects: &[first],
        },
        NodeEffectSet {
            definition: Id("fixture/sink"),
            instance: InstancePath::new("root/sink").unwrap(),
            effects: &[second],
        },
    ];
    assert_eq!(
        aggregate_composite_effect_sets(&children, &mut output),
        Ok(2)
    );
    assert_eq!(output, [Some(first), Some(second)]);
    assert_eq!(
        aggregate_composite_effect_sets(&children, &mut [None; 1]),
        Err(AuthorityReason::StorageTooSmall)
    );
}

#[test]
fn protected_evidence_cannot_contain_value_material() {
    let redacted = EvidenceValue::redacted(Sensitivity::Secret, VALUE_TYPE, true).unwrap();
    assert!(format!("{redacted:?}").contains("Secret"));
    assert!(format!("{redacted:?}").contains("present: true"));
    assert_eq!(
        EvidenceValue::redacted(Sensitivity::Public, VALUE_TYPE, true),
        Err(AuthorityReason::InvalidDescriptor)
    );
    let public = EvidenceValue::public(VALUE_TYPE, CanonicalValue::Text("visible"));
    assert!(format!("{public:?}").contains("visible"));

    assert_eq!(
        assess_sensitivity(
            Sensitivity::Secret,
            Sensitivity::Public,
            SensitivityUse::Connect,
            Some(Id("conduit/data.present")),
        )
        .reason,
        SensitivityReason::DestinationTooWeak
    );
    assert_eq!(
        assess_sensitivity(
            Sensitivity::Secret,
            Sensitivity::Secret,
            SensitivityUse::Present,
            None,
        )
        .reason,
        SensitivityReason::GrantRequired
    );
    assert_eq!(
        assess_sensitivity(
            Sensitivity::Secret,
            Sensitivity::Secret,
            SensitivityUse::Present,
            Some(Id("conduit/data.present")),
        )
        .disposition,
        SensitivityDisposition::Value
    );
    assert_eq!(
        assess_sensitivity(
            Sensitivity::Secret,
            Sensitivity::Secret,
            SensitivityUse::Evidence,
            Some(Id("conduit/data.present")),
        )
        .disposition,
        SensitivityDisposition::Redacted
    );
}

#[test]
fn effect_and_grant_identities_cover_authority_facts() {
    let requirement = effect(ResourceSelector::Exact(RESOURCE));
    let requirement_hash = requirement.semantic_hash().unwrap();
    assert_ne!(
        requirement_hash,
        EffectRequirement {
            audience: Id("fixture/other"),
            ..requirement
        }
        .semantic_hash()
        .unwrap()
    );
    assert_eq!(
        EffectRequirement {
            constraints: &[CONSTRAINT, OTHER_CONSTRAINT],
            ..requirement
        }
        .semantic_hash()
        .unwrap(),
        EffectRequirement {
            constraints: &[OTHER_CONSTRAINT, CONSTRAINT],
            ..requirement
        }
        .semantic_hash()
        .unwrap()
    );
    let base = grant("grant", RESOURCE, "root");
    assert_ne!(
        base.semantic_hash().unwrap(),
        AuthorityGrant {
            delegation: DelegationPolicy::CrossHostDescendants,
            ..base
        }
        .semantic_hash()
        .unwrap()
    );
    assert_ne!(
        base.semantic_hash().unwrap(),
        AuthorityGrant {
            time_basis: Id("clock/wall"),
            ..base
        }
        .semantic_hash()
        .unwrap()
    );

    let fixture = include_str!("../../../conformance/c2/authority-v1.tsv");
    for case in [
        "allow",
        "deny_missing_grant",
        "resource_mismatch",
        "scope_mismatch",
        "expiry",
        "nondelegable_cross_host",
        "sensitivity_downgrade",
        "composite_aggregation",
        "redacted_evidence",
    ] {
        assert!(
            fixture.lines().any(|line| line.starts_with(case)),
            "missing fixture {case}"
        );
    }
}
