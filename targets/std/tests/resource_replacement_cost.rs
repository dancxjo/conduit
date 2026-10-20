//! Real hosted publication/reservation bridge; fixed clocks test admission,
//! not measured speed. Actual retained/moved timing belongs to the proof worker.
#[path = "resource_common/provider.rs"]
mod resource_provider;
use conduit_core::*;
use conduit_planner::{proof::resource_frame::*, resource_cost::*};

fn clock(host: &HostAdvertisement) -> MonotonicClockIdentity {
    MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "replacement-cost-clock".into(),
        TemporalScale::Nanoseconds,
        1,
        0,
    )
    .unwrap()
}
fn observations(host: &HostAdvertisement) -> Vec<ResourceObservation> {
    host.resources
        .iter()
        .map(|r| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: r.pool_id.clone(),
            class_id: r.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: r.capacity_units,
            utilized_units: 0,
            sign_id: format!("resource/{}", r.pool_id.as_str()).into(),
        })
        .collect()
}
fn source<'a>(
    host: &'a HostAdvertisement,
    gear: &PlannedGear,
    pool: &ResourcePoolId,
    sign: &'a SignId,
    calibration: CostCalibrationRevision<'a>,
    tick: u64,
) -> CostEvidenceSource<'a> {
    let clock = clock(host);
    CostEvidenceSource::bind(
        host,
        host.capabilities
            .iter()
            .find(|c| c.capability_id == gear.capability_id)
            .unwrap(),
        Some(host.resources.iter().find(|r| &r.pool_id == pool).unwrap()),
        sign,
        calibration,
        CostObservationWindow::admit(
            MonotonicInstant::new(tick, clock.clone()).unwrap(),
            MonotonicInstant::new(tick + 1000, clock).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn real_published_generation_requires_its_exact_live_plan_reservation() {
    let case = frame_resource_plan(false, false).unwrap();
    let gear = case.plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "frame/source")
        .unwrap();
    let binding = &gear.resources[0];
    let mut owner = ResourceAdmissionOwner::new(case.host.clone());
    owner
        .admit_planned_placement(case.plan.plan_id.clone(), gear, &observations(&case.host))
        .unwrap();
    let reader: AuthorityGrantId = "reader/observed-frame".into();
    let empty = resource_provider::provider(binding, &reader, None, None);
    let profile: ArtifactId = "replacement-resource@1".into();
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign: SignId = "residence/published".into();
    let source = source(&case.host, gear, &binding.pool_id, &sign, revision, 100);
    assert!(matches!(
        CostResidentResource::bind(&source, &case.plan, gear, &owner, &empty),
        Err(CostResidenceRefusal::NotPublished)
    ));
    let bytes = vec![7; FRAME_BYTES as usize];
    let provider = empty.initialize(&bytes).unwrap();
    let resident =
        CostResidentResource::bind(&source, &case.plan, gear, &owner, &provider).unwrap();
    assert_eq!(
        resident.provider().cost_resident_bytes(),
        u64::from(FRAME_BYTES)
    );
    assert_eq!(resident.provider().cost_binding(), binding);
    let now = MonotonicInstant::new(150, clock(&case.host)).unwrap();
    let calibrations = [revision];
    let context = CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    assert_eq!(resident.validate_at(&context, &owner), Ok(()));
    let mut released = owner.clone();
    released
        .release(&case.plan.plan_id, &gear.placement_id)
        .unwrap();
    assert_eq!(
        resident.validate_at(&context, &released),
        Err(CostResidenceRefusal::MissingReservation)
    );
    let mut restarted = case.host.clone();
    restarted.boot_id = "restarted".into();
    let restarted_context = CostEvaluationContext {
        host: &restarted,
        ..context
    };
    assert_eq!(
        resident.validate_at(&restarted_context, &owner),
        Err(CostResidenceRefusal::Evidence(CostEvidenceRefusal::Scope(
            CostScopeChange::Boot
        )))
    );
    let expired = MonotonicInstant::new(1100, clock(&case.host)).unwrap();
    let expired_context = CostEvaluationContext {
        now: &expired,
        ..context
    };
    assert_eq!(
        resident.validate_at(&expired_context, &owner),
        Err(CostResidenceRefusal::Evidence(CostEvidenceRefusal::Stale))
    );
}

#[test]
fn complete_cost_facts_keep_physical_uncertainty_and_never_replace_a_plan() {
    let case = frame_resource_plan(false, false).unwrap();
    let before = case.plan.clone();
    let gear = case.plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "frame/source")
        .unwrap();
    let binding = &gear.resources[0];
    let mut owner = ResourceAdmissionOwner::new(case.host.clone());
    owner
        .admit_planned_placement(case.plan.plan_id.clone(), gear, &observations(&case.host))
        .unwrap();
    let reader: AuthorityGrantId = "reader/cost-frame".into();
    let provider = resource_provider::provider(binding, &reader, None, None)
        .initialize(&vec![9; FRAME_BYTES as usize])
        .unwrap();
    let profile: ArtifactId = "replacement-components@1".into();
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let signs: Vec<SignId> = (0..5).map(|i| format!("component/{i}").into()).collect();
    let clock = clock(&case.host);
    let mut work = ExactNumericWork::new(1_000_000_000);
    let measurements: Vec<_> = (0..5)
        .map(|i| {
            CostMeasuredDuration::measure(
                &MonotonicInstant::new(100 + 20 * i, clock.clone()).unwrap(),
                &MonotonicInstant::new(101 + 21 * i, clock.clone()).unwrap(),
                &mut work,
            )
            .unwrap()
        })
        .collect();
    let sources: Vec<_> = signs
        .iter()
        .enumerate()
        .map(|(i, sign)| {
            source(
                &case.host,
                gear,
                &binding.pool_id,
                sign,
                revision,
                measurements[i].completed().ticks(),
            )
        })
        .collect();
    let stages = [
        ReplacementCostStage::Preparation,
        ReplacementCostStage::Transfer,
        ReplacementCostStage::Residency,
        ReplacementCostStage::Migration,
        ReplacementCostStage::Continuity,
    ];
    let components: Vec<_> = (0..5)
        .map(|i| {
            ReplacementCostComponent::bind(
                stages[i],
                &sources[i],
                &case.plan,
                &measurements[i],
                Some(provider.reference()),
            )
            .unwrap()
        })
        .collect();
    let resident =
        CostResidentResource::bind(&sources[0], &case.plan, gear, &owner, &provider).unwrap();
    let residences = [&resident];
    let now = MonotonicInstant::new(300, clock.clone()).unwrap();
    let calibrations = [revision];
    let context = CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let parts = core::array::from_fn(|i| Some(&components[i]));
    let facts = ReplacementCostFacts::admit(
        &case.plan,
        gear,
        &case.plan,
        gear,
        parts,
        &residences,
        &[context],
        &mut work,
    )
    .unwrap();
    let range = facts.sequential_overhead();
    let expected =
        |n, work: &mut ExactNumericWork| ExactRational::ratio(n, 1_000_000_000, work).unwrap();
    assert_eq!(range.lower().seconds(), expected(6, &mut work));
    assert_eq!(range.upper().seconds(), expected(25, &mut work));
    assert_eq!(facts.components().len(), 5);
    assert_eq!(
        facts.residences()[0].provider().cost_resident_bytes(),
        FRAME_BYTES as u64
    );
    assert_eq!(case.plan, before);
    assert!(verify_plan(&case.plan));

    let make_context = || CostEvaluationContext {
        host: &case.host,
        calibrations: &calibrations,
        now: &now,
        execution_observations: &[],
    };
    let mut missing = parts;
    missing[3] = None;
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            missing,
            &residences,
            &[make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::MissingStage)
    ));
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            parts,
            &[],
            &[make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::MissingResidence)
    ));
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            parts,
            &[&resident, &resident],
            &[make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::DuplicateResidence)
    ));
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            parts,
            &residences,
            &[],
            &mut work
        ),
        Err(ReplacementCostRefusal::MissingContext)
    ));
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            parts,
            &residences,
            &[make_context(), make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::AmbiguousContext)
    ));
    let duplicate = ReplacementCostComponent::bind(
        ReplacementCostStage::Transfer,
        &sources[0],
        &case.plan,
        &measurements[0],
        Some(provider.reference()),
    )
    .unwrap();
    let mut duplicates = parts;
    duplicates[1] = Some(&duplicate);
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            duplicates,
            &residences,
            &[make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::DuplicateStageEvidence)
    ));
    assert!(matches!(
        ReplacementCostComponent::bind(
            stages[0],
            &sources[0],
            &case.plan,
            &measurements[1],
            Some(provider.reference())
        ),
        Err(ReplacementCostRefusal::MeasurementScopeChanged)
    ));
    assert!(ReplacementCostFacts::admit(
        &case.plan,
        gear,
        &case.plan,
        gear,
        parts,
        &residences,
        &[make_context()],
        &mut ExactNumericWork::new(0)
    )
    .is_err());
    let mut changed_reference = provider.reference().clone();
    changed_reference.extent.bytes -= 1;
    let other_extent = ReplacementCostComponent::bind(
        stages[0],
        &sources[0],
        &case.plan,
        &measurements[0],
        Some(&changed_reference),
    )
    .unwrap();
    let mut changed_work = parts;
    changed_work[0] = Some(&other_extent);
    assert!(matches!(
        ReplacementCostFacts::admit(
            &case.plan,
            gear,
            &case.plan,
            gear,
            changed_work,
            &residences,
            &[make_context()],
            &mut work
        ),
        Err(ReplacementCostRefusal::ResourceWorkloadChanged)
    ));
    assert!(matches!(
        ReplacementCostComponent::bind(stages[0], &sources[0], &case.plan, &measurements[0], None),
        Err(ReplacementCostRefusal::MissingResourceWorkload)
    ));
    assert_eq!(case.plan, before);
}

#[test]
fn same_generation_back_artifact_mutation_cannot_borrow_an_old_plan_cost() {
    let case = frame_resource_plan(false, false).unwrap();
    let gear = case.plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == "frame/source")
        .unwrap();
    let mut changed = case.host.clone();
    changed
        .capabilities
        .iter_mut()
        .find(|c| c.capability_id == gear.capability_id)
        .unwrap()
        .implementation
        .artifact_id = "changed-artifact-without-generation".into();
    let profile: ArtifactId = "exact-back-cost@1".into();
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign: SignId = "changed-back-cost".into();
    let changed_source = source(
        &changed,
        gear,
        &gear.resources[0].pool_id,
        &sign,
        revision,
        101,
    );
    let measurement = CostMeasuredDuration::measure(
        &MonotonicInstant::new(100, clock(&changed)).unwrap(),
        &MonotonicInstant::new(101, clock(&changed)).unwrap(),
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    assert!(matches!(
        ReplacementCostComponent::bind(
            ReplacementCostStage::Preparation,
            &changed_source,
            &case.plan,
            &measurement,
            None,
        ),
        Err(ReplacementCostRefusal::MeasurementScopeChanged)
    ));
}

#[path = "resource_common/mod.rs"]
mod publication_fixture;
#[test]
fn lost_publication_stays_distinct_from_a_not_yet_published_candidate() {
    let mut generation = publication_fixture::prepared(ResourceSharing::SingleWriterPublished, 1);
    assert_eq!(
        generation.cost_published_reference().unwrap_err(),
        CostResidenceRefusal::NotPublished
    );
    generation
        .write_candidate(
            &publication_fixture::writer(),
            &vec![1; publication_fixture::FRAME_BYTES],
        )
        .unwrap();
    assert_eq!(
        generation.cost_published_reference().unwrap_err(),
        CostResidenceRefusal::NotPublished
    );
    generation.publish(&publication_fixture::writer()).unwrap();
    assert!(generation.cost_published_reference().is_ok());
    generation.retire(&publication_fixture::writer()).unwrap();
    assert_eq!(
        generation.cost_published_reference().unwrap_err(),
        CostResidenceRefusal::Lost
    );
}
