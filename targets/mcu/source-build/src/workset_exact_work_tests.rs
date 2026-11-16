//! Small checked Source proof: preparation metadata, not runtime admission.
use super::*;
use conduit_embedded_build::ResidentExecutionStorage;

const SOURCE: &str = r#"
type Number = {
    value: U64
}
plot increment (
    input: Number...| >> output: Number...|
) = {value: .value + 1}
plot transform (
    input: Number...| >> output: Number...|
) = {value: (.value * 2) + 3}
plot three (
    >> first: Number...|
    >> second: Number...|
    >> third: Number...|
    first-result: Number...| >>
    second-result: Number...| >>
    third-result: Number...| >>
) {
    a: increment
    b: increment
    c: transform
    first >> a.input
    second >> b.input
    third >> c.input
    a.output >> first-result
    b.output >> second-result
    c.output >> third-result
}
"#;

#[test]
fn repeated_resident_owners_keep_exact_unequal_work_and_original_aggregate_limit() {
    let workset = compile(
        SOURCE.as_bytes(),
        &format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
        &["three".into()],
        None,
    )
    .unwrap();
    let bounds = crate::resident_expression_bounds(1000);
    let storage = ResidentExecutionStorage::SharedWorkspaceSchema;
    let prepared = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, 1000)
        .unwrap();
    assert_eq!(prepared.len(), 3);
    let repeated = prepared
        .iter()
        .filter(|owner| owner.kind_id() == prepared[0].kind_id())
        .collect::<Vec<_>>();
    assert_eq!(repeated.len(), 2);
    assert_eq!(
        repeated[0].computation_work(),
        repeated[1].computation_work()
    );
    assert_ne!(repeated[0].artifact_id(), repeated[1].artifact_id());
    assert_ne!(
        repeated[0].offer_description().capability_id,
        repeated[1].offer_description().capability_id
    );
    assert_eq!(
        prepared
            .iter()
            .map(|owner| &owner.offer_description().capability_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        prepared.len()
    );
    let mut total = 0u32;
    let work = prepared
        .iter()
        .map(|owner| {
            let work = u32::from(owner.computation_work());
            let requirements = &owner.offer_description().resource_requirements;
            assert_eq!(requirements.len(), 1);
            assert_eq!(
                requirements[0].class_id.as_str(),
                conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID
            );
            assert_eq!(requirements[0].units, work);
            total = total.checked_add(work).unwrap();
            work
        })
        .collect::<Vec<_>>();
    assert!(work.iter().max() > work.iter().min());
    assert!((1..=1000).contains(&total));
    let exact = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, total)
        .unwrap();
    assert_eq!(exact.len(), prepared.len());
    for (before, after) in prepared.iter().zip(&exact) {
        assert_eq!(before.offer_description(), after.offer_description());
    }
    let refusal = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, total - 1)
        .err()
        .unwrap();
    assert!(refusal.contains("original pool"), "{refusal}");
    assert!(
        refusal.contains(&format!("requires {total} units")),
        "{refusal}"
    );
}

#[test]
fn heterogeneous_owned_source_uses_exact_requirements_not_equal_pool_shares() {
    let workset = compile(
        SOURCE.as_bytes(),
        &format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
        &["three".into()],
        None,
    )
    .unwrap();
    let bounds = crate::resident_expression_bounds(1000);
    let measured = workset
        .prepare_selected_residents_with_exact_work(bounds, ResidentExecutionStorage::Owned, 1000)
        .unwrap();
    let total: u32 = measured
        .iter()
        .map(|owner| u32::from(owner.computation_work()))
        .sum();
    let average = total / u32::try_from(measured.len()).unwrap();
    assert!(measured
        .iter()
        .any(|owner| u32::from(owner.computation_work()) > average));
    let exact = workset
        .prepare_selected_residents_with_exact_work(bounds, ResidentExecutionStorage::Owned, total)
        .unwrap();
    for owner in &exact {
        assert_eq!(
            owner.offer_description().resource_requirements[0].units,
            u32::from(owner.computation_work())
        );
    }
    // The ordinary planner/lowerer separately retains scheduler port/commit
    // fuel; it must not disappear when the private resource is sealed exactly.
    let host = conduit_core::HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "source-work/host".into(),
        boot_id: "source-work/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "source-work/owned".into(),
        bases: vec![],
        resources: vec![conduit_core::resource_offer(
            "source-work/pool",
            conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
            total,
        )],
        planner_capabilities: vec![],
        capabilities: exact
            .iter()
            .map(|owner| owner.offer_description().clone())
            .collect(),
    };
    let plot = &workset.entries[0].plot;
    assert_eq!(plot.expanded.gears.len(), exact.len());
    let placements = conduit_planner::PlacementChoices {
        by_gear: plot
            .expanded
            .gears
            .iter()
            .zip(&exact)
            .map(|(gear, owner)| {
                (
                    gear.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: owner.offer_description().capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let empty = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let mut boundaries = std::collections::BTreeMap::new();
    for (direction, bindings) in [
        (conduit_core::PortDirection::Input, &plot.input_bindings),
        (conduit_core::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: exact[0].offer_description().limits.max_queue_bytes,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        plot,
        &[host],
        &placements,
        &[conduit_core::LOCAL_BASE_IMPLEMENTATION_ID.into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: exact[0].offer_description().limits.max_queue_bytes,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    for placement in &fragment.placements {
        let private = placement
            .resources
            .iter()
            .find(|r| r.class_id.as_str() == conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID)
            .unwrap()
            .units;
        let base =
            1 + placement.inputs.len() + placement.outputs.len() + placement.host_calls.len();
        assert_eq!(base, 3);
        let node = lowered
            .nodes
            .iter()
            .find(|node| node.placement_id == placement.placement_id)
            .unwrap();
        assert_eq!(
            usize::from(node.maximum_step_fuel),
            usize::try_from(private).unwrap() + base
        );
    }
    let refusal = workset
        .prepare_selected_residents_with_exact_work(
            bounds,
            ResidentExecutionStorage::Owned,
            total - 1,
        )
        .err()
        .unwrap();
    assert!(refusal.contains(&format!(
        "requires {total} units; original pool has {}",
        total - 1
    )));
}
