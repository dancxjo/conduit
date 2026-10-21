//! Focused exact partition selection; not physical lane evidence.
extern crate alloc;
use conduit_core::*;
#[allow(dead_code)]
#[path = "../../plan-lowering/tests/common/execution_regions.rs"]
mod fixture;
#[path = "../src/execution_regions.rs"]
mod selection;
use selection::*;
fn input() -> (Plan, Vec<PartitionedRegionSelection>) {
    let mut fragment = fixture::transfer_plan().fragments.remove(0);
    for placement in &mut fragment.placements {
        placement.semantic_contract.laws = vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ];
    }
    let selections = fragment
        .execution_regions
        .drain(..)
        .map(|region| PartitionedRegionSelection {
            region_id: region.region_id,
            placements: region.admitted_placements,
            execution_profile_id: region.execution_profile_id,
            lane_resource: region.lane_resource,
            lane_base_id: region.lane_base_id,
            worker_storage: region.scheduling.worker_storage().unwrap().clone(),
        })
        .collect();
    (fixture::common::seal(fragment), selections)
}
#[test]
fn selection_commits_exact_partition_and_preserves_source_realization() {
    let (original, selections) = input();
    let selected = seal_partitioned_execution(original.clone(), selections).unwrap();
    assert_ne!(selected.plan_id, original.plan_id);
    assert_eq!(selected.source_document_id, original.source_document_id);
    assert_eq!(selected.checked_plot_id, original.checked_plot_id);
    assert_eq!(selected.expanded_plot_id, original.expanded_plot_id);
    assert_eq!(
        selected.fragments[0].placements,
        original.fragments[0].placements
    );
    assert_eq!(
        selected.fragments[0].connections,
        original.fragments[0].connections
    );
    for region in &selected.fragments[0].execution_regions {
        assert_eq!(region.requirements.cord_item_capacity, 2);
        assert_eq!(region.requirements.cord_byte_capacity, 2);
    }
    let prepared = conduit_plan_lowering::execution_regions::PreparedExecutionRegions::prepare(
        &selected,
        &selected.fragments[0].fragment_id,
        conduit_plan_lowering::execution_regions::ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    assert_eq!(prepared.transfers().len(), 1);
    let (original, mut selections) = input();
    let second = selections.pop().unwrap();
    selections[0].placements.extend(second.placements);
    let selected = seal_partitioned_execution(original, selections).unwrap();
    assert_eq!(
        selected.fragments[0].execution_regions[0]
            .requirements
            .cord_byte_capacity,
        2
    );
}
#[test]
fn selection_refuses_missing_duplicate_stale_and_cross_region_startup() {
    let (original, mut selections) = input();
    selections[1].placements.clear();
    assert_eq!(
        seal_partitioned_execution(original, selections).unwrap_err(),
        RegionPlanningError::InvalidPartition
    );
    let (original, mut selections) = input();
    selections[1].placements = selections[0].placements.clone();
    assert_eq!(
        seal_partitioned_execution(original, selections).unwrap_err(),
        RegionPlanningError::InvalidPartition
    );
    let (mut original, selections) = input();
    original.fragments[0].connections[0].byte_capacity += 1;
    assert_eq!(
        seal_partitioned_execution(original, selections).unwrap_err(),
        RegionPlanningError::InvalidPlan
    );
    let (mut original, selections) = input();
    let mut fragment = original.fragments.remove(0);
    fragment.startup_dependencies.push(StartupDependency {
        prerequisite_placement_id: fragment.placements[0].placement_id.clone(),
        dependent_placement_id: fragment.placements[1].placement_id.clone(),
    });
    assert_eq!(
        seal_partitioned_execution(fixture::common::seal(fragment), selections).unwrap_err(),
        RegionPlanningError::UnsupportedPlan
    );
}
