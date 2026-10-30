use super::*;
use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use conduit_face::{MaskTopologyAdmission, MaskTopologyError};

fn host() -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
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
    // CPU identity fixture only; no DOM or physical acknowledgement is earned.
    let face = conduit_face::Face::new(
        1,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_face::FaceSubject {
            identity: "document/typed-mask".into(),
            role: conduit_face::FaceRole::Document,
            name: "Typed Mask".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let terminal = planned.show_placement();
    let play = conduit_core::bind_active_play(
        &planned.plan.plan_id,
        &terminal.host_id,
        &terminal.boot_id,
        1,
    );
    let show = conduit_face::MaskShow::prepared(
        &planned,
        &face,
        play,
        face.subjects[0].identity.clone(),
        "fixture/document".into(),
        "sign/fixture/prepared".into(),
    )
    .unwrap();
    // The old renderer API continues to refuse this ordinary Mask Back.
    assert!(show.show.validate_against(&face, &planned.plan).is_err());
    assert_eq!(
        show.show
            .validate_against_mask_topology(&face, &planned.plan)
            .unwrap()
            .placement_id,
        terminal.placement_id
    );
    let mut foreign = show.show.clone();
    foreign.boot_id = "boot/foreign".into();
    assert_eq!(
        foreign.validate_against_mask_topology(&face, &planned.plan),
        Err(conduit_face::ManifestationError::StaleIdentity)
    );
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
    catalog.insert_kind(malformed.clone()).unwrap();
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

#[test]
fn legacy_presenter_contract_cannot_be_reinterpreted_as_a_terminal_mask() {
    let mut malformed = kind();
    malformed.kind_id = kind_id(conduit_face::PRESENTER_STAGE_KIND);
    malformed.kind_contract_revision =
        KindIdentity::from(conduit_face::PRESENTER_STAGE_CONTRACT_REVISION);
    let mut catalog = ProfileCatalog::new();
    catalog.insert_kind(malformed.clone()).unwrap();
    let plot = conduit_plot::parse(
        "plot malformed {\n mask: face/presenter-stage\n}\n",
        &catalog,
    )
    .unwrap();
    let mut host = host();
    host.capabilities[0].kind_id = malformed.kind_id;
    host.capabilities[0].kind_contract_revision = malformed.kind_contract_revision;
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
        Err(MaskTopologyError::IncompatibleType)
    );
}
