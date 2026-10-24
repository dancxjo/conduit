//! Planning fixtures are descriptions, never initialized renderer owners.
use conduit_core as c;
use conduit_environment_make::{DisplayMaskSource, MaskPlanningRefusal};
use std::collections::BTreeMap;
#[path = "support/mask_offers.rs"]
mod mask_offers;

#[test]
fn mask_construction_requires_each_selected_back_and_explicit_fore_bounds() {
    let source = DisplayMaskSource::compile().unwrap();
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/mask".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/mask".into(),
        bases: vec![],
        resources: vec![],
        capabilities: mask_offers::offers(&source),
        planner_capabilities: vec![],
    };
    let boundaries: BTreeMap<_, _> = source
        .plot()
        .front
        .inputs()
        .iter()
        .chain(source.plot().front.outputs())
        .map(|port| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: port.direction,
                    front_port_id: port.port_id.clone(),
                    track: c::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: conduit_face::MAX_FACE_TOTAL_BYTES as u32,
                },
            )
        })
        .collect();
    let plan = |host: &c::HostAdvertisement, boundaries: &BTreeMap<_, _>| {
        source.plan(
            core::slice::from_ref(host),
            &[c::BaseImplementationId::from(
                c::LOCAL_BASE_IMPLEMENTATION_ID,
            )],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_face::MAX_FACE_TOTAL_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            boundaries,
        )
    };
    let planned = plan(&host, &boundaries).unwrap();
    assert!(c::verify_plan(&planned.plan));
    assert_eq!(
        planned.mask.plot_identity.expanded_plot_id,
        source.plot().expanded.expanded_plot_id
    );
    assert_eq!(planned.plan.fragments[0].fore_ports.len(), 3);
    assert!(plan(&host, &BTreeMap::new()).is_err());
    for missing in 0..host.capabilities.len() {
        let mut incomplete = host.clone();
        incomplete.capabilities.remove(missing);
        assert!(matches!(
            plan(&incomplete, &boundaries),
            Err(MaskPlanningRefusal::Placement(_))
        ));
    }
}
