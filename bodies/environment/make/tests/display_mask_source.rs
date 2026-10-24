//! Planning fixtures are descriptions, never initialized renderer owners.
use conduit_core as c;
use conduit_environment_make::{DisplayMaskSource, MaskPlanningRefusal};
use std::collections::BTreeMap;

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
        capabilities: source
            .plot()
            .expanded
            .gears
            .iter()
            .map(|gear| {
                c::BackOfferBuilder::new(
                    fixture_kind(&gear.kind_id),
                    c::Back {
                        capability_id: format!("fixture/{}", gear.gear_id.as_str()).into(),
                        execution_profile_id: "fixture/mask".into(),
                        implementation_id: format!("fixture/{}", gear.gear_id.as_str()).into(),
                        artifact_id: "fixture/mask".into(),
                        host_calls: vec![],
                        resource_requirements: vec![],
                        authority_requirements: vec![],
                    },
                )
                .build()
            })
            .collect(),
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
            &[c::BaseImplementationId::from(c::LOCAL_BASE_IMPLEMENTATION_ID)],
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

fn fixture_kind(id: &c::KindId) -> c::Kind {
    let projection = [
        conduit_face::renderer_kind_projection(),
        conduit_face::face_interaction_kind_projection(),
        conduit_face::face_tee_kind_projection(),
    ]
    .into_iter()
    .find(|projection| &projection.kind_id == id)
    .unwrap();
    c::Kind {
        startup_parameters: c::data_label_startup_front(),
        shorthand: None,
        kind_id: projection.kind_id,
        kind_contract_revision: projection.kind_contract_revision,
        inputs: projection.inputs,
        outputs: projection.outputs,
        configuration: projection.configuration,
        semantic_laws: Default::default(),
        limits: c::CapabilityLimits {
            max_active_instances: conduit_face::MAX_FACE_ACTIVE_INSTANCES,
            max_queue_items: conduit_face::MAX_FACE_QUEUE_ITEMS,
            max_queue_bytes: conduit_face::MAX_RENDERER_VALUE_BYTES
                * u32::from(conduit_face::MAX_FACE_QUEUE_ITEMS),
        },
    }
}
