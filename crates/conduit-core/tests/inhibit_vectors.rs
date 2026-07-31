use conduit_core::{
    AdministrativeApproval, AdministrativeApprovalStatus, AdministrativeApprover,
    AdministrativeCommit, AdministrativeExecution, AdministrativePrincipal, AdministrativeProof,
    AdministrativeProposal, AdministrativeSubject, CONTAINMENT_POLICY_SCHEMA_VERSION,
    ContainmentPolicy, EnvelopeValue, HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION, HazardArmRequest,
    HazardControlPhase, HazardControlState, HazardEvidenceKind, HazardEvidenceRecord,
    HazardousCommand, HazardousHostBinding, HazardousHostProfile, HostLifecycleChange,
    INHIBIT_OBSERVATION_SCHEMA_VERSION, Id, ImplementationConfinement, InhibitCause,
    InhibitClearRequest, InhibitLatchState, InhibitObservation, InhibitReason,
    OperatingEnvelopeLimit, PinnedDescriptor, SemanticHash, accept_hazardous_command,
    arm_hazardous_host, clear_inhibit, enforce_command_expiry, inhibit_hazardous_host,
    recover_after_host_change, validate_hazard_evidence, validate_hazardous_host_binding,
    validate_required_hazardous_host_binding,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../conformance/c4/inhibit-plane.json");
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

fn profile() -> HazardousHostProfile<'static> {
    let envelope: &'static [OperatingEnvelopeLimit<'static>] = Box::leak(
        vec![
            OperatingEnvelopeLimit {
                dimension: pin("domain.axis-a", 11),
                minimum: -10,
                maximum: 10,
            },
            OperatingEnvelopeLimit {
                dimension: pin("domain.energy-class", 12),
                minimum: 0,
                maximum: 50,
            },
        ]
        .into_boxed_slice(),
    );
    let mut profile = HazardousHostProfile {
        schema_version: HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("profile.hazardous-host", 1),
        safe_state: pin("domain.safe-state", 2),
        inhibit_boundary: pin("host.inhibit-boundary", 3),
        watchdog: pin("host.watchdog", 4),
        effect_boundary: pin("host.effect-boundary", 5),
        command_effect_class: pin("effect.command", 6),
        clear_effect_class: pin("effect.inhibit-clear", 7),
        clear_operation: pin("operation.inhibit-clear", 8),
        clear_ceremony: pin("ceremony.physical-clear", 9),
        time_basis: Id("clock.monotonic"),
        maximum_command_horizon_ticks: 10,
        maximum_observation_age_ticks: 20,
        maximum_evidence_records: 16,
        require_physical_presence_to_clear: true,
        require_isolated_implementation: true,
        envelope,
    };
    let mut scratch = [ZERO; 16];
    profile.identity = profile.computed_semantic_hash(&mut scratch).unwrap();
    profile
}

fn observation(profile: HazardousHostProfile<'static>) -> InhibitObservation<'static> {
    let mut observation = InhibitObservation {
        schema_version: INHIBIT_OBSERVATION_SCHEMA_VERSION,
        identity: ZERO,
        profile_identity: profile.identity,
        host: Id("host.effect"),
        safe_state: profile.safe_state,
        inhibit_boundary: profile.inhibit_boundary,
        watchdog: profile.watchdog,
        effect_boundary: profile.effect_boundary,
        time_basis: profile.time_basis,
        observed_at_tick: 10,
        valid_until_tick: 30,
        latch_generation: 1,
        latch_state: InhibitLatchState::SafeDisarmed,
        independent_from_plan: true,
        local_safe_path: true,
        survives_executor_loss: true,
        survives_partition: true,
        graph_cannot_replace: true,
        confinement: ImplementationConfinement::EffectBoundaryEnforced,
    };
    observation.identity = observation.computed_semantic_hash().unwrap();
    observation
}

fn binding(profile: HazardousHostProfile<'static>) -> HazardousHostBinding<'static> {
    HazardousHostBinding {
        host: Id("host.effect"),
        profile,
        observation: observation(profile),
    }
}

fn initial_state() -> HazardControlState {
    HazardControlState::safe_disarmed(1, hash(20))
}

fn armed(profile: HazardousHostProfile<'static>) -> HazardControlState {
    arm_hazardous_host(
        binding(profile),
        initial_state(),
        HazardArmRequest {
            plan: hash(21),
            epoch: 7,
            command_authority: hash(22),
            time_basis: Id("clock.monotonic"),
            now_tick: 15,
        },
        &mut [ZERO; 16],
    )
    .unwrap()
}

fn command(profile: HazardousHostProfile<'static>) -> HazardousCommand<'static> {
    let values: &'static [EnvelopeValue<'static>] = Box::leak(
        vec![
            EnvelopeValue {
                dimension: profile.envelope[0].dimension,
                value: 4,
            },
            EnvelopeValue {
                dimension: profile.envelope[1].dimension,
                value: 25,
            },
        ]
        .into_boxed_slice(),
    );
    HazardousCommand {
        plan: hash(21),
        epoch: 7,
        authority: hash(22),
        sequence: 1,
        time_basis: Id("clock.monotonic"),
        issued_at_tick: 15,
        expires_at_tick: 20,
        values,
    }
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

fn subject() -> AdministrativeSubject<'static> {
    AdministrativeSubject {
        realm: Id("realm.alpha"),
        entity: Id("host.effect"),
        plan: hash(21),
        epoch: 7,
        artifact: None,
        budget: None,
    }
}

fn clear_proof(
    profile: HazardousHostProfile<'static>,
    with_ceremony: bool,
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
    let ceremony = with_ceremony.then_some(profile.clear_ceremony);
    let mut policy = ContainmentPolicy {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("policy.inhibit-clear", 66),
        effect_class: profile.clear_effect_class,
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
        ceremony,
    };
    policy.identity = policy.computed_semantic_hash().unwrap();
    let subject = subject();
    let beneficiaries: &'static [AdministrativeSubject<'static>] =
        Box::leak(vec![subject].into_boxed_slice());
    let mut proposal = AdministrativeProposal {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("proposal.inhibit-clear"),
        effect_class: profile.clear_effect_class,
        operation: profile.clear_operation,
        requester,
        subject,
        beneficiaries,
        predecessor_plan: None,
        delegation: None,
        protected_handle: with_ceremony.then_some(profile.inhibit_boundary),
        ceremony,
        time_basis: profile.time_basis,
        created_at_tick: 15,
        expires_at_tick: 40,
    };
    proposal.identity = proposal.computed_semantic_hash().unwrap();
    let mut approval = AdministrativeApproval {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("approval.inhibit-clear"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approver,
        failure_domain: failure,
        time_basis: profile.time_basis,
        issued_at_tick: 16,
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
        id: Id("commit.inhibit-clear"),
        proposal_identity: proposal.identity,
        policy_identity: policy.identity,
        approvals: approval_hashes,
        committed_by: committer,
        committed_at_tick: 18,
    };
    commit.identity = commit.computed_semantic_hash().unwrap();
    let mut execution = AdministrativeExecution {
        schema_version: CONTAINMENT_POLICY_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("execution.inhibit-clear"),
        proposal_identity: proposal.identity,
        commit_identity: commit.identity,
        executor,
        time_basis: profile.time_basis,
        not_before_tick: 18,
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

fn outcome(result: Result<(), InhibitReason>) -> Value {
    match result {
        Ok(()) => json!({"accepted": true}),
        Err(reason) => json!({"accepted": false, "code": reason.code()}),
    }
}

fn state_outcome(result: Result<HazardControlState, InhibitReason>) -> Value {
    match result {
        Ok(state) => json!({
            "accepted": true,
            "phase": match state.phase {
                HazardControlPhase::SafeDisarmed => "safe-disarmed",
                HazardControlPhase::Armed => "armed",
                HazardControlPhase::Inhibited => "inhibited",
            }
        }),
        Err(reason) => json!({"accepted": false, "code": reason.code()}),
    }
}

fn run_case(id: &str) -> Value {
    let profile = profile();
    let binding = binding(profile);
    let armed = armed(profile);
    match id {
        "exact-independent-boundary-is-accepted" => outcome(validate_hazardous_host_binding(
            binding,
            Id("clock.monotonic"),
            15,
            &mut [ZERO; 16],
        )),
        "missing-interlock-is-not-observed-as-present" => {
            let mut binding = binding;
            binding.observation.local_safe_path = false;
            binding.observation.identity = binding.observation.computed_semantic_hash().unwrap();
            outcome(validate_hazardous_host_binding(
                binding,
                Id("clock.monotonic"),
                15,
                &mut [ZERO; 16],
            ))
        }
        "absent-inhibit-evidence-fails-resolution" => {
            outcome(validate_required_hazardous_host_binding(
                None,
                Id("clock.monotonic"),
                15,
                &mut [ZERO; 16],
            ))
        }
        "stale-inhibit-evidence-fails-run-start" => outcome(validate_hazardous_host_binding(
            binding,
            Id("clock.monotonic"),
            31,
            &mut [ZERO; 16],
        )),
        "unconfined-native-rejected-for-high-assurance" => {
            let mut binding = binding;
            binding.observation.confinement = ImplementationConfinement::UnconfinedNative;
            binding.observation.identity = binding.observation.computed_semantic_hash().unwrap();
            outcome(validate_hazardous_host_binding(
                binding,
                Id("clock.monotonic"),
                15,
                &mut [ZERO; 16],
            ))
        }
        "safe-disarmed-host-arms-exact-epoch" => state_outcome(arm_hazardous_host(
            binding,
            initial_state(),
            HazardArmRequest {
                plan: hash(21),
                epoch: 7,
                command_authority: hash(22),
                time_basis: Id("clock.monotonic"),
                now_tick: 15,
            },
            &mut [ZERO; 16],
        )),
        "latched-host-cannot-arm" => state_outcome(arm_hazardous_host(
            binding,
            inhibit_hazardous_host(initial_state(), hash(30), InhibitCause::StopRequest),
            HazardArmRequest {
                plan: hash(21),
                epoch: 7,
                command_authority: hash(22),
                time_basis: Id("clock.monotonic"),
                now_tick: 15,
            },
            &mut [ZERO; 16],
        )),
        "finite-command-lease-is-accepted" => state_outcome(accept_hazardous_command(
            profile,
            armed,
            command(profile),
            16,
        )),
        "expired-command-is-rejected" => state_outcome(accept_hazardous_command(
            profile,
            armed,
            command(profile),
            20,
        )),
        "delayed-command-is-rejected" => {
            let mut command = command(profile);
            command.issued_at_tick = 17;
            state_outcome(accept_hazardous_command(profile, armed, command, 16))
        }
        "duplicate-command-is-rejected" => {
            let accepted = accept_hazardous_command(profile, armed, command(profile), 16).unwrap();
            state_outcome(accept_hazardous_command(
                profile,
                accepted,
                command(profile),
                17,
            ))
        }
        "stale-epoch-command-is-rejected" => {
            let mut command = command(profile);
            command.epoch = 6;
            state_outcome(accept_hazardous_command(profile, armed, command, 16))
        }
        "wrong-authority-command-is-rejected" => {
            let mut command = command(profile);
            command.authority = hash(99);
            state_outcome(accept_hazardous_command(profile, armed, command, 16))
        }
        "over-envelope-command-is-rejected" => {
            let mut command = command(profile);
            command.values = Box::leak(
                vec![
                    EnvelopeValue {
                        dimension: profile.envelope[0].dimension,
                        value: 11,
                    },
                    EnvelopeValue {
                        dimension: profile.envelope[1].dimension,
                        value: 25,
                    },
                ]
                .into_boxed_slice(),
            );
            state_outcome(accept_hazardous_command(profile, armed, command, 16))
        }
        "command-expiry-enters-inhibited-safe-state" => {
            let accepted = accept_hazardous_command(profile, armed, command(profile), 16).unwrap();
            state_outcome(enforce_command_expiry(accepted, 20, hash(31)))
        }
        "partition-inhibits-without-remote-participation" => state_outcome(Ok(
            inhibit_hazardous_host(armed, hash(32), InhibitCause::Partition),
        )),
        "executor-death-inhibits-without-plan-callback" => state_outcome(Ok(
            inhibit_hazardous_host(armed, hash(33), InhibitCause::ImplementationFailed),
        )),
        "plan-transition-drops-old-command" => state_outcome(Ok(recover_after_host_change(
            armed,
            HostLifecycleChange::PlanReplacement,
        ))),
        "rollback-retains-inhibit-latch" => retained(armed, HostLifecycleChange::Rollback),
        "reboot-retains-inhibit-latch" => retained(armed, HostLifecycleChange::Reboot),
        "firmware-update-retains-inhibit-latch" => {
            retained(armed, HostLifecycleChange::FirmwareUpdate)
        }
        "reconnect-retains-inhibit-latch" => retained(armed, HostLifecycleChange::Reconnect),
        "realm-recovery-retains-inhibit-latch" => {
            retained(armed, HostLifecycleChange::RealmRecovery)
        }
        "stop-request-needs-no-clear-authority" => state_outcome(Ok(inhibit_hazardous_host(
            armed,
            hash(34),
            InhibitCause::StopRequest,
        ))),
        "remote-plan-clear-without-ceremony-fails" => {
            let inhibited = inhibit_hazardous_host(armed, hash(35), InhibitCause::StopRequest);
            state_outcome(clear_inhibit(
                profile,
                inhibited,
                InhibitClearRequest {
                    profile_identity: profile.identity,
                    host: Id("host.effect"),
                    latch_identity: inhibited.latch_identity,
                    latch_generation: inhibited.latch_generation,
                    subject: subject(),
                    physical_presence_receipt: Some(hash(80)),
                    proof: clear_proof(profile, false),
                },
                20,
            ))
        }
        "clear-without-required-physical-presence-fails" => clear_case(profile, armed, false),
        "approved-clear-returns-safe-disarmed-not-armed" => clear_case(profile, armed, true),
        "safe-state-evidence-has-exact-identity" => {
            let mut record = HazardEvidenceRecord {
                identity: ZERO,
                sequence: 1,
                predecessor: None,
                profile_identity: profile.identity,
                host: Id("host.effect"),
                plan: hash(21),
                epoch: 7,
                kind: HazardEvidenceKind::SafeStateEntered,
                time_basis: profile.time_basis,
                observed_at_tick: 20,
                receipt: hash(71),
            };
            record.identity = record.computed_semantic_hash().unwrap();
            outcome(validate_hazard_evidence(profile, record))
        }
        "evidence-capacity-is-bounded" => outcome(validate_hazard_evidence(
            profile,
            HazardEvidenceRecord {
                identity: ZERO,
                sequence: 17,
                predecessor: Some(hash(70)),
                profile_identity: profile.identity,
                host: Id("host.effect"),
                plan: hash(21),
                epoch: 7,
                kind: HazardEvidenceKind::SafeStateEntered,
                time_basis: profile.time_basis,
                observed_at_tick: 20,
                receipt: hash(71),
            },
        )),
        other => panic!("unknown inhibit fixture case `{other}`"),
    }
}

fn retained(armed: HazardControlState, change: HostLifecycleChange) -> Value {
    let inhibited = inhibit_hazardous_host(armed, hash(40), InhibitCause::EvidenceFailed);
    let recovered = recover_after_host_change(inhibited, change);
    assert_eq!(recovered.latch_identity, inhibited.latch_identity);
    assert_eq!(recovered.latch_generation, inhibited.latch_generation);
    state_outcome(Ok(recovered))
}

fn clear_case(
    profile: HazardousHostProfile<'static>,
    armed: HazardControlState,
    physical_presence: bool,
) -> Value {
    let inhibited = inhibit_hazardous_host(armed, hash(41), InhibitCause::StopRequest);
    state_outcome(clear_inhibit(
        profile,
        inhibited,
        InhibitClearRequest {
            profile_identity: profile.identity,
            host: Id("host.effect"),
            latch_identity: inhibited.latch_identity,
            latch_generation: inhibited.latch_generation,
            subject: subject(),
            physical_presence_receipt: physical_presence.then_some(hash(80)),
            proof: clear_proof(profile, true),
        },
        20,
    ))
}

#[test]
fn every_inhibit_fixture_is_independently_dispatched() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.inhibit-plane");
    assert!(
        fixture["claim_boundary"]
            .as_str()
            .unwrap()
            .contains("not a domain safety case")
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 29);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(run_case(id), case["expected"], "fixture case {id}");
    }
}
