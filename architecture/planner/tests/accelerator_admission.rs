use conduit_core::{
    BootId, CapabilityId, GearId, HostId, ImplementationId, OfferGeneration, ResourcePoolId, SignId,
};
use conduit_planner::{
    select_accelerator_candidate, AcceleratorCandidate, AcceleratorCandidateDisposition,
    AcceleratorDemand, AcceleratorDimension, AcceleratorObservation, AcceleratorOffer,
    AcceleratorPlanningBasis, ExecutionMechanism, ObservationProvenance,
};
use std::collections::BTreeMap;

fn dimensions(vram: u64, queues: u64) -> BTreeMap<AcceleratorDimension, u64> {
    BTreeMap::from([
        (AcceleratorDimension::from("device-memory-bytes"), vram),
        (AcceleratorDimension::from("concurrent-queues"), queues),
    ])
}

fn provenance(id: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(id),
        source: "provider inventory".to_string(),
        observed_at_ms: 900,
        valid_until_ms: 1_100,
    }
}

fn offer() -> AcceleratorOffer {
    AcceleratorOffer {
        host_id: HostId::from("gpu-host"),
        boot_id: BootId::from("gpu-boot-1"),
        offer_generation: OfferGeneration(4),
        capability_id: CapabilityId::from("infer-gpu"),
        implementation_id: ImplementationId::from("provider/infer@1"),
        pool_id: ResourcePoolId::from("gpu-0"),
        capacities: dimensions(16_000, 2),
    }
}

fn observation() -> AcceleratorObservation {
    AcceleratorObservation {
        host_id: HostId::from("gpu-host"),
        boot_id: BootId::from("gpu-boot-1"),
        offer_generation: OfferGeneration(4),
        pool_id: ResourcePoolId::from("gpu-0"),
        resource_generation: 7,
        runtime_usable: true,
        unreserved: dimensions(16_000, 2),
        resident_artifacts: vec!["model-a".to_string()],
        provenance: provenance("gpu-sign-1"),
    }
}

fn mechanism(generation: u64, artifact: Option<&str>) -> ExecutionMechanism {
    ExecutionMechanism::Accelerator {
        host_id: HostId::from("gpu-host"),
        boot_id: BootId::from("gpu-boot-1"),
        offer_generation: OfferGeneration(4),
        capability_id: CapabilityId::from("infer-gpu"),
        implementation_id: ImplementationId::from("provider/infer@1"),
        pool_id: ResourcePoolId::from("gpu-0"),
        resource_generation: generation,
        residency_artifact: artifact.map(str::to_string),
    }
}

fn demand(gear: &str, vram: u64, queues: u64) -> AcceleratorDemand {
    AcceleratorDemand {
        gear_id: GearId::from(gear),
        mechanism: mechanism(7, None),
        dimensions: dimensions(vram, queues),
    }
}

fn cpu(cost: u64) -> AcceleratorCandidate {
    AcceleratorCandidate {
        candidate_id: "cpu".to_string(),
        demands: vec![AcceleratorDemand {
            gear_id: GearId::from("infer"),
            mechanism: ExecutionMechanism::Cpu,
            dimensions: BTreeMap::new(),
        }],
        compute_work_units: cost,
        transfer_work_units: 0,
        setup_work_units: 0,
    }
}

fn gpu(cost: u64, transfer: u64, setup: u64) -> AcceleratorCandidate {
    AcceleratorCandidate {
        candidate_id: "gpu".to_string(),
        demands: vec![demand("infer", 8_000, 1)],
        compute_work_units: cost,
        transfer_work_units: transfer,
        setup_work_units: setup,
    }
}

fn basis() -> AcceleratorPlanningBasis {
    AcceleratorPlanningBasis {
        now_ms: 1_000,
        residency_credit_work_units: 25,
        offers: vec![offer()],
        observations: vec![observation()],
    }
}

#[test]
fn cpu_wins_when_accelerator_transfer_and_setup_dominate() {
    let selection = select_accelerator_candidate(&[cpu(100), gpu(20, 60, 30)], &basis())
        .expect("both exact candidates are comparable");
    assert_eq!(selection.selected_candidate_id, "cpu");
    assert!(selection.explain().contains("current-residency credit"));
}

#[test]
fn accelerator_wins_for_heavy_work_and_seals_exact_dimensions() {
    let selection = select_accelerator_candidate(&[cpu(1_000), gpu(100, 50, 30)], &basis())
        .expect("accelerator is admitted");
    assert_eq!(selection.selected_candidate_id, "gpu");
    let winner = selection
        .considered
        .iter()
        .find(|item| item.disposition == AcceleratorCandidateDisposition::Selected)
        .expect("winner exists");
    assert_eq!(winner.reservations.len(), 1);
    assert_eq!(winner.reservations[0].resource_generation, 7);
    assert_eq!(winner.reservations[0].dimensions, dimensions(8_000, 1));
}

#[test]
fn presence_without_runtime_or_implementation_offer_is_not_capacity() {
    let mut unavailable = basis();
    unavailable.observations[0].runtime_usable = false;
    let selection = select_accelerator_candidate(&[cpu(300), gpu(10, 10, 10)], &unavailable)
        .expect("CPU remains an ordinary candidate");
    assert_eq!(selection.selected_candidate_id, "cpu");
    assert!(matches!(selection.considered[1].disposition,
        AcceleratorCandidateDisposition::Rejected(ref reason) if reason.contains("runtime")));

    unavailable.offers.clear();
    let selection = select_accelerator_candidate(&[cpu(300), gpu(10, 10, 10)], &unavailable)
        .expect("device observation alone cannot become an implementation offer");
    assert!(matches!(selection.considered[1].disposition,
        AcceleratorCandidateDisposition::Rejected(ref reason) if reason.contains("implementation offer")));
}

#[test]
fn individually_fitting_workloads_cannot_oversubscribe_vram_or_queues() {
    let competing = AcceleratorCandidate {
        candidate_id: "two-gpu-works".to_string(),
        demands: vec![demand("infer-a", 9_000, 1), demand("infer-b", 9_000, 1)],
        compute_work_units: 20,
        transfer_work_units: 10,
        setup_work_units: 10,
    };
    let selection = select_accelerator_candidate(&[cpu(500), competing], &basis())
        .expect("oversubscribed candidate is refused without losing CPU fallback");
    assert_eq!(selection.selected_candidate_id, "cpu");
    assert!(matches!(selection.considered[1].disposition,
        AcceleratorCandidateDisposition::Rejected(ref reason) if reason.contains("aggregate")));
}

#[test]
fn stable_residency_is_only_a_fresh_exact_generation_credit() {
    let mut resident = gpu(120, 40, 20);
    if let ExecutionMechanism::Accelerator {
        residency_artifact, ..
    } = &mut resident.demands[0].mechanism
    {
        *residency_artifact = Some("model-a".to_string());
    }
    let selection = select_accelerator_candidate(&[cpu(170), resident.clone()], &basis())
        .expect("fresh residency may avoid repeated setup work");
    assert_eq!(selection.selected_candidate_id, "gpu");
    assert_eq!(selection.considered[1].residency_credit_work_units, 25);

    if let ExecutionMechanism::Accelerator {
        resource_generation,
        ..
    } = &mut resident.demands[0].mechanism
    {
        *resource_generation = 6;
    }
    let selection = select_accelerator_candidate(&[cpu(170), resident], &basis())
        .expect("CPU remains after stale residency truth is rejected");
    assert_eq!(selection.selected_candidate_id, "cpu");
    assert!(matches!(selection.considered[1].disposition,
        AcceleratorCandidateDisposition::Rejected(ref reason) if reason.contains("generation")));
}

#[test]
fn provider_reset_or_capacity_loss_has_ordinary_reselection_semantics() {
    let before = select_accelerator_candidate(&[cpu(500), gpu(50, 40, 20)], &basis())
        .expect("GPU initially wins");
    assert_eq!(before.selected_candidate_id, "gpu");

    let mut after_basis = basis();
    after_basis.observations[0].resource_generation = 8;
    after_basis.observations[0].unreserved = dimensions(4_000, 1);
    after_basis.observations[0].provenance = provenance("gpu-sign-after-reset");
    let selection = select_accelerator_candidate(&[cpu(500), gpu(50, 40, 20)], &after_basis)
        .expect("fresh planning chooses remaining ordinary CPU realization");
    assert_eq!(selection.selected_candidate_id, "cpu");
    assert_eq!(before.planning_basis.observations[0].resource_generation, 7);
    assert_eq!(
        selection.planning_basis.observations[0].resource_generation,
        8
    );
}

#[test]
fn observation_and_search_bounds_fail_closed() {
    let mut stale = basis();
    stale.now_ms = 1_101;
    assert!(select_accelerator_candidate(&[cpu(1)], &stale).is_err());

    let candidates = (0..=conduit_planner::MAXIMUM_ACCELERATOR_CANDIDATES)
        .map(|index| AcceleratorCandidate {
            candidate_id: format!("candidate-{index}"),
            ..cpu(1)
        })
        .collect::<Vec<_>>();
    assert!(select_accelerator_candidate(&candidates, &basis()).is_err());
}
