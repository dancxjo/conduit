use conduit_body::Body;
use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, CheckedFormId, HostId, ImplementationId,
    KindIdentity, PlacementId, PlanId, PortDescriptor, PortDirection, PortTemporal, SignId,
    SourceDocumentId,
};
use conduit_presentation::{
    AdmittedMaskRoutes, BodyMaskWardrobe, MaskBoundaryPort, MaskBoundaryRole,
    MaskPlanningDisposition, MaskShowDisposition, MaskSpecification, MaskStageId,
    MaskStageSpecification, MaskWardrobe, MaskWardrobeLifetime, PlannedMask, PlannedMaskStage,
    Presentation, PresentationBasis, PresentationRole, PresentationSubject, SealedMaskRoute,
};

use crate::{MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlError};

fn mask(name: &str) -> MaskSpecification {
    let stage = MaskStageId::new("show").unwrap();
    MaskSpecification::new(
        name,
        1,
        vec![MaskStageSpecification {
            stage_id: stage.clone(),
            kind_id: kind_id("presentation/render"),
            kind_contract_revision: KindIdentity::from("conduit.test/render@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("presentation"),
                value_kind: kind_id("presentation/presentation@1"),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("show"),
                value_kind: kind_id("presentation/manifestation@1"),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
            }],
        }],
        vec![],
        vec![
            MaskBoundaryPort {
                role: MaskBoundaryRole::PresentationInput,
                stage_id: stage.clone(),
                port_id: port_id("presentation"),
            },
            MaskBoundaryPort {
                role: MaskBoundaryRole::ShowOutput,
                stage_id: stage,
                port_id: port_id("show"),
            },
        ],
    )
    .unwrap()
}

fn route(mask: &MaskSpecification, plan: &str, name: &str) -> SealedMaskRoute {
    SealedMaskRoute {
        route_id: name.into(),
        specification_id: mask.specification_id.clone(),
        plan_id: PlanId::from(plan),
        stage_ids: vec![MaskStageId::new("show").unwrap()],
        currently_available: true,
    }
}

fn admitted(
    plan: &str,
    specifications: &[MaskSpecification],
    routes: Vec<SealedMaskRoute>,
) -> AdmittedMaskRoutes {
    let presentation = presentation(plan);
    let planned = specifications
        .iter()
        .map(|specification| PlannedMask {
            specification_id: specification.specification_id.clone(),
            specification_revision: specification.revision,
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            plan_id: PlanId::from(plan),
            stage_placements: vec![],
            stages: vec![PlannedMaskStage {
                stage_id: MaskStageId::new("show").unwrap(),
                placement_id: PlacementId::from(format!("{plan}/show")),
                capability_id: CapabilityId::from("capability/show"),
                implementation_id: ImplementationId::from("implementation/show"),
                artifact_id: ArtifactId::from("artifact/show"),
                host_id: HostId::from("host/show"),
                boot_id: BootId::from("boot/show"),
                resources: vec![],
            }],
            cords: vec![],
        })
        .collect::<Vec<_>>();
    AdmittedMaskRoutes::new(PlanId::from(plan), specifications, &planned, routes).unwrap()
}

fn presentation(_plan: &str) -> Presentation {
    Presentation::new(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "face/root".into(),
            role: PresentationRole::Document,
            label: "Face".into(),
            accessibility_name: "Face".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/control"),
        CheckedFormId::from("checked/control"),
        0,
        SignId::from("sign/born"),
    )
    .unwrap()
}

#[test]
fn one_action_seam_distinguishes_same_plan_selection_from_replanning_need() {
    let body = body();
    let graphical = mask("graphical");
    let spoken = mask("spoken");
    let wardrobe = BodyMaskWardrobe::new(
        body.body_id.clone(),
        None,
        MaskWardrobe::new(
            MaskWardrobeLifetime::Body,
            vec![graphical.specification_id.clone()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    let routes = admitted(
        "plan/a",
        &[graphical.clone(), spoken.clone()],
        vec![
            route(&graphical, "plan/a", "route/graphical"),
            route(&spoken, "plan/a", "route/spoken"),
        ],
    );
    let mut control = MaskWardrobeControl::new(
        &body.body_id,
        wardrobe,
        PlanId::from("plan/a"),
        &routes,
        None,
    )
    .unwrap();

    let worn = control
        .apply(
            0,
            MaskWardrobeAction::Wear(spoken.specification_id.clone()),
            &routes,
        )
        .unwrap();
    assert_eq!(worn.active_plan_id, PlanId::from("plan/a"));
    assert_eq!(
        worn.reconciliation.planning,
        MaskPlanningDisposition::NotRequired
    );
    assert!(matches!(
        worn.reconciliation.show,
        MaskShowDisposition::Retain(_)
    ));

    control
        .apply(
            1,
            MaskWardrobeAction::Doff(graphical.specification_id.clone()),
            &routes,
        )
        .unwrap();
    let no_route = control
        .apply(
            2,
            MaskWardrobeAction::Doff(spoken.specification_id.clone()),
            &routes,
        )
        .unwrap();
    assert_eq!(
        no_route.reconciliation.planning,
        MaskPlanningDisposition::NotRequired
    );
    assert!(control.selected.is_none());

    let no_routes = admitted("plan/a", &[graphical, spoken.clone()], vec![]);
    let unsealed = control
        .apply(
            3,
            MaskWardrobeAction::Wear(spoken.specification_id),
            &no_routes,
        )
        .unwrap();
    assert_eq!(
        unsealed.reconciliation.planning,
        MaskPlanningDisposition::ReplacementRequired
    );
}

#[test]
fn replacement_admission_requires_fresh_exact_plan_and_matching_routes() {
    let body = body();
    let spoken = mask("spoken");
    let wardrobe = BodyMaskWardrobe::new(
        body.body_id.clone(),
        None,
        MaskWardrobe::new(
            MaskWardrobeLifetime::Body,
            vec![spoken.specification_id.clone()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    let empty = admitted("plan/a", core::slice::from_ref(&spoken), vec![]);
    let mut control = MaskWardrobeControl::new(
        &body.body_id,
        wardrobe,
        PlanId::from("plan/a"),
        &empty,
        None,
    )
    .unwrap();
    assert_eq!(
        control.admit_replacement_plan(&PlanId::from("plan/a"), PlanId::from("plan/a"), &empty,),
        Err(MaskWardrobeControlError::ReusedPlan)
    );
    let replacement = admitted(
        "plan/b",
        core::slice::from_ref(&spoken),
        vec![route(&spoken, "plan/b", "route/spoken")],
    );
    let reconciled = control
        .admit_replacement_plan(
            &PlanId::from("plan/a"),
            PlanId::from("plan/b"),
            &replacement,
        )
        .unwrap();
    assert_eq!(control.active_plan_id, PlanId::from("plan/b"));
    assert!(matches!(
        reconciled.show,
        MaskShowDisposition::SelectSealed { .. }
    ));
}

#[test]
fn fabricated_route_stage_is_refused_before_runtime_selection() {
    let spoken = mask("spoken");
    let mut fabricated = route(&spoken, "plan/a", "route/fabricated");
    fabricated.stage_ids = vec![MaskStageId::new("not-in-plan").unwrap()];
    let planned = PlannedMask {
        specification_id: spoken.specification_id.clone(),
        specification_revision: spoken.revision,
        presentation_id: presentation("plan/a").identity,
        presentation_revision: 1,
        plan_id: PlanId::from("plan/a"),
        stage_placements: vec![],
        stages: vec![],
        cords: vec![],
    };
    assert_eq!(
        AdmittedMaskRoutes::new(
            PlanId::from("plan/a"),
            &[spoken],
            &[planned],
            vec![fabricated],
        ),
        Err(conduit_presentation::MaskWardrobeError::UnsealedRoute)
    );
}
