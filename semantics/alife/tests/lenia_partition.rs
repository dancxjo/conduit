use conduit_alife::{
    orbium_seed, LeniaFieldState, LeniaParameters, LeniaPartition, LeniaPartitionRefusal,
    LeniaRegionId,
};

#[test]
fn three_unequal_regions_match_the_direct_lenia_generation() {
    let initial = orbium_seed(128, 128, 1).unwrap();
    let direct = initial
        .evolve_reference(LeniaParameters::ORBIUM, 1)
        .unwrap();
    let partition = LeniaPartition::vertical(&initial, &[40, 43, 45]).unwrap();
    let results = partition
        .regions()
        .iter()
        .map(|region| {
            let work = partition
                .prepare_region(&initial, region.id, LeniaParameters::ORBIUM)
                .unwrap();
            assert!(work.expanded_cells().len() < initial.cells().len());
            work.evolve(LeniaParameters::ORBIUM).unwrap()
        })
        .collect::<Vec<_>>();
    let joined = partition.join(&results).unwrap();
    assert_eq!(joined, direct);
    assert_eq!(
        joined.semantic_digest().unwrap(),
        direct.semantic_digest().unwrap()
    );
}

#[test]
fn join_refuses_missing_duplicate_stale_and_wrong_field_results() {
    let initial = orbium_seed(128, 128, 1).unwrap();
    let partition = LeniaPartition::vertical(&initial, &[40, 43, 45]).unwrap();
    let results = partition
        .regions()
        .iter()
        .map(|region| {
            partition
                .prepare_region(&initial, region.id, LeniaParameters::ORBIUM)
                .unwrap()
                .evolve(LeniaParameters::ORBIUM)
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        partition.join(&results[..2]),
        Err(LeniaPartitionRefusal::MissingRegion)
    );
    let mut duplicate = results.clone();
    duplicate[2] = duplicate[1].clone();
    assert_eq!(
        partition.join(&duplicate),
        Err(LeniaPartitionRefusal::DuplicateRegion)
    );

    let later = initial
        .evolve_reference(LeniaParameters::ORBIUM, 1)
        .unwrap();
    assert_eq!(
        partition.prepare_region(&later, LeniaRegionId(0), LeniaParameters::ORBIUM),
        Err(LeniaPartitionRefusal::WrongGeneration)
    );
    let other = LeniaFieldState::from_cells(
        conduit_alife::LeniaFieldId([9; 16]),
        initial.generation,
        initial.width,
        initial.height,
        initial.cells().to_vec(),
    )
    .unwrap();
    assert_eq!(
        partition.prepare_region(&other, LeniaRegionId(0), LeniaParameters::ORBIUM),
        Err(LeniaPartitionRefusal::WrongField)
    );
}
