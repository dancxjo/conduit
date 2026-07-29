use conduit_core::{
    EXECUTION_PLAN_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION_V15, Id, InstancePath,
    PinnedDescriptor, PlanInstancePool, PlanPoolRuntime, PlanResourceBudget,
    PoolAdmissionDisposition, PoolAdmissionFacts, PoolAdmissionPolicy, PoolCleanupPolicy,
    PoolContract, PoolController, PoolError, PoolFailureDisposition, PoolGeneration,
    PoolGenerationReservation, PoolReason, PoolReservationProfile, PoolSlotState,
    PoolSupervisionPolicy, PoolWorkIdentity, SemanticHash, select_fair_pool,
};
use conduit_runtime::{HostedPoolError, instantiate_plan_pool};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    expected: Value,
}

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn profile(multiplier: u16) -> PoolReservationProfile {
    PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 128 * u64::from(multiplier),
            storage_bytes: 16 * u64::from(multiplier),
            cpu_units: u32::from(multiplier),
            timers: multiplier,
            transports: multiplier,
            checkpoints: multiplier,
            evidence_bytes: 64 * u64::from(multiplier),
        },
        child_nodes: 2 * multiplier,
        child_cords: multiplier,
        state_bytes: 32 * u64::from(multiplier),
        scheduler_slots: 3 * multiplier,
        host_operations: multiplier,
        cancellation_scopes: 2 * multiplier,
    }
}

fn queued_profile(multiplier: u16) -> PoolReservationProfile {
    PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 32 * u64::from(multiplier),
            evidence_bytes: 32 * u64::from(multiplier),
            ..PlanResourceBudget::ZERO
        },
        state_bytes: 16 * u64::from(multiplier),
        scheduler_slots: multiplier,
        cancellation_scopes: multiplier,
        ..PoolReservationProfile::default()
    }
}

fn contract<'a>(
    maximum_live: u16,
    maximum_queued: u16,
    admission: PoolAdmissionPolicy,
    supervision: PoolSupervisionPolicy<'a>,
    cleanup: PoolCleanupPolicy,
    evidence: u16,
) -> PoolContract<'a> {
    let generation = profile(maximum_live + 2);
    let queued = queued_profile(maximum_queued);
    PoolContract {
        pool: InstancePath::new("root/pool.workers").unwrap(),
        template_hash: hash(1),
        implementation_set_hash: hash(5),
        maximum_live,
        maximum_queued,
        admission,
        supervision,
        cleanup,
        deadline_ticks: 100,
        idle_timeout_ticks: 20,
        cleanup_ticks: 5,
        reservation: profile(1),
        total_reservation: generation.checked_add(queued).unwrap(),
        maximum_evidence_events: evidence,
    }
}

fn controller<'a>(
    maximum_live: u16,
    maximum_queued: u16,
    admission: PoolAdmissionPolicy,
    supervision: PoolSupervisionPolicy<'a>,
    cleanup: PoolCleanupPolicy,
) -> PoolController<'a, 8, 256> {
    PoolController::new(
        contract(
            maximum_live,
            maximum_queued,
            admission,
            supervision,
            cleanup,
            256,
        ),
        PoolGeneration {
            plan: hash(2),
            epoch: 7,
            generation: 1,
            template_hash: hash(1),
        },
    )
    .unwrap()
}

fn work(byte: u8) -> PoolWorkIdentity {
    PoolWorkIdentity {
        request: hash(byte),
        work_unit: hash(byte.wrapping_add(40)),
        correlation: hash(byte.wrapping_add(80)),
    }
}

fn facts() -> PoolAdmissionFacts {
    PoolAdmissionFacts {
        authority_granted: true,
        sensitivity_allowed: true,
        template_hash: hash(1),
        implementation_set_hash: hash(5),
        available: profile(1),
    }
}

fn started(pool: &mut PoolController<'_, 8, 256>, byte: u8, tick: u64) -> u16 {
    let PoolAdmissionDisposition::Started { slot } = pool.offer(work(byte), facts(), tick).unwrap()
    else {
        panic!("fixture expected start");
    };
    slot
}

fn capacity_result(value: PoolAdmissionDisposition) -> Value {
    match value {
        PoolAdmissionDisposition::Blocked => json!({"outcome":"blocked","queued":0}),
        PoolAdmissionDisposition::Rejected(reason) => {
            json!({"outcome":"rejected","reason":reason.as_str()})
        }
        PoolAdmissionDisposition::Failed(reason) => {
            json!({"outcome":"failed","reason":reason.as_str()})
        }
        PoolAdmissionDisposition::Queued { .. } => json!({"outcome":"queued","queued":1}),
        PoolAdmissionDisposition::Started { .. } => json!({"outcome":"started"}),
    }
}

fn pin(name: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(name),
        schema_version: 1,
        semantic_hash: hash(byte),
    }
}

fn plan_pool(runtime: bool) -> PlanInstancePool<'static> {
    let pool = InstancePath::new("root/pool.workers").unwrap();
    let per_instance = profile(1);
    let generation = profile(4);
    PlanInstancePool {
        instance: pool,
        template_hash: hash(1),
        derived_identity_hash: hash(2),
        maximum_live: 2,
        maximum_queued: 0,
        admission_policy: pin("fixture/admission", 3),
        supervision_policy: pin("fixture/supervision", 4),
        per_instance_budget: per_instance.resources,
        authority_grants: &[],
        maximum_instance_ticks: 100,
        implementation_set_hash: hash(5),
        correlation_slots: 2,
        worst_case_budget: generation.resources,
        child_nodes: per_instance.child_nodes,
        child_cords: per_instance.child_cords,
        runtime: runtime.then_some(PlanPoolRuntime {
            contract: contract(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
                64,
            ),
            queued_reservation: PoolReservationProfile::default(),
            generation_reservation: PoolGenerationReservation {
                old_maximum_live: 2,
                candidate_maximum_live: 1,
                rollback_maximum_live: 1,
                reserved_slots: 4,
                per_instance,
                reserved_resources: generation,
            },
        }),
    }
}

fn execute(id: &str) -> Value {
    match id {
        "maximum-one" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            let second = pool.offer(work(2), facts(), 0).unwrap();
            json!({"live":pool.population().live,"second":match second {
                PoolAdmissionDisposition::Rejected(reason) => reason.as_str(),
                _ => "wrong",
            }})
        }
        "maximum-many" => {
            let mut pool = controller(
                4,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            for byte in 1..=4 {
                started(&mut pool, byte, 0);
            }
            json!({"live":pool.population().live,"queued":pool.population().queued})
        }
        "admission-reject" | "admission-block" | "admission-fail" => {
            let admission = match id {
                "admission-reject" => PoolAdmissionPolicy::Reject,
                "admission-block" => PoolAdmissionPolicy::Block,
                _ => PoolAdmissionPolicy::Fail,
            };
            let mut pool = controller(
                1,
                0,
                admission,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            capacity_result(pool.offer(work(2), facts(), 0).unwrap())
        }
        "admission-queue-bounded" | "queue-full" => {
            let mut pool = controller(
                1,
                1,
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            let queued = pool.offer(work(2), facts(), 0).unwrap();
            if id == "admission-queue-bounded" {
                capacity_result(queued)
            } else {
                capacity_result(pool.offer(work(3), facts(), 0).unwrap())
            }
        }
        "queued-cancellation" => {
            let mut pool = controller(
                1,
                1,
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            let PoolAdmissionDisposition::Queued { slot } =
                pool.offer(work(2), facts(), 0).unwrap()
            else {
                unreachable!()
            };
            pool.cancel(slot, hash(90), 1).unwrap();
            json!({"state":"cancelled","cause":pool.evidence().last().unwrap().cause.is_some()})
        }
        "identity-order-independent" => {
            let mut left = controller(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            let mut right = controller(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut left, 1, 0);
            started(&mut left, 2, 0);
            started(&mut right, 2, 0);
            started(&mut right, 1, 0);
            let left_identity = left
                .slots()
                .iter()
                .find(|slot| slot.request == hash(1))
                .unwrap()
                .identity;
            let right_identity = right
                .slots()
                .iter()
                .find(|slot| slot.request == hash(1))
                .unwrap()
                .identity;
            json!({"same":left_identity == right_identity,"attempt":left_identity.attempt})
        }
        "cross-pool-identity-distinct" => {
            let left_contract = contract(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
                256,
            );
            let right_contract = PoolContract {
                pool: InstancePath::new("root/pool.other").unwrap(),
                ..left_contract
            };
            let generation = PoolGeneration {
                plan: hash(2),
                epoch: 7,
                generation: 1,
                template_hash: hash(1),
            };
            let mut left = PoolController::<8, 256>::new(left_contract, generation).unwrap();
            let mut right = PoolController::<8, 256>::new(right_contract, generation).unwrap();
            started(&mut left, 1, 0);
            started(&mut right, 1, 0);
            json!({"distinct":left.slots()[0].identity.instance != right.slots()[0].identity.instance})
        }
        "isolate-failure" | "fail-together" => {
            let supervision = if id == "isolate-failure" {
                PoolSupervisionPolicy::Isolate
            } else {
                PoolSupervisionPolicy::FailTogether
            };
            let mut pool = controller(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                supervision,
                PoolCleanupPolicy::Abort,
            );
            let first = started(&mut pool, 1, 0);
            let second = started(&mut pool, 2, 0);
            let disposition = pool.fail(first, hash(90), 1).unwrap();
            if id == "isolate-failure" {
                json!({"affected":1,"other":match pool.slots()[usize::from(second)].state {
                    PoolSlotState::Reserved => "reserved",
                    _ => "wrong",
                }})
            } else {
                json!({"cleanup":pool.population().cleanup,"disposition":match disposition {
                    PoolFailureDisposition::FailPool => "fail-pool",
                    _ => "wrong",
                }})
            }
        }
        "restart-success" | "restart-exhaustion" | "restart-backoff" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::RestartBounded {
                    maximum_attempts: 2,
                    backoff_ticks: 5,
                },
                PoolCleanupPolicy::Abort,
            );
            let slot = started(&mut pool, 1, 0);
            pool.mark_running(slot, 0).unwrap();
            pool.fail(slot, hash(90), 1).unwrap();
            let before = pool.slots()[usize::from(slot)].state;
            pool.tick(6).unwrap();
            if id == "restart-success" {
                json!({"attempt":pool.slots()[usize::from(slot)].identity.attempt,"state":"reserved"})
            } else if id == "restart-backoff" {
                json!({"before":match before {
                    PoolSlotState::RestartBackoff => "restart-backoff",
                    _ => "wrong",
                },"at":"reserved"})
            } else {
                pool.mark_running(slot, 6).unwrap();
                let disposition = pool.fail(slot, hash(91), 7).unwrap();
                json!({"disposition":match disposition {
                    PoolFailureDisposition::RestartExhausted => "restart-exhausted",
                    _ => "wrong",
                },"state":"cleanup"})
            }
        }
        "explicit-fallback" | "escalation" => {
            let supervision = if id == "explicit-fallback" {
                PoolSupervisionPolicy::Fallback {
                    target: InstancePath::new("root/fallback").unwrap(),
                }
            } else {
                PoolSupervisionPolicy::Escalate
            };
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                supervision,
                PoolCleanupPolicy::Abort,
            );
            let slot = started(&mut pool, 1, 0);
            let disposition = pool.fail(slot, hash(90), 1).unwrap();
            match disposition {
                PoolFailureDisposition::Fallback(target) => {
                    json!({"disposition":"fallback","target":target.as_str()})
                }
                PoolFailureDisposition::Escalate => {
                    json!({"disposition":"escalate","state":"cleanup"})
                }
                _ => json!({"disposition":"wrong"}),
            }
        }
        "idle-timeout" | "deadline-expiry" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            let slot = started(&mut pool, 1, 0);
            pool.mark_running(slot, 0).unwrap();
            let tick = if id == "idle-timeout" { 20 } else { 100 };
            pool.tick(tick).unwrap();
            let reason = pool
                .evidence()
                .iter()
                .rev()
                .find(|event| event.from != event.to)
                .unwrap()
                .reason;
            json!({"reason":reason.as_str(),"state":"cleanup"})
        }
        "cleanup-drain" | "cleanup-abort" => {
            let cleanup = if id == "cleanup-drain" {
                PoolCleanupPolicy::Drain
            } else {
                PoolCleanupPolicy::Abort
            };
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                cleanup,
            );
            let slot = started(&mut pool, 1, 0);
            pool.complete(slot, 1).unwrap();
            let cleanup_reason = pool.evidence().last().unwrap().reason.as_str();
            pool.tick(6).unwrap();
            let terminal = match pool.slots()[usize::from(slot)].state {
                PoolSlotState::Succeeded => "succeeded",
                PoolSlotState::Failed => "failed",
                _ => "wrong",
            };
            json!({"terminal":terminal,"cleanup":cleanup_reason})
        }
        "authority-denial" | "resource-denial" | "implementation-set-denial" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            let denied = match id {
                "authority-denial" => PoolAdmissionFacts {
                    authority_granted: false,
                    ..facts()
                },
                "resource-denial" => PoolAdmissionFacts {
                    available: PoolReservationProfile::default(),
                    ..facts()
                },
                _ => PoolAdmissionFacts {
                    implementation_set_hash: hash(99),
                    ..facts()
                },
            };
            let result = pool.offer(work(1), denied, 0).unwrap();
            let reason = match result {
                PoolAdmissionDisposition::Rejected(reason) => reason.as_str(),
                _ => "wrong",
            };
            json!({"live":pool.population().live,"reason":reason})
        }
        "pressure-isolation" => {
            let mut pool = controller(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            let first = started(&mut pool, 1, 0);
            let second = started(&mut pool, 2, 0);
            pool.mark_running(first, 0).unwrap();
            pool.mark_running(second, 0).unwrap();
            pool.observe_pressure(first, true, Some(hash(91)), 1)
                .unwrap();
            let mut excess = profile(1);
            excess.host_operations += 1;
            pool.observe_usage(first, excess, 2).unwrap();
            let pressure = pool
                .evidence()
                .iter()
                .find(|event| event.reason == PoolReason::Loss)
                .unwrap();
            json!({"failed":"cleanup","pressure":"loss","cause":pressure.cause == Some(hash(91)),"other":match pool.slots()[usize::from(second)].state {
                PoolSlotState::Running => "running",
                _ => "wrong",
            }})
        }
        "checkpoint-compatible" | "checkpoint-incompatible" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Drain,
            );
            let slot = started(&mut pool, 1, 0);
            pool.mark_running(slot, 0).unwrap();
            let accepted = pool
                .checkpoint(
                    slot,
                    if id == "checkpoint-compatible" {
                        hash(1)
                    } else {
                        hash(9)
                    },
                    1,
                )
                .unwrap();
            json!({"accepted":accepted,"state":match pool.slots()[usize::from(slot)].state {
                PoolSlotState::Checkpointing => "checkpointing",
                PoolSlotState::Running => "running",
                _ => "wrong",
            }})
        }
        "pool-cancellation-causality" => {
            let mut pool = controller(
                2,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            started(&mut pool, 2, 0);
            let cause = hash(90);
            pool.begin_generation_drain(cause, 1).unwrap();
            let caused = pool
                .evidence()
                .iter()
                .filter(|event| event.cause == Some(cause))
                .count();
            json!({"caused":caused,"cause_exact":pool.evidence().iter()
                .filter(|event| event.cause.is_some())
                .all(|event| event.cause == Some(cause))})
        }
        "generation-overlap" | "generation-overlap-denied" => {
            let reservation = PoolGenerationReservation {
                old_maximum_live: 2,
                candidate_maximum_live: 1,
                rollback_maximum_live: 1,
                reserved_slots: if id == "generation-overlap" { 4 } else { 3 },
                per_instance: profile(1),
                reserved_resources: profile(4),
            };
            match reservation.validate() {
                Ok(()) => json!({"valid":true,"reserved_slots":reservation.reserved_slots}),
                Err(error) => json!({"code":error.code()}),
            }
        }
        "generation-drain" => {
            let mut pool = controller(
                2,
                1,
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            started(&mut pool, 2, 0);
            pool.offer(work(3), facts(), 0).unwrap();
            pool.begin_generation_drain(hash(90), 1).unwrap();
            json!({"retiring":pool.population().retiring,"queued":pool.population().queued})
        }
        "generation-rollback" => {
            let mut pool = controller(
                2,
                1,
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            started(&mut pool, 2, 0);
            pool.offer(work(3), facts(), 0).unwrap();
            pool.rollback_generation(hash(90), 1).unwrap();
            json!({"cleanup":pool.population().cleanup,"cancelled":pool.slots().iter()
                .filter(|slot| slot.state == PoolSlotState::Cancelled).count()})
        }
        "malicious-profile" => {
            let mut pool = controller(
                1,
                0,
                PoolAdmissionPolicy::Reject,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            let slot = started(&mut pool, 1, 0);
            pool.mark_running(slot, 0).unwrap();
            let mut excess = profile(1);
            excess.scheduler_slots += 1;
            let accepted = pool.observe_usage(slot, excess, 1).unwrap();
            json!({"accepted":accepted,"reason":pool.evidence().iter()
                .find(|event| event.reason == PoolReason::ForeignProfileExceeded)
                .unwrap().reason.as_str()})
        }
        "long-run-population" => {
            let mut pool = controller(
                2,
                2,
                PoolAdmissionPolicy::QueueBounded,
                PoolSupervisionPolicy::Isolate,
                PoolCleanupPolicy::Abort,
            );
            started(&mut pool, 1, 0);
            started(&mut pool, 2, 0);
            pool.mark_running(0, 0).unwrap();
            pool.mark_running(1, 0).unwrap();
            pool.offer(work(3), facts(), 0).unwrap();
            pool.offer(work(4), facts(), 0).unwrap();
            for tick in 0..10_000 {
                pool.tick(tick).unwrap();
                let population = pool.population();
                assert!(population.live <= 2);
                assert!(population.queued <= 2);
                assert!(population.restarting <= population.live);
                assert!(population.retiring <= population.live);
            }
            json!({"ticks":10000,"maximum_live":2,"maximum_queued":2})
        }
        "fair-pool-selection" => {
            let ready = [false, true, true, false];
            json!({"order":[
                select_fair_pool(&ready, 0).unwrap(),
                select_fair_pool(&ready, 2).unwrap(),
                select_fair_pool(&ready, 3).unwrap()
            ]})
        }
        "evidence-exhaustion-before-mutation" => {
            let mut pool = PoolController::<8, 256>::new(
                contract(
                    1,
                    0,
                    PoolAdmissionPolicy::Reject,
                    PoolSupervisionPolicy::Isolate,
                    PoolCleanupPolicy::Abort,
                    3,
                ),
                PoolGeneration {
                    plan: hash(2),
                    epoch: 7,
                    generation: 1,
                    template_hash: hash(1),
                },
            )
            .unwrap();
            let slot = started(&mut pool, 1, 0);
            pool.mark_running(slot, 0).unwrap();
            pool.progress(slot, 1).unwrap();
            let before = pool.slots()[usize::from(slot)];
            let error = pool.progress(slot, 2).unwrap_err();
            json!({"code":error.code(),"unchanged":before == pool.slots()[usize::from(slot)]})
        }
        "legacy-plan-not-executable" => {
            let error = match instantiate_plan_pool::<2, 64>(
                EXECUTION_PLAN_SCHEMA_VERSION_V15,
                hash(9),
                plan_pool(false),
                7,
                1,
            ) {
                Err(error) => error,
                Ok(_) => panic!("legacy pool unexpectedly executable"),
            };
            json!({"code":error.code()})
        }
        "schema-16-roundtrip" => {
            let pool = plan_pool(true);
            json!({"schema_version":EXECUTION_PLAN_SCHEMA_VERSION,"runtime":pool.runtime.is_some()})
        }
        _ => panic!("unimplemented replicated-pool fixture `{id}`"),
    }
}

#[test]
fn every_replicated_pool_fixture_executes_independently() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../conformance/c4/replicated-pools-v1.json"
    ))
    .unwrap();
    assert_eq!(fixture.cases.len(), 38);
    for case in fixture.cases {
        assert_eq!(execute(&case.id), case.expected, "fixture {}", case.id);
    }
}

#[test]
fn errors_keep_stable_pool_families() {
    assert_eq!(PoolError::ReservationExceeded.code(), "CND-POL-005");
    assert_eq!(HostedPoolError::LegacyPlan.code(), "CND-POOL-HOST-002");
}

#[test]
fn plan_transition_pool_cases_execute_the_real_generation_controller() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c5/plan-transitions-v1.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        if case["runner"] != "replicated-pool" {
            continue;
        }
        match case["id"].as_str().unwrap() {
            "replicated-pool-generation-overlap" => {
                assert_eq!(
                    execute("generation-overlap"),
                    json!({"valid":true,"reserved_slots":4})
                );
            }
            "replicated-pool-generation-rollback" => {
                assert_eq!(
                    execute("generation-rollback"),
                    json!({"cleanup":2,"cancelled":1})
                );
            }
            other => panic!("unhandled transition pool fixture {other}"),
        }
    }
}
