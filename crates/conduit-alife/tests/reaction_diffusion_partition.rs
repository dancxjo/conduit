use conduit_alife::{
    partition_reaction_diffusion_generation, GrayScottParameters, ReactionDiffusionBoundaryState,
    ReactionDiffusionCell, ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId,
    ReactionDiffusionFieldState, ReactionDiffusionPartition, ReactionDiffusionPartitionRefusal,
    ReactionDiffusionRegion, ReactionDiffusionRegionId, REACTION_DIFFUSION_MAXIMUM_BOUNDARIES,
    REACTION_DIFFUSION_MAXIMUM_BOUNDARY_BYTES, REACTION_DIFFUSION_MAXIMUM_BOUNDARY_VALUES,
    REACTION_DIFFUSION_MAXIMUM_GENERATIONS_IN_FLIGHT, REACTION_DIFFUSION_MAXIMUM_OPERATION_CELLS,
    REACTION_DIFFUSION_MAXIMUM_PARTITION_WORK, REACTION_DIFFUSION_MAXIMUM_REGIONS,
    REACTION_DIFFUSION_MAXIMUM_RETAINED_REGION_CELLS,
};

const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a1-proof01");

#[test]
fn unequal_region_layouts_match_the_direct_oracle_for_multiple_generations() {
    for partition in [vertical_partition(), horizontal_partition()] {
        let mut direct = initial();
        let mut partitioned = initial();
        for generation in 0..6 {
            direct = direct
                .evolve_reference(ReactionDiffusionEvolveRequest {
                    field_id: FIELD_ID,
                    expected_generation: generation,
                    generations: 1,
                    admitted_cell_generations: 80,
                })
                .unwrap();
            let source = partitioned.clone();
            let regions =
                partition_reaction_diffusion_generation(&source, partition.clone()).unwrap();
            partitioned = regions.evolve_and_join().unwrap();
            assert_eq!(partitioned.encode().unwrap(), direct.encode().unwrap());
            assert_eq!(source.generation, generation);
            assert_eq!(source.field_id, FIELD_ID);
        }
    }
}

#[test]
fn partition_requires_unique_exact_complete_coverage() {
    let duplicate = ReactionDiffusionPartition {
        regions: vec![region(0, 0, 0, 4, 10), region(0, 4, 0, 4, 10)],
    };
    assert_eq!(
        duplicate.validate(8, 10),
        Err(ReactionDiffusionPartitionRefusal::DuplicateRegionIdentity)
    );
    let zero = ReactionDiffusionPartition {
        regions: vec![region(0, 0, 0, 0, 10), region(1, 0, 0, 8, 10)],
    };
    assert_eq!(
        zero.validate(8, 10),
        Err(ReactionDiffusionPartitionRefusal::ZeroExtent)
    );
    let outside = ReactionDiffusionPartition {
        regions: vec![region(0, 0, 0, 9, 10)],
    };
    assert_eq!(
        outside.validate(8, 10),
        Err(ReactionDiffusionPartitionRefusal::RegionOutOfRange)
    );
    let overlap = ReactionDiffusionPartition {
        regions: vec![region(0, 0, 0, 5, 10), region(1, 4, 0, 4, 10)],
    };
    assert_eq!(
        overlap.validate(8, 10),
        Err(ReactionDiffusionPartitionRefusal::OverlappingRegions)
    );
    let gap = ReactionDiffusionPartition {
        regions: vec![region(0, 0, 0, 3, 10), region(1, 4, 0, 4, 10)],
    };
    assert_eq!(
        gap.validate(8, 10),
        Err(ReactionDiffusionPartitionRefusal::IncompleteCoverage)
    );
}

#[test]
fn boundary_codec_retains_exact_directed_generation_truth() {
    let generation =
        partition_reaction_diffusion_generation(&initial(), vertical_partition()).unwrap();
    let boundary = &generation.boundaries[0];
    assert_eq!(generation.boundaries.len(), 56);
    assert_eq!(
        generation
            .boundaries
            .iter()
            .map(|boundary| boundary.values.len())
            .sum::<usize>(),
        56
    );
    assert!(generation.boundaries.iter().any(|boundary| {
        boundary.destination_region == ReactionDiffusionRegionId(10)
            && boundary.source_region == ReactionDiffusionRegionId(20)
            && boundary.destination_edge == conduit_alife::ReactionDiffusionBoundaryEdge::East
    }));
    let encoded = boundary.encode().unwrap();
    assert_eq!(
        ReactionDiffusionBoundaryState::decode(&encoded),
        Ok(boundary.clone())
    );

    let mut wrong_profile = encoded.clone();
    wrong_profile[8] ^= 1;
    assert_eq!(
        ReactionDiffusionBoundaryState::decode(&wrong_profile),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryProfile)
    );
    let mut wrong_edge = encoded.clone();
    wrong_edge[44] = 9;
    assert_eq!(
        ReactionDiffusionBoundaryState::decode(&wrong_edge),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge)
    );
    assert_eq!(
        ReactionDiffusionBoundaryState::decode(&encoded[..49]),
        Err(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength)
    );
}

#[test]
fn every_missing_duplicate_stale_and_mismatched_boundary_refuses() {
    let valid = partition_reaction_diffusion_generation(&initial(), vertical_partition()).unwrap();

    let mut missing = valid.clone();
    missing.boundaries.pop();
    assert_eq!(
        missing.validate(),
        Err(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth)
    );

    let mut duplicate_id = valid.clone();
    duplicate_id.boundaries[1].boundary_id = duplicate_id.boundaries[0].boundary_id;
    assert_eq!(
        duplicate_id.validate(),
        Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryIdentity)
    );

    let mut stale = valid.clone();
    stale.boundaries[0].generation += 1;
    assert_eq!(
        stale.validate(),
        Err(ReactionDiffusionPartitionRefusal::StaleBoundaryGeneration)
    );

    let mut wrong_field = valid.clone();
    wrong_field.boundaries[0].field_id = ReactionDiffusionFieldId(*b"field-wrong-0001");
    assert_eq!(
        wrong_field.validate(),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryField)
    );

    let mut wrong_destination = valid.clone();
    wrong_destination.boundaries[0].destination_region = ReactionDiffusionRegionId(99);
    assert_eq!(
        wrong_destination.validate(),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination)
    );

    let mut wrong_source = valid.clone();
    wrong_source.boundaries[0].source_region = ReactionDiffusionRegionId(99);
    assert_eq!(
        wrong_source.validate(),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundarySource)
    );

    let mut wrong_edge = valid.clone();
    wrong_edge.boundaries[0].destination_offset = u16::MAX;
    assert_eq!(
        wrong_edge.validate(),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge)
    );

    let mut wrong_value = valid.clone();
    wrong_value.boundaries[0].values[0] = ReactionDiffusionCell::REST;
    if wrong_value.boundaries[0].values[0] == valid.boundaries[0].values[0] {
        wrong_value.boundaries[0].values[0] = ReactionDiffusionCell::new(0, 1_000_000).unwrap();
    }
    assert_eq!(
        wrong_value.validate(),
        Err(ReactionDiffusionPartitionRefusal::WrongBoundaryValue)
    );

    let mut duplicate_truth = valid.clone();
    duplicate_truth.boundaries[1].destination_region =
        duplicate_truth.boundaries[0].destination_region;
    duplicate_truth.boundaries[1].destination_edge = duplicate_truth.boundaries[0].destination_edge;
    duplicate_truth.boundaries[1].destination_offset =
        duplicate_truth.boundaries[0].destination_offset;
    duplicate_truth.boundaries[1].source_region = duplicate_truth.boundaries[0].source_region;
    duplicate_truth.boundaries[1].values = duplicate_truth.boundaries[0].values.clone();
    assert_eq!(
        duplicate_truth.validate(),
        Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryTruth)
    );

    let mut excess = valid.clone();
    let mut extra = excess.boundaries[0].clone();
    extra.boundary_id = u32::MAX;
    excess.boundaries.push(extra);
    assert_eq!(
        excess.validate(),
        Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth)
    );
}

#[test]
fn generation_overflow_refuses_before_any_region_update() {
    let state = ReactionDiffusionFieldState::from_cells(
        FIELD_ID,
        u64::MAX,
        8,
        10,
        GrayScottParameters::REFERENCE,
        initial().cells().to_vec(),
    )
    .unwrap();
    let partitioned =
        partition_reaction_diffusion_generation(&state, vertical_partition()).unwrap();
    assert_eq!(
        partitioned.evolve_and_join(),
        Err(ReactionDiffusionPartitionRefusal::GenerationOverflow)
    );
}

#[test]
fn partition_exchange_and_operation_bounds_are_exact_and_finite() {
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_REGIONS, 16);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_BOUNDARIES, 4_096);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_BOUNDARY_VALUES, 4_096);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_BOUNDARY_BYTES, 237_568);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_GENERATIONS_IN_FLIGHT, 1);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_PARTITION_WORK, 4_096);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_RETAINED_REGION_CELLS, 4_096);
    assert_eq!(REACTION_DIFFUSION_MAXIMUM_OPERATION_CELLS, 12_288);
}

fn initial() -> ReactionDiffusionFieldState {
    ReactionDiffusionFieldState::initialized(FIELD_ID, 8, 10, GrayScottParameters::REFERENCE, 1703)
        .unwrap()
}

fn vertical_partition() -> ReactionDiffusionPartition {
    ReactionDiffusionPartition {
        regions: vec![region(10, 0, 0, 3, 10), region(20, 3, 0, 5, 10)],
    }
}

fn horizontal_partition() -> ReactionDiffusionPartition {
    ReactionDiffusionPartition {
        regions: vec![region(30, 0, 0, 8, 4), region(40, 0, 4, 8, 6)],
    }
}

fn region(
    id: u16,
    origin_x: u16,
    origin_y: u16,
    width: u16,
    height: u16,
) -> ReactionDiffusionRegion {
    ReactionDiffusionRegion {
        region_id: ReactionDiffusionRegionId(id),
        origin_x,
        origin_y,
        width,
        height,
    }
}
