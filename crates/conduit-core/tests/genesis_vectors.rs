use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal, AdministrativeProof,
    AdministrativeProposal, AdministrativeSubject, ArtifactDigest, AuthoritySurface,
    BootstrapAttempt, BootstrapChannel, BootstrapOrigin, CONTAINMENT_POLICY_SCHEMA_VERSION,
    ContainmentContext, ContainmentPolicy, DISTRIBUTION_PROFILE_SCHEMA_VERSION,
    DistributionProvider, GENESIS_CONTROL_SCHEMA_VERSION, GENESIS_PROFILE_SCHEMA_VERSION,
    GenesisControlKind, GenesisControlRecord, GenesisPhase, GenesisReason, GenesisStateObservation,
    HostDistributionKind, Id, MemberDisposition, MemberSecurityState, PinnedDescriptor,
    ProviderAvailability, ProviderEnablement, ProviderRequirement, ProviderRiskTraits,
    ProviderSelection, PublicGenesisOperation, RealmGenesisClass, RealmGenesisProfile,
    RecoveryKind, RecoveryTransition, ReferenceDistributionProfile, SafePlanDisposition,
    SemanticHash, assess_provider_requirement, authorize_public_operation, require_provider,
    validate_bootstrap_attempt, validate_genesis_control_record, validate_genesis_profile,
    validate_provider_enablement, validate_quarantined_member, validate_recovery_transition,
    validate_reference_distribution, validate_safe_initial_state,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../conformance/c4/safe-genesis.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const CHANNELS: [BootstrapChannel; 4] = [
    BootstrapChannel::PhysicalPresence,
    BootstrapChannel::Usb,
    BootstrapChannel::Ble,
    BootstrapChannel::TemporaryLocal,
];

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

fn profile(class: RealmGenesisClass) -> RealmGenesisProfile<'static> {
    let public_operations: &'static [PublicGenesisOperation<'static>] =
        if class == RealmGenesisClass::DeliberatelyPublic {
            Box::leak(
                vec![PublicGenesisOperation {
                    operation: pin("operation.public-observe", 20),
                    maximum_uses: 4,
                    administrative: false,
                    deployment: false,
                    protected_subscription: false,
                    actuating: false,
                }]
                .into_boxed_slice(),
            )
        } else {
            &[]
        };
    let mut profile = RealmGenesisProfile {
        schema_version: GENESIS_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("profile.genesis", 1),
        class,
        safe_plan: pin("plan.safe-disabled", 2),
        safe_plan_disposition: if class == RealmGenesisClass::SimulationOnly {
            SafePlanDisposition::SimulationOnly
        } else {
            SafePlanDisposition::Disabled
        },
        local_bootstrap_realm: Some(Id("realm.local-bootstrap")),
        bootstrap_identity: Some(Id("entity.bootstrap")),
        bootstrap_authority: pin("authority.local-bootstrap", 3),
        control_recorder: pin("recorder.genesis-control", 4),
        recovery_effect_class: pin("effect.recovery", 5),
        recovery_operation: pin("operation.recover", 6),
        bootstrap_channels: &CHANNELS,
        time_basis: Id("clock.monotonic"),
        bootstrap_ttl_ticks: 20,
        maximum_bootstrap_attempts: 3,
        maximum_evidence_events: 16,
        public_operations,
    };
    let mut scratch = [ZERO; 32];
    profile.identity = profile.computed_semantic_hash(&mut scratch).unwrap();
    profile
}

fn quarantined(entity: &'static str) -> MemberSecurityState<'static> {
    MemberSecurityState {
        entity: Id(entity),
        passport: hash(30),
        disposition: MemberDisposition::Quarantined,
        roles: &[],
        grants: &[],
        delegations: &[],
        federations: 0,
        installed_providers: 0,
        protected_subscriptions: 0,
        remote_plan_activations: 0,
        administrative_effects: 0,
        actuating_effects: 0,
    }
}

fn initial_state<'a>(
    profile: RealmGenesisProfile<'a>,
    phase: GenesisPhase,
    members: &'a [MemberSecurityState<'a>],
) -> GenesisStateObservation<'a> {
    GenesisStateObservation {
        profile_identity: profile.identity,
        phase,
        realm: if phase == GenesisPhase::LocalBootstrap {
            profile.local_bootstrap_realm
        } else {
            None
        },
        active_plan: profile.safe_plan,
        active_plan_disposition: profile.safe_plan_disposition,
        remote_discovery_enabled: false,
        public_listener_enabled: false,
        unrestricted_network_enabled: false,
        members,
        federations: 0,
        authority_grants: 0,
        dangerous_providers_enabled: 0,
    }
}

fn evidence(
    profile: RealmGenesisProfile<'static>,
    kind: GenesisControlKind,
    subject: &'static str,
    sequence: u64,
    tick: u64,
    receipt_byte: u8,
) -> GenesisControlRecord<'static> {
    let mut record = GenesisControlRecord {
        schema_version: GENESIS_CONTROL_SCHEMA_VERSION,
        identity: ZERO,
        sequence,
        predecessor: if sequence == 1 {
            None
        } else {
            Some(hash(receipt_byte.wrapping_sub(1)))
        },
        profile_identity: profile.identity,
        kind,
        subject: Id(subject),
        authority: profile.control_recorder,
        time_basis: profile.time_basis,
        observed_at_tick: tick,
        receipt: hash(receipt_byte),
    };
    record.identity = record.computed_semantic_hash().unwrap();
    record
}

fn bootstrap(
    profile: RealmGenesisProfile<'static>,
    origin: BootstrapOrigin,
) -> BootstrapAttempt<'static> {
    BootstrapAttempt {
        id: Id("bootstrap.one"),
        profile_identity: profile.identity,
        candidate_entity: Id("entity.candidate"),
        candidate_key: Id("key.candidate"),
        authorization: profile.bootstrap_authority,
        origin,
        channel: Some(BootstrapChannel::Usb),
        time_basis: profile.time_basis,
        issued_at_tick: 10,
        expires_at_tick: 20,
        ordinal: 1,
        local_confirmation: true,
        replayed: false,
        remote_session: false,
        receipt: hash(40),
    }
}

fn dangerous_traits() -> ProviderRiskTraits {
    ProviderRiskTraits {
        enrollment_issuer: false,
        unrestricted_native_execution: false,
        remote_artifact_installation: false,
        firmware_mutation: true,
        unrestricted_network: false,
        realm_root_administration: false,
        remote_plan_activation: false,
        actuating_effects: false,
    }
}

fn distribution<'a>(
    genesis: RealmGenesisProfile<'static>,
    kind: HostDistributionKind,
    providers: &'a [DistributionProvider<'a>],
) -> ReferenceDistributionProfile<'a> {
    let mut profile = ReferenceDistributionProfile {
        schema_version: DISTRIBUTION_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("distribution.reference", 50),
        kind,
        genesis_profile: genesis.identity,
        control_recorder: genesis.control_recorder,
        provider_enablement_effect_class: pin("effect.provider-enablement", 51),
        provider_enablement_operation: pin("operation.provider-enable", 52),
        providers,
        maximum_provider_enablement_ticks: 20,
        maximum_provider_install_attempts: 2,
        maximum_evidence_events: 16,
    };
    let mut scratch = [ZERO; 32];
    profile.identity = profile.computed_semantic_hash(&mut scratch).unwrap();
    profile
}

fn principal(
    entity: &'static str,
    key: &'static str,
    plan_byte: u8,
) -> AdministrativePrincipal<'static> {
    AdministrativePrincipal {
        realm: Id("realm.alpha"),
        entity: Id(entity),
        key: Id(key),
        profile: pin("profile.member", 60),
        source_plan: hash(plan_byte),
        source_epoch: 1,
    }
}

fn administrative_proof(
    subject: AdministrativeSubject<'static>,
    effect_class: PinnedDescriptor<'static>,
    operation: PinnedDescriptor<'static>,
) -> AdministrativeProof<'static> {
    let requester = principal("requester", "key.requester", 61);
    let approver = principal("approver", "key.approver", 62);
    let committer = principal("committer", "key.committer", 63);
    let executor = principal("executor", "key.executor", 64);
    let failure = pin("failure.independent", 65);
    let approvers: &'static [AdministrativeApprover<'static>] = Box::leak(
        vec![AdministrativeApprover {
            realm: approver.realm,
            entity: approver.entity,
            key: approver.key,
            profile: approver.profile,
            failure_domain: failure,
        }]
        .into_boxed_slice(),
    );
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.genesis-admin", 66),
        effect_class,
        approvers,
        committer: AdministrativeApprover {
            realm: committer.realm,
            entity: committer.entity,
            key: committer.key,
            profile: committer.profile,
            failure_domain: pin("failure.committer", 67),
        },
        executor: AdministrativeApprover {
            realm: executor.realm,
            entity: executor.entity,
            key: executor.key,
            profile: executor.profile,
            failure_domain: pin("failure.executor", 68),
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
        id: Id("proposal.genesis-admin"),
        effect_class,
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
        id: Id("approval.genesis-admin"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approver,
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
        id: Id("commit.genesis-admin"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: approval_hashes,
        committed_by: committer,
        committed_at_tick: 8,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.genesis-admin"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor,
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

fn outcome(result: Result<(), GenesisReason>) -> Value {
    match result {
        Ok(()) => json!({"accepted": true}),
        Err(reason) => json!({"accepted": false, "code": reason.code()}),
    }
}

fn bootstrap_outcome(result: Result<MemberDisposition, GenesisReason>) -> Value {
    match result {
        Ok(MemberDisposition::Quarantined) => {
            json!({"accepted": true, "disposition": "quarantined"})
        }
        Ok(MemberDisposition::Authorized) => {
            json!({"accepted": true, "disposition": "authorized"})
        }
        Err(reason) => json!({"accepted": false, "code": reason.code()}),
    }
}

fn provider_case(
    kind: HostDistributionKind,
    availability: ProviderAvailability,
) -> Result<(), GenesisReason> {
    let genesis = profile(RealmGenesisClass::Private);
    let providers = [DistributionProvider {
        provider: pin("provider.firmware", 70),
        artifact: Some(ArtifactDigest::from_bytes([71; 32])),
        availability,
        traits: dangerous_traits(),
    }];
    let distribution = distribution(genesis, kind, &providers);
    require_provider(
        distribution,
        ProviderRequirement {
            provider: providers[0].provider,
            traits: dangerous_traits(),
        },
    )
    .map(|_| ())
}

fn provider_enablement_case(with_grant: bool) -> Result<(), GenesisReason> {
    let genesis = profile(RealmGenesisClass::Private);
    let artifact = ArtifactDigest::from_bytes([71; 32]);
    let providers: &'static [DistributionProvider<'static>] = Box::leak(
        vec![DistributionProvider {
            provider: pin("provider.firmware", 70),
            artifact: Some(artifact),
            availability: ProviderAvailability::Disabled,
            traits: dangerous_traits(),
        }]
        .into_boxed_slice(),
    );
    let distribution = distribution(genesis, HostDistributionKind::Hosted, providers);
    let subject = AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("entity.target"),
        plan: hash(72),
        epoch: 1,
        artifact: Some(artifact),
        budget: Some(providers[0].provider),
    };
    let approval = administrative_proof(
        subject,
        distribution.provider_enablement_effect_class,
        distribution.provider_enablement_operation,
    );
    let context = ContainmentContext {
        subject,
        time_basis: Id("clock.monotonic"),
        now_tick: 10,
    };
    let mut control = GenesisControlRecord {
        schema_version: GENESIS_CONTROL_SCHEMA_VERSION,
        identity: ZERO,
        sequence: 2,
        predecessor: Some(hash(73)),
        profile_identity: genesis.identity,
        kind: GenesisControlKind::ProviderEnabled,
        subject: providers[0].provider.id,
        authority: distribution.control_recorder,
        time_basis: Id("clock.monotonic"),
        observed_at_tick: 10,
        receipt: hash(74),
    };
    control.identity = control.computed_semantic_hash().unwrap();
    let grants: &'static [SemanticHash] = if with_grant {
        Box::leak(vec![hash(75)].into_boxed_slice())
    } else {
        &[]
    };
    validate_provider_enablement(
        distribution,
        ProviderEnablement {
            distribution_identity: distribution.identity,
            provider: providers[0].provider,
            artifact,
            ordinal: 1,
            time_basis: Id("clock.monotonic"),
            enabled_at_tick: 10,
            expires_at_tick: 20,
            effect_grants: grants,
            evidence: control,
            approval,
        },
        context,
    )
}

fn recovery_case(
    kind: RecoveryKind,
    candidate: AuthoritySurface,
    mismatched_snapshot: bool,
) -> Result<(), GenesisReason> {
    let profile = profile(RealmGenesisClass::Private);
    let prior = AuthoritySurface {
        members: 3,
        grants: 2,
        delegations: 1,
        federations: 1,
        executable_providers: 1,
        root_authorities: 1,
        remote_plan_activations: 1,
        protected_subscriptions: 1,
        actuating_bindings: 1,
        remote_discovery: true,
        public_listener: false,
        unrestricted_network: false,
        ambient_root: false,
        trust_on_first_use: false,
    };
    let restoring = matches!(
        kind,
        RecoveryKind::Restore | RecoveryKind::Rollback | RecoveryKind::Emergency
    );
    let snapshot = hash(80);
    let subject = AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("entity.target"),
        plan: if mismatched_snapshot {
            hash(81)
        } else {
            snapshot
        },
        epoch: 1,
        artifact: None,
        budget: Some(profile.descriptor),
    };
    let approval = restoring.then(|| {
        administrative_proof(
            subject,
            profile.recovery_effect_class,
            profile.recovery_operation,
        )
    });
    let evidence_kind = match kind {
        RecoveryKind::FactoryReset => GenesisControlKind::FactoryReset,
        RecoveryKind::LostRoot | RecoveryKind::FailedRestore => GenesisControlKind::RecoveryDenied,
        RecoveryKind::Restore | RecoveryKind::Rollback | RecoveryKind::Emergency => {
            GenesisControlKind::Restored
        }
    };
    validate_recovery_transition(
        profile,
        RecoveryTransition {
            profile_identity: profile.identity,
            kind,
            prior,
            candidate,
            recovery_ceiling: restoring.then_some(prior),
            snapshot: restoring.then_some(snapshot),
            evidence: evidence(profile, evidence_kind, "entity.target", 3, 10, 82),
            approval,
        },
        ContainmentContext {
            subject,
            time_basis: Id("clock.monotonic"),
            now_tick: 10,
        },
    )
}

fn run_fixture_case(case: &str) -> Value {
    let private = profile(RealmGenesisClass::Private);
    match case {
        "fresh-unconfigured-is-isolated" | "hostile-lan-does-not-change-safe-state" => {
            let mut scratch = [ZERO; 32];
            outcome(validate_safe_initial_state(
                private,
                initial_state(private, GenesisPhase::Unconfigured, &[]),
                &mut scratch,
            ))
        }
        "local-bootstrap-yields-quarantine" => {
            let attempt = bootstrap(private, BootstrapOrigin::LocalCeremony);
            bootstrap_outcome(validate_bootstrap_attempt(
                private,
                attempt,
                evidence(
                    private,
                    GenesisControlKind::BootstrapRequested,
                    "entity.candidate",
                    1,
                    10,
                    40,
                ),
                15,
            ))
        }
        "expired-local-bootstrap-denied" => {
            let attempt = bootstrap(private, BootstrapOrigin::LocalCeremony);
            bootstrap_outcome(validate_bootstrap_attempt(
                private,
                attempt,
                evidence(
                    private,
                    GenesisControlKind::BootstrapRequested,
                    "entity.candidate",
                    1,
                    10,
                    40,
                ),
                20,
            ))
        }
        "replayed-local-bootstrap-denied" => {
            let mut attempt = bootstrap(private, BootstrapOrigin::LocalCeremony);
            attempt.replayed = true;
            bootstrap_outcome(validate_bootstrap_attempt(
                private,
                attempt,
                evidence(
                    private,
                    GenesisControlKind::BootstrapRequested,
                    "entity.candidate",
                    1,
                    10,
                    40,
                ),
                15,
            ))
        }
        "bootstrap-retry-limit-enforced" => {
            let mut attempt = bootstrap(private, BootstrapOrigin::LocalCeremony);
            attempt.ordinal = 4;
            bootstrap_outcome(validate_bootstrap_attempt(
                private,
                attempt,
                evidence(
                    private,
                    GenesisControlKind::BootstrapRequested,
                    "entity.candidate",
                    1,
                    10,
                    40,
                ),
                15,
            ))
        }
        "browser-navigation-does-not-enroll"
        | "pwa-install-does-not-enroll"
        | "browser-permission-does-not-enroll"
        | "transport-handshake-does-not-enroll"
        | "capability-report-does-not-enroll" => {
            let origin = match case {
                "browser-navigation-does-not-enroll" => BootstrapOrigin::BrowserNavigation,
                "pwa-install-does-not-enroll" => BootstrapOrigin::PwaInstall,
                "browser-permission-does-not-enroll" => BootstrapOrigin::BrowserPermission,
                "transport-handshake-does-not-enroll" => BootstrapOrigin::TransportHandshake,
                _ => BootstrapOrigin::CapabilityReport,
            };
            bootstrap_outcome(validate_bootstrap_attempt(
                private,
                bootstrap(private, origin),
                evidence(
                    private,
                    GenesisControlKind::BootstrapRequested,
                    "entity.candidate",
                    1,
                    10,
                    40,
                ),
                15,
            ))
        }
        "quarantined-member-has-no-ambient-authority" => {
            outcome(validate_quarantined_member(quarantined("entity.candidate")))
        }
        "role-breaks-quarantine" => {
            let roles: &'static [PinnedDescriptor<'static>] =
                Box::leak(vec![pin("role.member", 90)].into_boxed_slice());
            let mut member = quarantined("entity.candidate");
            member.roles = roles;
            outcome(validate_quarantined_member(member))
        }
        "grant-breaks-quarantine" => {
            let grants: &'static [SemanticHash] = Box::leak(vec![hash(91)].into_boxed_slice());
            let mut member = quarantined("entity.candidate");
            member.grants = grants;
            outcome(validate_quarantined_member(member))
        }
        "public-profile-exact-bounded-operation" => outcome(authorize_public_operation(
            profile(RealmGenesisClass::DeliberatelyPublic),
            pin("operation.public-observe", 20),
            4,
        )),
        "public-profile-administration-denied" => outcome(authorize_public_operation(
            profile(RealmGenesisClass::DeliberatelyPublic),
            pin("operation.realm-admin", 92),
            1,
        )),
        "hosted-reference-dangerous-providers-absent" => {
            let providers = [DistributionProvider {
                provider: pin("provider.firmware", 70),
                artifact: None,
                availability: ProviderAvailability::Absent,
                traits: dangerous_traits(),
            }];
            let distribution = distribution(private, HostDistributionKind::Hosted, &providers);
            let mut scratch = [ZERO; 32];
            outcome(validate_reference_distribution(distribution, &mut scratch))
        }
        "browser-profile-unsupported-is-explicit" => outcome(provider_case(
            HostDistributionKind::Browser,
            ProviderAvailability::Unsupported,
        )),
        "constrained-profile-unsupported-is-explicit" => outcome(provider_case(
            HostDistributionKind::Constrained,
            ProviderAvailability::Unsupported,
        )),
        "dangerous-provider-enabled-by-default-denied" => {
            let providers = [DistributionProvider {
                provider: pin("provider.firmware", 70),
                artifact: Some(ArtifactDigest::from_bytes([71; 32])),
                availability: ProviderAvailability::Enabled,
                traits: dangerous_traits(),
            }];
            let distribution = distribution(private, HostDistributionKind::Hosted, &providers);
            let mut scratch = [ZERO; 32];
            outcome(validate_reference_distribution(distribution, &mut scratch))
        }
        "absent-provider-requirement-is-exact" => {
            let providers = [DistributionProvider {
                provider: pin("provider.firmware", 70),
                artifact: None,
                availability: ProviderAvailability::Absent,
                traits: dangerous_traits(),
            }];
            let distribution = distribution(private, HostDistributionKind::Hosted, &providers);
            let decision = assess_provider_requirement(
                distribution,
                ProviderRequirement {
                    provider: providers[0].provider,
                    traits: dangerous_traits(),
                },
            )
            .unwrap();
            assert_eq!(decision.selection, ProviderSelection::Absent);
            let mut result = outcome(
                require_provider(
                    distribution,
                    ProviderRequirement {
                        provider: providers[0].provider,
                        traits: dangerous_traits(),
                    },
                )
                .map(|_| ()),
            );
            result["provider"] = Value::String(decision.provider.id.to_string());
            result
        }
        "deliberate-provider-opt-in-is-exact" => outcome(provider_enablement_case(false)),
        "provider-opt-in-does-not-grant-effects" => outcome(provider_enablement_case(true)),
        "factory-reset-returns-to-isolation" => outcome(recovery_case(
            RecoveryKind::FactoryReset,
            AuthoritySurface::default(),
            false,
        )),
        "failed-root-recovery-returns-to-isolation" => outcome(recovery_case(
            RecoveryKind::LostRoot,
            AuthoritySurface::default(),
            false,
        )),
        "failed-recovery-cannot-create-ambient-root" => outcome(recovery_case(
            RecoveryKind::FailedRestore,
            AuthoritySurface {
                ambient_root: true,
                ..AuthoritySurface::default()
            },
            false,
        )),
        "authorized-restore-uses-exact-snapshot-ceiling" => outcome(recovery_case(
            RecoveryKind::Restore,
            AuthoritySurface {
                members: 1,
                root_authorities: 1,
                ..AuthoritySurface::default()
            },
            false,
        )),
        "restore-snapshot-mismatch-denied" => outcome(recovery_case(
            RecoveryKind::Restore,
            AuthoritySurface {
                members: 1,
                root_authorities: 1,
                ..AuthoritySurface::default()
            },
            true,
        )),
        "emergency-recovery-cannot-create-universal-root" => outcome(recovery_case(
            RecoveryKind::Emergency,
            AuthoritySurface {
                root_authorities: 2,
                ..AuthoritySurface::default()
            },
            false,
        )),
        "genesis-evidence-capacity-is-bounded" => {
            let record = evidence(
                private,
                GenesisControlKind::BootstrapRequested,
                "entity.candidate",
                17,
                10,
                93,
            );
            outcome(validate_genesis_control_record(private, record))
        }
        "local-bootstrap-allows-only-pinned-identity" => {
            let members = [quarantined("entity.other")];
            let mut scratch = [ZERO; 32];
            outcome(validate_safe_initial_state(
                private,
                initial_state(private, GenesisPhase::LocalBootstrap, &members),
                &mut scratch,
            ))
        }
        other => panic!("unknown safe-genesis fixture case `{other}`"),
    }
}

#[test]
fn fresh_and_local_bootstrap_states_are_non_actuating_and_non_administrative() {
    let profile = profile(RealmGenesisClass::Private);
    let mut scratch = [ZERO; 32];
    assert_eq!(validate_genesis_profile(profile, &mut scratch), Ok(()));
    assert_eq!(
        validate_safe_initial_state(
            profile,
            initial_state(profile, GenesisPhase::Unconfigured, &[]),
            &mut scratch,
        ),
        Ok(())
    );
    let members = [quarantined("entity.bootstrap")];
    assert_eq!(
        validate_safe_initial_state(
            profile,
            initial_state(profile, GenesisPhase::LocalBootstrap, &members),
            &mut scratch,
        ),
        Ok(())
    );
}

#[test]
fn enrollment_sources_are_local_bounded_and_quarantined() {
    let profile = profile(RealmGenesisClass::Private);
    let record = evidence(
        profile,
        GenesisControlKind::BootstrapRequested,
        "entity.candidate",
        1,
        10,
        40,
    );
    assert_eq!(
        validate_bootstrap_attempt(
            profile,
            bootstrap(profile, BootstrapOrigin::LocalCeremony),
            record,
            15,
        ),
        Ok(MemberDisposition::Quarantined)
    );
    for origin in [
        BootstrapOrigin::NetworkAttachment,
        BootstrapOrigin::BrowserNavigation,
        BootstrapOrigin::PwaInstall,
        BootstrapOrigin::BrowserPermission,
        BootstrapOrigin::TransportHandshake,
        BootstrapOrigin::CapabilityReport,
        BootstrapOrigin::Callsign,
    ] {
        assert_eq!(
            validate_bootstrap_attempt(profile, bootstrap(profile, origin), record, 15),
            Err(GenesisReason::ImplicitOrRemoteBootstrap)
        );
    }
}

#[test]
fn public_profile_and_provider_defaults_are_exact_and_fail_closed() {
    let public = profile(RealmGenesisClass::DeliberatelyPublic);
    assert_eq!(
        authorize_public_operation(public, pin("operation.public-observe", 20), 4),
        Ok(())
    );
    assert_eq!(
        authorize_public_operation(public, pin("operation.realm-admin", 92), 1),
        Err(GenesisReason::PublicOperationDenied)
    );
    assert_eq!(
        provider_case(
            HostDistributionKind::Browser,
            ProviderAvailability::Unsupported
        ),
        Err(GenesisReason::ProviderUnavailable)
    );
    assert_eq!(
        provider_case(
            HostDistributionKind::Constrained,
            ProviderAvailability::Unsupported
        ),
        Err(GenesisReason::ProviderUnavailable)
    );
}

#[test]
fn provider_installation_is_independently_approved_but_never_grants_effects() {
    assert_eq!(provider_enablement_case(false), Ok(()));
    assert_eq!(
        provider_enablement_case(true),
        Err(GenesisReason::ProviderEnablementInvalid)
    );
}

#[test]
fn recovery_is_monotonic_and_exactly_snapshot_bound() {
    assert_eq!(
        recovery_case(
            RecoveryKind::FactoryReset,
            AuthoritySurface::default(),
            false
        ),
        Ok(())
    );
    assert_eq!(
        recovery_case(
            RecoveryKind::Restore,
            AuthoritySurface {
                members: 1,
                root_authorities: 1,
                ..AuthoritySurface::default()
            },
            false,
        ),
        Ok(())
    );
    assert_eq!(
        recovery_case(
            RecoveryKind::Restore,
            AuthoritySurface {
                members: 1,
                root_authorities: 1,
                ..AuthoritySurface::default()
            },
            true,
        ),
        Err(GenesisReason::RecoveryWidened)
    );
}

#[test]
fn every_safe_genesis_fixture_case_executes_independently() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.safe-genesis");
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert_eq!(run_fixture_case(id), case["expected"], "{id}");
    }
}
