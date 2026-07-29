use conduit_core::{
    ArtifactDigest, Id, InstancePath, OptionalCharacteristicChange, PLAN_TRANSITION_SCHEMA_VERSION,
    PinnedDescriptor, PlanEpoch, PlanResourceBudget, ReplacementSupport, ReplayGapPolicy,
    SemanticHash, TransitionAdmissionProofs, TransitionBudget, TransitionContract,
    TransitionController, TransitionDrainObservation, TransitionEvidenceKind,
    TransitionGuaranteeFloor, TransitionKind, TransitionLevel, TransitionModeDecision,
    TransitionPhase, TransitionReason, TransitionRecoveryPolicy, TransitionReplayContract,
    TransitionReplayObservation, TransitionStateContract, TransitionUsage,
    validate_replacement_support, validate_transition_contract,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TRANSITION_FIXTURE: &str = include_str!("../../../conformance/c5/plan-transitions-v1.json");

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

const fn resources(memory: u64, timers: u16, transports: u16) -> PlanResourceBudget {
    PlanResourceBudget {
        memory_bytes: memory,
        storage_bytes: 0,
        cpu_units: 1,
        timers,
        transports,
        checkpoints: 1,
        evidence_bytes: 1024,
    }
}

const FLOOR: TransitionGuaranteeFloor = TransitionGuaranteeFloor {
    semantic_contract: hash(20),
    authority: hash(21),
    sensitivity: hash(22),
    delivery: hash(23),
    memory: hash(24),
    security: hash(25),
    committedness: hash(26),
};

const CHANGES: [OptionalCharacteristicChange<'static>; 1] = [OptionalCharacteristicChange {
    characteristic: pin("fixture/quality", 30),
    old_value: hash(31),
    new_value: hash(32),
    weakened: true,
}];

fn contract() -> TransitionContract<'static> {
    let mut value = TransitionContract {
        schema_version: PLAN_TRANSITION_SCHEMA_VERSION,
        identity: ZERO,
        old: PlanEpoch {
            plan: hash(1),
            epoch: 4,
        },
        candidate: PlanEpoch {
            plan: hash(2),
            epoch: 5,
        },
        stable_subject: InstancePath::new("root/service").unwrap(),
        old_implementation: pin("fixture/old", 3),
        candidate_implementation: pin("fixture/candidate", 4),
        old_artifact: ArtifactDigest::from_bytes([5; 32]),
        candidate_artifact: ArtifactDigest::from_bytes([6; 32]),
        kind: TransitionKind::ImplementationReplacement,
        level: TransitionLevel::Stateful,
        boundary: pin("fixture/request-boundary", 7),
        state: Some(TransitionStateContract {
            descriptor: pin("fixture/state", 8),
            maximum_export_bytes: 32,
            maximum_import_bytes: 32,
            sensitivity: pin("fixture/private", 9),
            authority: pin("fixture/state-authority", 10),
        }),
        replay: Some(TransitionReplayContract {
            stream: pin("fixture/input-stream", 11),
            stream_epoch: 3,
            first_cursor: 9,
            maximum_items: 4,
            maximum_bytes: 64,
            duplicates_permitted: true,
            gap_policy: ReplayGapPolicy::Rollback,
        }),
        discontinuity_permitted: false,
        required_floor: FLOOR,
        candidate_floor: FLOOR,
        optional_changes: &CHANGES,
        mode_decision: None,
        budget: TransitionBudget {
            old: resources(100, 1, 1),
            candidate: resources(120, 2, 1),
            rollback: resources(80, 1, 1),
            overlap_reserved: PlanResourceBudget {
                memory_bytes: 300,
                storage_bytes: 0,
                cpu_units: 3,
                timers: 4,
                transports: 3,
                checkpoints: 3,
                evidence_bytes: 3072,
            },
            maximum_in_flight_values: 4,
            maximum_pending_operations: 2,
            maximum_replay_items: 4,
            maximum_replay_bytes: 64,
            maximum_state_bytes: 64,
            maximum_evidence_records: 16,
            maximum_ticks: 100,
        },
        recovery: TransitionRecoveryPolicy {
            maximum_attempts: 2,
            cooldown_ticks: 10,
            hysteresis_ticks: 5,
        },
    };
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    value
}

const fn proofs() -> TransitionAdmissionProofs {
    TransitionAdmissionProofs {
        request: hash(40),
        decision: hash(41),
        authorization: hash(42),
        candidate_resolution: hash(43),
        persistent_budget_status: hash(44),
        hazard_closure: hash(45),
        inhibit_decision: hash(46),
    }
}

fn usage() -> TransitionUsage {
    TransitionUsage {
        overlap: contract().budget.overlap_reserved,
        in_flight_values: 2,
        pending_operations: 1,
        drained_values: 0,
        rejected_values: 0,
        lost_values: 0,
        completed_operations: 0,
        cancelled_operations: 0,
        replay_items: 0,
        replay_bytes: 0,
        duplicate_replay_items: 0,
        state_bytes: 0,
    }
}

#[test]
fn stateful_transition_switches_authority_only_at_commit() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    controller.barrier(contract.boundary, 3).unwrap();
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 0,
                remaining_operations: 0,
                drained_values: 2,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 1,
                cancelled_operations: 0,
            },
            4,
        )
        .unwrap();
    controller
        .transfer_state(contract.state.unwrap().descriptor, 32, 32, 5)
        .unwrap();
    controller
        .replayed(
            TransitionReplayObservation {
                stream: contract.replay.unwrap().stream,
                stream_epoch: 3,
                first_cursor: 9,
                items: 4,
                bytes: 64,
                duplicate_items: 0,
                gap: false,
            },
            6,
        )
        .unwrap();
    controller.rebind(7).unwrap();
    assert_eq!(controller.active_epoch(), contract.old);
    controller.commit(8).unwrap();
    assert_eq!(controller.active_epoch(), contract.candidate);
    controller.retire_old(9).unwrap();
    controller.complete(10).unwrap();
    assert_eq!(controller.phase(), TransitionPhase::Completed);
    assert_eq!(controller.evidence().len(), 11);
    assert_eq!(
        controller.evidence().last().unwrap().unwrap().kind,
        TransitionEvidenceKind::Completed
    );
}

#[test]
fn precommit_failure_rolls_back_without_candidate_authority() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    controller.rollback(hash(90), 3).unwrap();
    assert_eq!(controller.active_epoch(), contract.old);
    assert_eq!(controller.phase(), TransitionPhase::RolledBack);
}

#[test]
fn retry_preserves_attempt_limit_and_cooldown() {
    let mut contract = contract();
    contract.recovery.maximum_attempts = 2;
    contract.identity = contract.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.rollback(hash(90), 2).unwrap();
    assert_eq!(controller.retry(10), Err(TransitionReason::CooldownActive));
    controller.retry(11).unwrap();
    controller.reserve(proofs(), usage(), 12).unwrap();
    controller.rollback(hash(91), 13).unwrap();
    assert_eq!(controller.retry(30), Err(TransitionReason::AttemptLimit));
}

#[test]
fn guarantee_floor_cannot_be_weakened() {
    let mut value = contract();
    value.candidate_floor.security = hash(99);
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    assert_eq!(
        validate_transition_contract(&value, &mut [ZERO; 2]),
        Err(TransitionReason::GuaranteeWeakened)
    );
}

#[test]
fn overlap_reserve_is_exact_not_a_smaller_or_surplus_claim() {
    let mut value = contract();
    value.budget.overlap_reserved.memory_bytes += 1;
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    assert_eq!(
        validate_transition_contract(&value, &mut [ZERO; 2]),
        Err(TransitionReason::OverlapExceeded)
    );
}

#[test]
fn stale_active_epoch_is_rejected() {
    let value = contract();
    assert_eq!(
        TransitionController::<16>::new(
            value,
            PlanEpoch {
                plan: hash(1),
                epoch: 3
            },
            0,
            &mut [ZERO; 2]
        ),
        Err(TransitionReason::StaleEpoch)
    );
}

#[test]
fn boundary_mismatch_does_not_advance_phase() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    assert_eq!(
        controller.barrier(pin("fixture/segment-boundary", 77), 3),
        Err(TransitionReason::BoundaryMismatch)
    );
    assert_eq!(controller.phase(), TransitionPhase::Prepared);
}

#[test]
fn state_and_replay_bounds_fail_before_phase_mutation() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    controller.barrier(contract.boundary, 3).unwrap();
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 0,
                remaining_operations: 0,
                drained_values: 2,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 1,
                cancelled_operations: 0,
            },
            4,
        )
        .unwrap();
    assert_eq!(
        controller.transfer_state(contract.state.unwrap().descriptor, 33, 32, 5),
        Err(TransitionReason::StateContractMismatch)
    );
    assert_eq!(controller.phase(), TransitionPhase::Draining);
    assert_eq!(
        controller.replayed(
            TransitionReplayObservation {
                stream: contract.replay.unwrap().stream,
                stream_epoch: 3,
                first_cursor: 9,
                items: 5,
                bytes: 64,
                duplicate_items: 0,
                gap: false,
            },
            5,
        ),
        Err(TransitionReason::ReplayContractMismatch)
    );
    assert_eq!(controller.phase(), TransitionPhase::Draining);
}

#[test]
fn replay_gap_obeys_declared_policy() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    controller.barrier(contract.boundary, 3).unwrap();
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 0,
                remaining_operations: 0,
                drained_values: 2,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 1,
                cancelled_operations: 0,
            },
            4,
        )
        .unwrap();
    assert_eq!(
        controller.replayed(
            TransitionReplayObservation {
                stream: contract.replay.unwrap().stream,
                stream_epoch: 3,
                first_cursor: 9,
                items: 1,
                bytes: 8,
                duplicate_items: 0,
                gap: true,
            },
            5,
        ),
        Err(TransitionReason::ReplayGap)
    );
}

#[test]
fn evidence_exhaustion_is_atomic() {
    let mut contract = contract();
    contract.budget.maximum_evidence_records = 2;
    contract.identity = contract.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    let mut controller =
        TransitionController::<2>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    assert_eq!(
        controller.prepared(2),
        Err(TransitionReason::EvidenceExhausted)
    );
    assert_eq!(controller.phase(), TransitionPhase::Reserved);
}

#[test]
fn deadline_is_enforced_before_mutation() {
    let mut contract = contract();
    contract.budget.maximum_ticks = 2;
    contract.identity = contract.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 10, &mut [ZERO; 2]).unwrap();
    assert_eq!(
        controller.reserve(proofs(), usage(), 13),
        Err(TransitionReason::DeadlineExceeded)
    );
    assert_eq!(controller.phase(), TransitionPhase::Requested);
}

#[test]
fn remaining_work_blocks_rebind_until_every_item_has_a_disposition() {
    let contract = contract();
    let mut controller =
        TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
    controller.reserve(proofs(), usage(), 1).unwrap();
    controller.prepared(2).unwrap();
    controller.barrier(contract.boundary, 3).unwrap();
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 1,
                remaining_operations: 1,
                drained_values: 1,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
            },
            4,
        )
        .unwrap();
    assert_eq!(
        controller.preflight_rebind(5),
        Err(TransitionReason::IllegalPhase)
    );
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 0,
                remaining_operations: 0,
                drained_values: 2,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 1,
                cancelled_operations: 0,
            },
            5,
        )
        .unwrap();
    controller
        .transfer_state(contract.state.unwrap().descriptor, 32, 32, 6)
        .unwrap();
    controller.preflight_rebind(7).unwrap();
}

#[test]
fn requested_replacement_level_is_checked_against_exact_manifest_capability() {
    let contract = contract();
    assert_eq!(
        validate_replacement_support(ReplacementSupport::Cold, contract),
        Err(TransitionReason::StateContractMismatch)
    );
    assert_eq!(
        validate_replacement_support(
            ReplacementSupport::Stateful {
                state_contract: pin("fixture/other-state", 99),
                maximum_export_bytes: 32,
                maximum_import_bytes: 32,
                maximum_ticks: 100,
            },
            contract,
        ),
        Err(TransitionReason::StateContractMismatch)
    );
    validate_replacement_support(
        ReplacementSupport::Stateful {
            state_contract: contract.state.unwrap().descriptor,
            maximum_export_bytes: 32,
            maximum_import_bytes: 32,
            maximum_ticks: 100,
        },
        contract,
    )
    .unwrap();
}

#[test]
fn graph_mode_degradation_requires_an_exact_authorized_mode_decision() {
    let mut value = contract();
    value.kind = TransitionKind::PlanModeTransition;
    value.mode_decision = Some(TransitionModeDecision {
        policy: pin("fixture/degradation-policy", 170),
        selected_mode: pin("fixture/mode-without-timestamps", 171),
        minimum_mode: pin("fixture/minimum-mode", 172),
        trigger: pin("fixture/gpu-unavailable", 173),
        authorization: hash(174),
    });
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    validate_transition_contract(&value, &mut [ZERO; 2]).unwrap();

    value.mode_decision = None;
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    assert_eq!(
        validate_transition_contract(&value, &mut [ZERO; 2]),
        Err(TransitionReason::InvalidContract)
    );
}

#[test]
fn cold_replacement_is_distinct_and_transfers_no_private_state() {
    let mut value = contract();
    value.level = TransitionLevel::Cold;
    value.state = None;
    value.replay = None;
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    validate_replacement_support(ReplacementSupport::Cold, value).unwrap();
    let mut controller =
        TransitionController::<16>::new(value, value.old, 0, &mut [ZERO; 2]).unwrap();
    let mut empty = usage();
    empty.in_flight_values = 0;
    empty.pending_operations = 0;
    controller.reserve(proofs(), empty, 1).unwrap();
    controller.prepared(2).unwrap();
    controller.barrier(value.boundary, 3).unwrap();
    controller
        .drained(
            TransitionDrainObservation {
                remaining_values: 0,
                remaining_operations: 0,
                drained_values: 0,
                rejected_values: 0,
                lost_values: 0,
                completed_operations: 0,
                cancelled_operations: 0,
            },
            4,
        )
        .unwrap();
    controller.rebind(5).unwrap();
    controller.commit(6).unwrap();
    controller.retire_old(7).unwrap();
    controller.complete(8).unwrap();
}

fn guarantee_floor_field_is_rejected(field: &str) {
    let mut value = contract();
    match field {
        "security" => value.candidate_floor.security = hash(99),
        "sensitivity" => value.candidate_floor.sensitivity = hash(99),
        "delivery" => value.candidate_floor.delivery = hash(99),
        "committedness" => value.candidate_floor.committedness = hash(99),
        "memory" => value.candidate_floor.memory = hash(99),
        other => panic!("unknown guarantee field {other}"),
    }
    value.identity = value.computed_semantic_hash(&mut [ZERO; 2]).unwrap();
    assert_eq!(
        validate_transition_contract(&value, &mut [ZERO; 2]),
        Err(TransitionReason::GuaranteeWeakened)
    );
}

fn evidence_replay_equivalence() {
    fn execute() -> [Option<conduit_core::TransitionEvidence<'static>>; 16] {
        let contract = contract();
        let mut controller =
            TransitionController::<16>::new(contract, contract.old, 0, &mut [ZERO; 2]).unwrap();
        controller.reserve(proofs(), usage(), 1).unwrap();
        controller.prepared(2).unwrap();
        controller.barrier(contract.boundary, 3).unwrap();
        controller
            .drained(
                TransitionDrainObservation {
                    remaining_values: 0,
                    remaining_operations: 0,
                    drained_values: 2,
                    rejected_values: 0,
                    lost_values: 0,
                    completed_operations: 1,
                    cancelled_operations: 0,
                },
                4,
            )
            .unwrap();
        controller
            .transfer_state(contract.state.unwrap().descriptor, 32, 32, 5)
            .unwrap();
        controller
            .replayed(
                TransitionReplayObservation {
                    stream: contract.replay.unwrap().stream,
                    stream_epoch: 3,
                    first_cursor: 9,
                    items: 2,
                    bytes: 24,
                    duplicate_items: 0,
                    gap: false,
                },
                6,
            )
            .unwrap();
        let mut evidence = [None; 16];
        evidence[..controller.evidence().len()].copy_from_slice(controller.evidence());
        evidence
    }
    assert_eq!(execute(), execute());
}

#[test]
fn every_portable_transition_fixture_case_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(TRANSITION_FIXTURE).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        if case["runner"] != "transition-core" {
            continue;
        }
        match case["id"].as_str().unwrap() {
            "immutable-plan-epochs" => stateful_transition_switches_authority_only_at_commit(),
            "cold-replacement-no-private-state" => {
                cold_replacement_is_distinct_and_transfers_no_private_state();
            }
            "stateful-compatible-transfer" | "stateful-incompatible-transfer" => {
                requested_replacement_level_is_checked_against_exact_manifest_capability();
            }
            "output-full-input-in-flight" => {
                remaining_work_blocks_rebind_until_every_item_has_a_disposition();
            }
            "overlap-budget-exceeded" => {
                overlap_reserve_is_exact_not_a_smaller_or_surplus_claim();
            }
            "permitted-quality-degradation" => {
                validate_transition_contract(&contract(), &mut [ZERO; 2]).unwrap();
            }
            "explicit-optional-feature-loss" => {
                graph_mode_degradation_requires_an_exact_authorized_mode_decision();
            }
            "https-to-http-rejected" => guarantee_floor_field_is_rejected("security"),
            "secret-to-public-rejected" => guarantee_floor_field_is_rejected("sensitivity"),
            "lossless-to-lossy-rejected" => guarantee_floor_field_is_rejected("delivery"),
            "committed-to-partial-rejected" => {
                guarantee_floor_field_is_rejected("committedness");
            }
            "bounded-to-unbounded-rejected" => guarantee_floor_field_is_rejected("memory"),
            "replay-gap-obeys-policy" => replay_gap_obeys_declared_policy(),
            "cooldown-hysteresis-attempt-limit" => retry_preserves_attempt_limit_and_cooldown(),
            "evidence-exhaustion-before-mutation" => evidence_exhaustion_is_atomic(),
            "transition-evidence-replay-equivalence" => evidence_replay_equivalence(),
            other => panic!("unhandled portable transition fixture {other}"),
        }
    }
}
