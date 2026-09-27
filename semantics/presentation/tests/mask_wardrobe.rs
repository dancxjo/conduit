use conduit_core::{
    kind_id, port_id, KindIdentity, PlanId, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_presentation::{
    MaskBoundaryPort, MaskBoundaryRole, MaskPlanningDisposition, MaskShowDisposition,
    MaskSpecification, MaskSpecificationId, MaskStageId, MaskStageSpecification, MaskWardrobe,
    MaskWardrobeError, MaskWardrobeLifetime, SealedMaskRoute, SelectedMaskRoute,
};

fn mask(name: &str) -> MaskSpecificationId {
    let stage_id = MaskStageId::new("show").unwrap();
    MaskSpecification::new(
        name,
        1,
        vec![MaskStageSpecification {
            stage_id: stage_id.clone(),
            kind_id: kind_id("presentation/render"),
            kind_contract_revision: KindIdentity::from("conduit.test/presentation-render@1"),
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
                stage_id: stage_id.clone(),
                port_id: port_id("presentation"),
            },
            MaskBoundaryPort {
                role: MaskBoundaryRole::ShowOutput,
                stage_id,
                port_id: port_id("show"),
            },
        ],
    )
    .unwrap()
    .specification_id
}

fn route(mask: &MaskSpecificationId, name: &str, available: bool) -> SealedMaskRoute {
    SealedMaskRoute {
        route_id: name.into(),
        specification_id: mask.clone(),
        plan_id: PlanId::from("plan/current"),
        stage_ids: vec![MaskStageId::new("show").unwrap()],
        currently_available: available,
    }
}

fn selected(mask: &MaskSpecificationId, name: &str) -> SelectedMaskRoute {
    SelectedMaskRoute {
        route_id: name.into(),
        specification_id: mask.clone(),
        plan_id: PlanId::from("plan/current"),
    }
}

#[test]
fn wear_doff_and_preference_are_distinct_revisioned_planning_inputs() {
    let graphical = mask("graphical");
    let spoken = mask("spoken");
    let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![]).unwrap();
    let wardrobe = wardrobe.wear(0, graphical.clone()).unwrap();
    let wardrobe = wardrobe.wear(1, spoken.clone()).unwrap();
    let wardrobe = wardrobe.prefer(2, vec![spoken.clone()]).unwrap();
    assert_eq!(wardrobe.revision, 3);
    assert_eq!(wardrobe.worn, vec![graphical.clone(), spoken.clone()]);
    assert_eq!(wardrobe.preference, vec![spoken.clone()]);

    let wardrobe = wardrobe.doff(3, &spoken).unwrap();
    assert_eq!(wardrobe.worn, vec![graphical]);
    assert!(wardrobe.preference.is_empty());
    assert_eq!(
        wardrobe.wear(3, mask("late")),
        Err(MaskWardrobeError::StaleRevision)
    );
}

#[test]
fn preference_does_not_replace_a_still_valid_current_show() {
    let graphical = mask("graphical");
    let spoken = mask("spoken");
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Wake,
        vec![graphical.clone(), spoken.clone()],
        vec![spoken.clone()],
    )
    .unwrap();
    let current = selected(&graphical, "graphical-a");
    let result = wardrobe
        .reconcile(
            &PlanId::from("plan/current"),
            &[
                route(&graphical, "graphical-a", true),
                route(&spoken, "spoken-a", true),
            ],
            Some(&current),
        )
        .unwrap();
    assert_eq!(result.show, MaskShowDisposition::Retain(current));
    assert_eq!(result.planning, MaskPlanningDisposition::NotRequired);
}

#[test]
fn unavailable_selected_route_uses_only_an_available_route_sealed_in_the_same_plan() {
    let spoken = mask("spoken");
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Wake,
        vec![spoken.clone()],
        vec![spoken.clone()],
    )
    .unwrap();
    let current = selected(&spoken, "generative");
    let result = wardrobe
        .reconcile(
            &PlanId::from("plan/current"),
            &[
                route(&spoken, "generative", false),
                route(&spoken, "deterministic", true),
            ],
            Some(&current),
        )
        .unwrap();
    let MaskShowDisposition::SelectSealed { prior, selected } = result.show else {
        panic!("the sealed deterministic route must be selected")
    };
    assert_eq!(prior, Some(current));
    assert_eq!(selected.route_id, "deterministic");
    assert_eq!(result.planning, MaskPlanningDisposition::NotRequired);
}

#[test]
fn no_realizable_worn_mask_has_no_show_and_requires_authorized_replacement_planning() {
    let spoken = mask("spoken");
    let wardrobe =
        MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![spoken.clone()], vec![]).unwrap();
    let current = selected(&spoken, "spoken-a");
    let result = wardrobe
        .reconcile(
            &PlanId::from("plan/current"),
            &[route(&spoken, "spoken-a", false)],
            Some(&current),
        )
        .unwrap();
    assert_eq!(
        result.show,
        MaskShowDisposition::NoCurrentShow {
            prior: Some(current)
        }
    );
    assert_eq!(
        result.planning,
        MaskPlanningDisposition::ReplacementRequired
    );
}

#[test]
fn a_route_from_an_unsealed_plan_refuses_instead_of_becoming_fallback() {
    let graphical = mask("graphical");
    let wardrobe =
        MaskWardrobe::new(MaskWardrobeLifetime::Wake, vec![graphical.clone()], vec![]).unwrap();
    let mut unsealed = route(&graphical, "newly-discovered", true);
    unsealed.plan_id = PlanId::from("plan/replacement");
    assert_eq!(
        wardrobe.reconcile(&PlanId::from("plan/current"), &[unsealed], None),
        Err(MaskWardrobeError::InvalidRoute)
    );
}
