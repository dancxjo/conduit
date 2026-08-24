use conduit_core::{
    join_evolved_reaction_diffusion_regions, partition_reaction_diffusion_generation,
    GrayScottParameters, ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId,
    ReactionDiffusionFieldState, ReactionDiffusionPartition, ReactionDiffusionPartitionRefusal,
    ReactionDiffusionRegion, ReactionDiffusionRegionId, ReactionDiffusionRegionWork,
};

const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a2-local01");

#[test]
fn two_host_local_work_matches_direct_without_peer_cells() {
    let partition = unequal_partition();
    let mut distributed = initial();
    let mut direct = initial();
    for generation in 0..5 {
        let source = distributed.clone();
        let partitioned =
            partition_reaction_diffusion_generation(&source, partition.clone()).unwrap();
        let evolved = evolve_all(&partitioned);
        assert!(partitioned
            .regions
            .iter()
            .all(|region| region.cells.len() < source.cells().len()));
        distributed = join_evolved_reaction_diffusion_regions(
            FIELD_ID,
            generation,
            source.width,
            source.height,
            source.parameters,
            &partition,
            &evolved,
        )
        .unwrap();
        direct = direct
            .evolve_reference(ReactionDiffusionEvolveRequest {
                field_id: FIELD_ID,
                expected_generation: generation,
                generations: 1,
                admitted_cell_generations: 80,
            })
            .unwrap();
        assert_eq!(distributed.encode().unwrap(), direct.encode().unwrap());
        assert_eq!(source.generation, generation);
    }
}

#[test]
fn host_local_admission_and_join_refuse_missing_duplicate_stale_and_wrong_truth() {
    let partition = unequal_partition();
    let partitioned =
        partition_reaction_diffusion_generation(&initial(), partition.clone()).unwrap();
    let (contract, cells) = partitioned
        .region_work_basis(ReactionDiffusionRegionId(10))
        .unwrap();
    let first = partitioned
        .boundaries
        .iter()
        .find(|boundary| boundary.destination_region == ReactionDiffusionRegionId(10))
        .unwrap()
        .clone();

    let mut missing = ReactionDiffusionRegionWork::new(contract.clone(), cells.clone()).unwrap();
    assert_eq!(
        missing.clone().evolve(),
        Err(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth)
    );
    missing.admit_boundary(first.clone()).unwrap();
    assert_eq!(
        missing.admit_boundary(first.clone()),
        Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryIdentity)
    );

    let mut stale = first.clone();
    stale.generation += 1;
    assert_eq!(
        ReactionDiffusionRegionWork::new(contract.clone(), cells.clone())
            .unwrap()
            .admit_boundary(stale),
        Err(ReactionDiffusionPartitionRefusal::StaleBoundaryGeneration)
    );
    let mut wrong = first;
    wrong.destination_region = ReactionDiffusionRegionId(20);
    assert_eq!(
        ReactionDiffusionRegionWork::new(contract, cells)
            .unwrap()
            .admit_boundary(wrong),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination)
    );
    let mut wrong_field = partitioned.boundaries[0].clone();
    wrong_field.field_id = ReactionDiffusionFieldId(*b"field-wrong-0001");
    let (contract, cells) = partitioned
        .region_work_basis(wrong_field.destination_region)
        .unwrap();
    assert_eq!(
        ReactionDiffusionRegionWork::new(contract, cells)
            .unwrap()
            .admit_boundary(wrong_field),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryField)
    );

    let valid = evolve_all(&partitioned);
    assert_eq!(
        join_evolved_reaction_diffusion_regions(
            FIELD_ID,
            0,
            8,
            10,
            GrayScottParameters::REFERENCE,
            &partition,
            &valid[..1],
        ),
        Err(ReactionDiffusionPartitionRefusal::MissingRegionResult)
    );
    let duplicate = [valid[0].clone(), valid[0].clone()];
    assert_eq!(
        join_evolved_reaction_diffusion_regions(
            FIELD_ID,
            0,
            8,
            10,
            GrayScottParameters::REFERENCE,
            &partition,
            &duplicate,
        ),
        Err(ReactionDiffusionPartitionRefusal::DuplicateRegionResult)
    );
}

fn evolve_all(
    generation: &conduit_core::PartitionedReactionDiffusionGeneration,
) -> Vec<conduit_core::EvolvedReactionDiffusionRegion> {
    generation
        .regions
        .iter()
        .map(|region| {
            let (contract, cells) = generation
                .region_work_basis(region.region.region_id)
                .unwrap();
            let mut work = ReactionDiffusionRegionWork::new(contract, cells).unwrap();
            for boundary in generation
                .boundaries
                .iter()
                .filter(|boundary| boundary.destination_region == region.region.region_id)
            {
                work.admit_boundary(boundary.clone()).unwrap();
            }
            work.evolve().unwrap()
        })
        .collect()
}

fn initial() -> ReactionDiffusionFieldState {
    ReactionDiffusionFieldState::initialized(FIELD_ID, 8, 10, GrayScottParameters::REFERENCE, 1705)
        .unwrap()
}

fn unequal_partition() -> ReactionDiffusionPartition {
    ReactionDiffusionPartition {
        regions: vec![
            ReactionDiffusionRegion {
                region_id: ReactionDiffusionRegionId(10),
                origin_x: 0,
                origin_y: 0,
                width: 3,
                height: 10,
            },
            ReactionDiffusionRegion {
                region_id: ReactionDiffusionRegionId(20),
                origin_x: 3,
                origin_y: 0,
                width: 5,
                height: 10,
            },
        ],
    }
}
