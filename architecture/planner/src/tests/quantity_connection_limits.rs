//! Heterogeneous physical-value queue budgets stay local to each Cord.
use super::*;
use alloc::vec::Vec;

#[test]
fn heterogeneous_connection_limits_preserve_defaults_and_refuse_unfit_overrides() {
    let plot = parse_with_startup(
        "plot heterogeneous {\n large: flow/pulse(count=1, period-ms=0, initial=false)\n large_sink: presentation/show\n small: flow/pulse(count=1, period-ms=0, initial=false)\n small_sink: presentation/show\n large >> large_sink\n small >> small_sink\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    ).unwrap();
    let mut large = host();
    large.host_id = HostId::from("large-host");
    let mut small = host();
    small.host_id = HostId::from("small-host");
    for offer in &mut large.capabilities {
        offer.limits.max_queue_bytes = 119_336;
    }
    for offer in &mut small.capabilities {
        offer.limits.max_queue_bytes = 4_096;
    }
    let hosts = [large, small];
    let placements = PlacementChoices {
        by_gear: plot
            .gears
            .iter()
            .map(|gear| {
                let is_large = gear
                    .gear_id
                    .as_str()
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .starts_with("large");
                let host = &hosts[usize::from(!is_large)];
                let capability = if gear.kind_id.as_str() == PULSE_KIND {
                    "pulse-1"
                } else {
                    "stdout-show-1"
                };
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from(capability),
                    },
                )
            })
            .collect(),
    };
    let empty_bases = BTreeMap::new();
    let empty_lines = BTreeMap::new();
    let options = PlanningOptions {
        connection_bases: &empty_bases,
        line_candidates: &empty_lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 4_096,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let key = crate::connection_endpoints(
        plot.connections
            .iter()
            .find(|connection| connection.source_gear_id.as_str().ends_with("/large"))
            .unwrap(),
    );
    let mut limits = BTreeMap::from([(
        key.clone(),
        crate::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 119_336,
        },
    )]);
    let bases = [BaseImplementationId::from(
        conduit_core::LOCAL_BASE_IMPLEMENTATION_ID,
    )];
    let planned = crate::validated_planning::plan_validated_plot_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &bases,
        options,
        &limits,
    )
    .unwrap();
    assert!(verify_plan(&planned));
    let mut bytes = planned
        .fragments
        .iter()
        .flat_map(|fragment| {
            fragment
                .connections
                .iter()
                .map(|connection| connection.byte_capacity)
        })
        .collect::<Vec<_>>();
    bytes.sort();
    assert_eq!(bytes, vec![4_096, 119_336]);
    limits.get_mut(&key).unwrap().byte_capacity = 119_337;
    assert!(matches!(
        crate::validated_planning::plan_validated_plot_with_connection_limits(
            &plot,
            &hosts,
            &placements,
            &bases,
            options,
            &limits
        ),
        Err(PlannerError::QueueRequirementAboveHostLimit(_))
    ));
    limits.get_mut(&key).unwrap().byte_capacity = 0;
    assert!(matches!(
        crate::validated_planning::plan_validated_plot_with_connection_limits(
            &plot,
            &hosts,
            &placements,
            &bases,
            options,
            &limits
        ),
        Err(PlannerError::InvalidConnectionBudget(_))
    ));
    limits.clear();
    limits.insert(
        (GearId::from("absent"), key.1, key.2, key.3),
        crate::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 4_096,
        },
    );
    assert!(matches!(
        crate::validated_planning::plan_validated_plot_with_connection_limits(
            &plot,
            &hosts,
            &placements,
            &bases,
            options,
            &limits
        ),
        Err(PlannerError::InvalidConnectionBudget(_))
    ));
}
