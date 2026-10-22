use super::*;
use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use conduit_face::{MaskTopologyAdmission, MaskTopologyError};

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/typed-mask"),
        boot_id: BootId::from("boot/typed-mask"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("profile/typed-mask"),
        bases: vec![],
        resources: vec![conduit_core::resource_offer(
            "typed-mask/face",
            FACE_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: vec![],
        capabilities: vec![offer()],
    }
}

#[test]
fn actual_typed_mask_plan_projects_its_show_terminal_and_rejects_unsealed_ports() {
    let planned = planned_mask(&host(), MASK_SOURCE, "browser-graphical").unwrap();
    let topology = MaskTopologyAdmission::from_plan(&planned.plan).unwrap();
    assert_eq!(topology.chains.len(), 1);
    let stages = &topology.chains[0].stages;
    assert_eq!(stages.len(), 1);
    assert_eq!(
        stages[0].placement_id,
        planned.show_placement().placement_id
    );
    assert_eq!(stages[0].host_id, host().host_id);
    assert_eq!(stages[0].input_kind.as_str(), FACE_VALUE_KIND);
    assert_eq!(stages[0].output_kind.as_str(), SHOW_VALUE_KIND);
    let mut unsealed = planned.plan.clone();
    unsealed.fragments[0].placements[0].outputs[0].temporal = PortTemporal::Value;
    assert_eq!(
        MaskTopologyAdmission::from_plan(&unsealed),
        Err(MaskTopologyError::InvalidPlan)
    );
}

#[test]
fn sealed_show_with_wrong_temporal_contract_refuses_instead_of_disappearing() {
    let mut malformed = kind();
    malformed
        .outputs
        .iter_mut()
        .find(|port| port.value_kind.as_str() == SHOW_VALUE_KIND)
        .unwrap()
        .temporal = PortTemporal::Flow { closes: true };
    let mut catalog = ProfileCatalog::new();
    catalog.insert(malformed.clone()).unwrap();
    let plot = conduit_plot::parse(
        "plot malformed {\n mask: face/browser-dom-mask\n}\n",
        &catalog,
    )
    .unwrap();
    let mut host = host();
    host.capabilities[0].outputs = malformed.outputs;
    let choices = conduit_planner::PlacementChoices {
        by_gear: BTreeMap::from([(
            conduit_core::GearId::from("malformed/mask"),
            conduit_planner::PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: host.capabilities[0].capability_id.clone(),
            },
        )]),
    };
    let plan = conduit_planner::plan(
        &plot,
        &[host],
        &choices,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        MaskTopologyAdmission::from_plan(&plan),
        Err(MaskTopologyError::InvalidStageContract)
    );
}
