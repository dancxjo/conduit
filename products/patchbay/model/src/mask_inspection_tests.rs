use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, ConnectionId, HostId, ImplementationId,
    KindIdentity, PlacementId, PlanId, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_presentation::{
    MaskBoundaryPort, MaskBoundaryRole, MaskCordSpecification, MaskPlanningDisposition,
    MaskReconciliation, MaskShowDisposition, MaskSpecification, MaskStageId,
    MaskStageSpecification, MaskWardrobe, MaskWardrobeLifetime, PlannedMask, PlannedMaskCord,
    PlannedMaskStage, SealedMaskRoute,
};

use crate::{project_mask_inspection, MaskInspectionError};

fn port(name: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn mask_specification(name: &str) -> MaskSpecification {
    let stages = ["compose", "speak"]
        .into_iter()
        .enumerate()
        .map(|(index, id)| MaskStageSpecification {
            stage_id: MaskStageId::new(id).unwrap(),
            kind_id: kind_id(if index == 0 {
                "presentation/compose"
            } else {
                "speech/play"
            }),
            kind_contract_revision: KindIdentity::from(format!("conduit.test/{id}@1")),
            inputs: vec![port(
                "input",
                if index == 0 {
                    "presentation/presentation@1"
                } else {
                    "text/text@1"
                },
                PortDirection::Input,
            )],
            outputs: vec![port(
                "output",
                if index == 0 {
                    "text/text@1"
                } else {
                    "presentation/manifestation@1"
                },
                PortDirection::Output,
            )],
        })
        .collect::<Vec<_>>();
    MaskSpecification::new(
        name,
        1,
        stages,
        vec![MaskCordSpecification {
            source_stage_id: MaskStageId::new("compose").unwrap(),
            source_port_id: port_id("output"),
            sink_stage_id: MaskStageId::new("speak").unwrap(),
            sink_port_id: port_id("input"),
            value_kind: kind_id("text/text@1"),
        }],
        vec![
            MaskBoundaryPort {
                role: MaskBoundaryRole::PresentationInput,
                stage_id: MaskStageId::new("compose").unwrap(),
                port_id: port_id("input"),
            },
            MaskBoundaryPort {
                role: MaskBoundaryRole::ShowOutput,
                stage_id: MaskStageId::new("speak").unwrap(),
                port_id: port_id("output"),
            },
        ],
    )
    .unwrap()
}

fn planned(specification: &MaskSpecification) -> PlannedMask {
    let stages = specification
        .stages
        .iter()
        .map(|stage| PlannedMaskStage {
            stage_id: stage.stage_id.clone(),
            placement_id: PlacementId::from(format!("placement/{}", stage.stage_id.as_str())),
            capability_id: CapabilityId::from("capability/presentation"),
            implementation_id: ImplementationId::from(format!(
                "implementation/{}",
                stage.stage_id.as_str()
            )),
            artifact_id: ArtifactId::from("artifact/presenter"),
            host_id: HostId::from("host/local"),
            boot_id: BootId::from("boot/1"),
            resources: vec![],
        })
        .collect::<Vec<_>>();
    PlannedMask {
        specification_id: specification.specification_id.clone(),
        specification_revision: specification.revision,
        plan_id: PlanId::from("plan/current"),
        stage_placements: vec![],
        stages,
        cords: vec![PlannedMaskCord {
            source_stage_id: MaskStageId::new("compose").unwrap(),
            sink_stage_id: MaskStageId::new("speak").unwrap(),
            connection_id: ConnectionId::from("connection/spoken"),
            selected_line: None,
            admitted_lines: vec![],
            item_capacity: 1,
            byte_capacity: 4096,
        }],
    }
}

fn route(specification: &MaskSpecification) -> SealedMaskRoute {
    SealedMaskRoute {
        route_id: "route/spoken".into(),
        specification_id: specification.specification_id.clone(),
        plan_id: PlanId::from("plan/current"),
        stage_ids: vec![
            MaskStageId::new("compose").unwrap(),
            MaskStageId::new("speak").unwrap(),
        ],
        currently_available: true,
    }
}

#[test]
fn inspection_separates_portable_mask_exact_plan_and_replacement_fact() {
    let specification = mask_specification("spoken");
    let doffed = mask_specification("graphical");
    let specification_id = specification.specification_id.clone();
    let planned = planned(&specification);
    let route = route(&specification);
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Wake,
        vec![specification.specification_id.clone()],
        vec![],
    )
    .unwrap();
    let reconciliation = MaskReconciliation {
        show: MaskShowDisposition::NoCurrentShow { prior: None },
        planning: MaskPlanningDisposition::ReplacementRequired,
    };

    let projection = project_mask_inspection(
        &wardrobe,
        &[specification, doffed.clone()],
        &[planned],
        &[route],
        &reconciliation,
        None,
    )
    .unwrap();

    assert_eq!(projection.specifications.len(), 2);
    assert_eq!(
        projection.doffed_specification_ids,
        vec![doffed.specification_id]
    );
    assert!(projection.wardrobe.worn.contains(&specification_id));
    assert_eq!(projection.planned_masks[0].stages.len(), 2);
    assert_eq!(projection.planned_masks[0].cords[0].byte_capacity, 4096);
    assert!(!projection.routes[0].selected);
    assert_eq!(
        projection.planning,
        MaskPlanningDisposition::ReplacementRequired
    );
}

#[test]
fn route_must_be_one_connected_presentation_to_show_path() {
    let specification = mask_specification("spoken");
    let planned = planned(&specification);
    let mut route = route(&specification);
    route.stage_ids.reverse();
    let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![]).unwrap();
    let reconciliation = MaskReconciliation {
        show: MaskShowDisposition::NoCurrentShow { prior: None },
        planning: MaskPlanningDisposition::NotRequired,
    };

    assert_eq!(
        project_mask_inspection(
            &wardrobe,
            &[specification],
            &[planned],
            &[route],
            &reconciliation,
            None,
        ),
        Err(MaskInspectionError::InvalidRoutePath)
    );
}
