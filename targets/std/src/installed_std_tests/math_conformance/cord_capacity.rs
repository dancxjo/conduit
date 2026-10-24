use super::{quantity_fixture, BTreeMap, BaseImplementationId, PlanningOptions};
use conduit_core::{ConnectionTrack, FrontValueLocation};
use conduit_planner::{ConnectionQueueLimits, PlannerError};

const SOURCE: &str = r#"plot quantity_capacity {
 source: scalar/literal(value = -1)
 map: math/map-quantity(source-minimum = -2, source-maximum = 0, target-maximum = 100, unit = %)
 wrap: structured-info/wrap-quantity
 show: face/quantity("Capacity")
 source.value >> map.in
 map.out >> wrap.in
 wrap.out >> show.input
}
"#;

fn explicit_capacity(source_gear: &str) -> Result<conduit_core::Plan, PlannerError> {
    let (plot, advertisement) = quantity_fixture::prepare(SOURCE, "quantity_capacity");
    let hosts = [advertisement];
    let placements = conduit_planner::default_expanded_placements(&plot, &hosts).unwrap();
    let bases = [BaseImplementationId::from("conduit.base/local@1")];
    let full =
        conduit_planner::plan_expanded_canonical(&plot, &hosts, &placements, &bases).unwrap();
    let fragment = &full.fragments[0];
    let mut limits = BTreeMap::new();
    for edge in &plot.connections {
        let source = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == edge.source_gear_id)
            .unwrap();
        let sink = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == edge.sink_gear_id)
            .unwrap();
        let cord = fragment
            .connections
            .iter()
            .find(|c| {
                c.source_placement_id == source.placement_id
                    && c.sink_placement_id == sink.placement_id
                    && c.source_port_id == edge.source_port_id
                    && c.sink_port_id == edge.sink_port_id
                    && c.track == edge.track
            })
            .unwrap();
        let mut bytes = cord.byte_capacity;
        if edge.source_gear_id.as_str() == source_gear && edge.track == ConnectionTrack::Payload {
            let front = plot
                .gears
                .iter()
                .find(|g| g.gear_id == edge.source_gear_id)
                .unwrap()
                .checked_front();
            let contract = front
                .value_contracts()
                .iter()
                .find(|c| c.location == FrontValueLocation::Output(edge.source_port_id.clone()))
                .unwrap();
            bytes = contract.contract.maximum_bytes - 1;
            assert!(bytes > 0);
        }
        limits.insert(
            (
                edge.source_gear_id.clone(),
                edge.source_port_id.clone(),
                edge.sink_gear_id.clone(),
                edge.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: cord.item_capacity,
                byte_capacity: bytes,
            },
        );
    }
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
}

#[test]
fn raw_quantity_one_short_cord_refuses_before_play() {
    assert!(matches!(
        explicit_capacity("map"),
        Err(PlannerError::InvalidConnectionBudget(_))
    ));
}

#[test]
fn wrapped_quantity_one_short_cord_refuses_before_play() {
    assert!(matches!(
        explicit_capacity("wrap"),
        Err(PlannerError::InvalidConnectionBudget(_))
    ));
}

#[test]
fn host_queue_ceiling_below_complete_quantity_refuses_before_play() {
    let (plot, mut advertisement) = quantity_fixture::prepare(SOURCE, "quantity_capacity");
    for offer in &mut advertisement.capabilities {
        offer.limits.max_queue_bytes = offer.limits.max_queue_bytes.min(64);
    }
    let hosts = [advertisement];
    let placements = conduit_planner::default_expanded_placements(&plot, &hosts).unwrap();
    let result = conduit_planner::plan_expanded_canonical(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    );
    assert!(matches!(
        result,
        Err(PlannerError::InvalidConnectionBudget(_))
    ));
}
