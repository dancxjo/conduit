use conduit_core::{
    InstancePath, PlanResourceBudget, PoolAdmissionDisposition, PoolAdmissionFacts,
    PoolAdmissionPolicy, PoolCleanupPolicy, PoolContract, PoolController, PoolGeneration,
    PoolReservationProfile, PoolSupervisionPolicy, PoolWorkIdentity, SemanticHash,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn profile() -> PoolReservationProfile {
    PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 512,
            storage_bytes: 64,
            cpu_units: 1,
            timers: 2,
            transports: 1,
            checkpoints: 1,
            evidence_bytes: 256,
        },
        child_nodes: 2,
        child_cords: 1,
        state_bytes: 64,
        scheduler_slots: 3,
        host_operations: 1,
        cancellation_scopes: 2,
    }
}

fn work(byte: u8) -> PoolWorkIdentity {
    PoolWorkIdentity {
        request: hash(byte),
        work_unit: hash(byte + 20),
        correlation: hash(byte + 40),
    }
}

#[test]
fn rp2040_fixed_storage_pool_never_exceeds_static_population() {
    // This is the linkable RP2040 firmware oracle. Physical transport/board
    // execution remains a separate HIL availability claim.
    let contract = PoolContract {
        pool: InstancePath::new("root/pool.sensor").unwrap(),
        template_hash: hash(1),
        implementation_set_hash: hash(5),
        maximum_live: 2,
        maximum_queued: 2,
        admission: PoolAdmissionPolicy::QueueBounded,
        supervision: PoolSupervisionPolicy::Isolate,
        cleanup: PoolCleanupPolicy::Abort,
        deadline_ticks: 128,
        idle_timeout_ticks: 128,
        cleanup_ticks: 5,
        reservation: profile(),
        total_reservation: profile().checked_mul(5).unwrap(),
        maximum_evidence_events: 64,
    };
    let mut runtime = PoolController::<4, 64>::new(
        contract,
        PoolGeneration {
            plan: hash(2),
            epoch: 1,
            generation: 1,
            template_hash: hash(1),
        },
    )
    .unwrap();
    let facts = PoolAdmissionFacts {
        authority_granted: true,
        sensitivity_allowed: true,
        template_hash: hash(1),
        implementation_set_hash: hash(5),
        available: profile(),
    };
    let first = runtime.offer(work(1), facts, 0).unwrap();
    let second = runtime.offer(work(2), facts, 0).unwrap();
    let PoolAdmissionDisposition::Started { slot: first } = first else {
        panic!("first instance starts");
    };
    let PoolAdmissionDisposition::Started { slot: second } = second else {
        panic!("second instance starts");
    };
    runtime.mark_running(first, 0).unwrap();
    runtime.mark_running(second, 0).unwrap();
    assert!(matches!(
        runtime.offer(work(3), facts, 0).unwrap(),
        PoolAdmissionDisposition::Queued { .. }
    ));
    assert!(matches!(
        runtime.offer(work(4), facts, 0).unwrap(),
        PoolAdmissionDisposition::Queued { .. }
    ));
    for tick in 0..10_000 {
        runtime.tick(tick).unwrap();
        let population = runtime.population();
        assert!(population.live <= 2);
        assert!(population.queued <= 2);
        assert!(population.restarting <= population.live);
        assert!(population.retiring <= population.live);
    }
}
