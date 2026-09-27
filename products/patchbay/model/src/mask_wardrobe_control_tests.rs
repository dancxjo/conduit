use conduit_body::Body;
use conduit_core::{
    kind_id, port_id, CheckedFormId, KindIdentity, PlanId, PortDescriptor, PortDirection,
    PortTemporal, SignId, SourceDocumentId,
};
use conduit_presentation::{
    BodyMaskWardrobe, MaskBoundaryPort, MaskBoundaryRole, MaskPlanningDisposition,
    MaskShowDisposition, MaskSpecification, MaskStageId, MaskStageSpecification, MaskWardrobe,
    MaskWardrobeLifetime, SealedMaskRoute,
};

use crate::{MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlError};

fn mask(name: &str) -> conduit_presentation::MaskSpecificationId {
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
    .specification_id
}

fn route(
    mask: &conduit_presentation::MaskSpecificationId,
    plan: &str,
    name: &str,
    available: bool,
) -> SealedMaskRoute {
    SealedMaskRoute {
        route_id: name.into(),
        specification_id: mask.clone(),
        plan_id: PlanId::from(plan),
        stage_ids: vec![MaskStageId::new("show").unwrap()],
        currently_available: available,
    }
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
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![graphical.clone()], vec![]).unwrap(),
    )
    .unwrap();
    let routes = vec![
        route(&graphical, "plan/a", "route/graphical", true),
        route(&spoken, "plan/a", "route/spoken", true),
    ];
    let mut control = MaskWardrobeControl::new(
        &body.body_id,
        wardrobe,
        PlanId::from("plan/a"),
        &routes,
        None,
    )
    .unwrap();

    let worn = control
        .apply(0, MaskWardrobeAction::Wear(spoken.clone()), &routes)
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
        .apply(1, MaskWardrobeAction::Doff(graphical), &routes)
        .unwrap();
    let no_route = control
        .apply(2, MaskWardrobeAction::Doff(spoken.clone()), &routes)
        .unwrap();
    assert_eq!(
        no_route.reconciliation.planning,
        MaskPlanningDisposition::NotRequired
    );
    assert!(control.selected.is_none());

    let unsealed = control
        .apply(3, MaskWardrobeAction::Wear(spoken), &[])
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
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![spoken.clone()], vec![]).unwrap(),
    )
    .unwrap();
    let mut control =
        MaskWardrobeControl::new(&body.body_id, wardrobe, PlanId::from("plan/a"), &[], None)
            .unwrap();
    assert_eq!(
        control.admit_replacement_plan(&PlanId::from("plan/a"), PlanId::from("plan/a"), &[]),
        Err(MaskWardrobeControlError::ReusedPlan)
    );
    let replacement = vec![route(&spoken, "plan/b", "route/spoken", true)];
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
