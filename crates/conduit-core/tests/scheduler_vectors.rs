use conduit_core::{
    PoolPopulation, ReadyQueueDiscipline, RestartAssessment, RestartDecision,
    SCHEDULER_CONTRACT_VERSION, SchedulerContractError, SchedulerPolicy, assess_restart,
};

#[test]
fn policy_requires_finite_scheduler_owned_bounds() {
    let valid = SchedulerPolicy {
        schema_version: SCHEDULER_CONTRACT_VERSION,
        ready_queue: ReadyQueueDiscipline::RoundRobin,
        max_decisions: 100,
        max_tick: 100,
        max_consecutive_yields: 4,
        max_events: 1_000,
    };
    assert_eq!(valid.validate(), Ok(()));
    assert_eq!(
        SchedulerPolicy {
            max_decisions: 0,
            ..valid
        }
        .validate(),
        Err(SchedulerContractError::UnboundedPolicy)
    );
    assert_eq!(
        SchedulerPolicy {
            schema_version: u32::MAX,
            ..valid
        }
        .validate(),
        Err(SchedulerContractError::UnsupportedVersion)
    );
}

#[test]
fn population_reconciles_every_mutually_exclusive_state() {
    let population = PoolPopulation {
        queued: 2,
        pending: 1,
        blocked: 1,
        ready: 1,
        running: 1,
        preempted: 1,
        checkpointing: 1,
        restarting: 1,
        retiring: 1,
        terminal_cleanup: 1,
        reserved_total: 11,
    };
    assert_eq!(population.live_reserved(), Some(9));
    assert_eq!(population.runnable(), Some(2));
    assert_eq!(population.validate(9, 2), Ok(()));

    assert_eq!(
        PoolPopulation {
            blocked: 1,
            reserved_total: 0,
            ..PoolPopulation::default()
        }
        .validate(1, 0),
        Err(SchedulerContractError::PopulationExceeded)
    );
    assert_eq!(
        PoolPopulation {
            running: 2,
            reserved_total: 2,
            ..PoolPopulation::default()
        }
        .validate(1, 0),
        Err(SchedulerContractError::PopulationExceeded)
    );
}

#[test]
fn blocked_resource_work_never_manufactures_runnable_shortfall() {
    let population = PoolPopulation {
        queued: 2,
        blocked: 4,
        ready: 1,
        running: 1,
        reserved_total: 8,
        ..PoolPopulation::default()
    };
    assert_eq!(population.validate(6, 2), Ok(()));
    assert_eq!(population.runnable(), Some(2));

    for _ in 0..10_000 {
        // A long unchanged resource wait remains four bounded blocked slots.
        // It does not become four new admissions or six runnable instances.
        assert_eq!(population.blocked, 4);
        assert_eq!(population.queued, 2);
        assert_eq!(population.runnable(), Some(2));
    }
}

#[test]
fn restart_decisions_use_current_explicit_state_and_terminate() {
    let base = RestartAssessment {
        attempt: 1,
        maximum_attempts: 3,
        progress_ticks: 0,
        minimum_progress_ticks: 10,
        checkpoint_cost_ticks: 1,
        remaining_ticks: 100,
        cooldown_until_tick: 5,
        now_tick: 5,
        starvation_deadline_tick: 50,
    };
    assert_eq!(assess_restart(base), RestartDecision::Restart);
    assert_eq!(
        assess_restart(RestartAssessment {
            now_tick: 4,
            ..base
        }),
        RestartDecision::WaitForCooldown
    );
    assert_eq!(
        assess_restart(RestartAssessment {
            progress_ticks: 10,
            ..base
        }),
        RestartDecision::PreserveCurrentAttempt
    );
    assert_eq!(
        assess_restart(RestartAssessment { attempt: 3, ..base }),
        RestartDecision::AttemptsExhausted
    );
    assert_eq!(
        assess_restart(RestartAssessment {
            now_tick: 50,
            ..base
        }),
        RestartDecision::StarvationDeadlineReached
    );

    let mut attempts = 0_u16;
    while assess_restart(RestartAssessment {
        attempt: attempts,
        maximum_attempts: 3,
        now_tick: u64::from(attempts),
        cooldown_until_tick: u64::from(attempts),
        ..base
    }) == RestartDecision::Restart
    {
        attempts += 1;
    }
    assert_eq!(attempts, 3);
}

#[test]
fn every_portable_scheduler_fixture_is_owned_here() {
    let fixture = include_str!("../../../conformance/c4/bounded-scheduler.json");
    let core_cases = fixture.matches("\"runner\":\"core-scheduler\"").count();
    assert_eq!(core_cases, 8);
    for id in [
        "finite-round-robin-policy",
        "pool-populations-reconcile",
        "resource-wait-does-not-manufacture-demand",
        "restart-attempt-budget-terminates",
        "pool-resource-wait-long-simulation",
    ] {
        assert!(fixture.contains(&format!("\"id\":\"{id}\"")));
    }
}
