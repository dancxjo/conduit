use conduit_alife::{
    ReactionDiffusionPartition, ReactionDiffusionPartitionRefusal, ReactionDiffusionRegion,
    ReactionDiffusionRegionId,
};
use conduit_form::rust_binding::NativeRustBinding;

fn region(id: u16) -> ReactionDiffusionRegion {
    ReactionDiffusionRegion::new(ReactionDiffusionRegionId::new(id).unwrap(), id, 0, 1, 1).unwrap()
}

#[test]
fn reaction_diffusion_partition_round_trips_exact_bounded_region_order() {
    for count in [1, 16] {
        let partition =
            ReactionDiffusionPartition::from_regions((0..count).map(region).collect()).unwrap();
        assert_eq!(partition.regions().len(), usize::from(count));
        assert_eq!(
            partition
                .regions()
                .iter()
                .map(|region| *region.region_id().get())
                .collect::<Vec<_>>(),
            (0..count).collect::<Vec<_>>()
        );
        let structured = partition.clone().into_structured().unwrap();
        assert_eq!(
            ReactionDiffusionPartition::from_structured(structured).unwrap(),
            partition
        );
    }
}

#[test]
fn reaction_diffusion_partition_maps_exact_count_bounds_to_domain_refusal() {
    assert_eq!(
        ReactionDiffusionPartition::from_regions(Vec::new()),
        Err(ReactionDiffusionPartitionRefusal::InvalidRegionCount)
    );
    assert_eq!(
        ReactionDiffusionPartition::from_regions((0..17).map(region).collect()),
        Err(ReactionDiffusionPartitionRefusal::InvalidRegionCount)
    );
}
